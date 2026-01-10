use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::args::ServerArgs;
use anyhow::{Context, Result, bail};
use livekit::{DataPacket, Room, RoomEvent, RoomOptions};
use livekit_api::access_token;
use tokio::{net::UdpSocket, sync::mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct PlayerSession {
    cancel: CancellationToken,
    tx_to_udp: mpsc::Sender<Arc<Vec<u8>>>, // LK -> UDP
}

/// Packets from UDP receivers back to the LK publisher
struct UdpToLk {
    player_id: String,
    payload: Arc<Vec<u8>>, // UDP -> LK
}

pub async fn run(args: ServerArgs) -> Result<()> {
    let api_key = std::env::var("LIVEKIT_API_KEY").context("LIVEKIT_API_KEY not set")?;
    let api_secret = std::env::var("LIVEKIT_API_SECRET").context("LIVEKIT_API_SECRET not set")?;

    let room_name = args.game_id.to_string();
    let identity = "server";
    let token = make_token(&room_name, identity, &api_key, &api_secret)?;
    let options = RoomOptions::default();

    let (room, mut events) = Room::connect(&args.livekit_url, &token, options).await?;
    eprintln!("connected: identity={identity} room={room_name}");

    let (tx_udp_to_lk, mut rx_udp_to_lk) = mpsc::channel::<UdpToLk>(1024);
    let mut sessions: HashMap<String, PlayerSession> = HashMap::new();

    loop {
        tokio::select! {
            // UDP -> LK publish path (single owner of `Room`)
            maybe = rx_udp_to_lk.recv() => {
                let Some(msg) = maybe else { continue; };
                publish_to_livekit(&room, &msg.player_id, msg.payload).await?;
            }

            maybe = events.recv() => {
                let Some(ev) = maybe else { break };

                match ev {
                    RoomEvent::ParticipantConnected(participant) => {
                        let pid = participant.identity().to_string();
                        ensure_player_session(
                            &mut sessions,
                            &pid,
                            args.game_port,
                            tx_udp_to_lk.clone(),
                        ).await?;
                    }

                    RoomEvent::ParticipantDisconnected(participant) => {
                        let pid = participant.identity().to_string();
                        if let Some(sess) = sessions.remove(&pid) {
                            sess.cancel.cancel();
                        }
                    }

                    RoomEvent::DataReceived { payload, topic, participant, .. } => {
                        let Some(p) = participant.as_ref() else { continue };
                        if topic.as_deref().unwrap_or("") != "udp" {
                            continue;
                        }

                        let pid = p.identity().to_string();

                        ensure_player_session(
                            &mut sessions,
                            &pid,
                            args.game_port,
                            tx_udp_to_lk.clone(),
                        ).await?;

                        sessions
                            .get(&pid)
                            .unwrap()
                            .tx_to_udp
                            .send(payload)
                            .await
                            .context("failed to send LK->UDP payload")?;
                    }

                    RoomEvent::Disconnected { reason } => {
                        for (_, sess) in sessions.drain() {
                            sess.cancel.cancel();
                        }
                        bail!("disconnected from room: reason={reason:?}");
                    }

                    _ => {}
                }
            }
        }
    }

    Ok(())
}

async fn ensure_player_session(
    sessions: &mut HashMap<String, PlayerSession>,
    player_id: &str,
    game_port: u16,
    tx_udp_to_lk: mpsc::Sender<UdpToLk>,
) -> Result<()> {
    if sessions.contains_key(player_id) {
        return Ok(());
    }

    let (tx_to_udp, rx_to_udp) = mpsc::channel::<Arc<Vec<u8>>>(256);
    let cancel = CancellationToken::new();
    let pid = player_id.to_string();

    tokio::spawn(player_udp_sender(cancel.clone(), rx_to_udp, game_port));
    tokio::spawn(player_udp_receiver(
        cancel.clone(),
        game_port,
        pid.clone(),
        tx_udp_to_lk,
    ));

    sessions.insert(pid.clone(), PlayerSession { cancel, tx_to_udp });

    eprintln!("🧩 created per-player UDP session: {pid}");
    Ok(())
}

async fn player_udp_sender(
    cancel: CancellationToken,
    mut rx: mpsc::Receiver<Arc<Vec<u8>>>,
    game_port: u16,
) -> Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let game_addr = SocketAddr::from(([127, 0, 0, 1], game_port));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break Ok(()),
            Some(buf) = rx.recv() => {
                sock.send_to(buf.as_slice(), game_addr).await?;
            }
        }
    }
}

async fn player_udp_receiver(
    cancel: CancellationToken,
    game_port: u16,
    player_id: String,
    tx_udp_to_lk: mpsc::Sender<UdpToLk>,
) -> Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let game_addr = SocketAddr::from(([127, 0, 0, 1], game_port));
    sock.connect(game_addr).await?;

    let mut buf = vec![0u8; 2048];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break Ok(()),
            res = sock.recv(&mut buf) => {
                let n = res?;
                if n == 0 { continue; }

                // One unavoidable copy here
                let payload = Arc::new(buf[..n].to_vec());

                if tx_udp_to_lk.send(UdpToLk {
                    player_id: player_id.clone(),
                    payload,
                }).await.is_err() {
                    break Ok(());
                }
            }
        }
    }
}

async fn publish_to_livekit(room: &Room, player_id: &str, payload: Arc<Vec<u8>>) -> Result<()> {
    let datapacket = DataPacket {
        topic: Some(format!("udp:{player_id}")),
        payload: payload.as_ref().clone(), // unavoidable copy
        reliable: false,
        ..DataPacket::default()
    };

    room.local_participant()
        .publish_data(datapacket)
        .await
        .context("failed to publish to LiveKit")
}

fn make_token(room: &str, identity: &str, api_key: &str, api_secret: &str) -> Result<String> {
    access_token::AccessToken::with_api_key(api_key, api_secret)
        .with_identity(identity)
        .with_name(identity)
        .with_grants(access_token::VideoGrants {
            room_join: true,
            room: room.to_string(),
            ..Default::default()
        })
        .to_jwt()
        .context("failed to create access token")
}
