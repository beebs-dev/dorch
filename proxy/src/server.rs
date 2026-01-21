use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::args::ServerArgs;
use anyhow::{Context, Result, bail};
use dorch_common::shutdown::shutdown_signal;
use livekit::id::ParticipantIdentity;
use livekit::{DataPacket, Room, RoomEvent, RoomOptions};
use livekit_api::access_token;
use tokio::{net::UdpSocket, sync::mpsc};
use tokio_util::sync::CancellationToken;

fn udp_debug_enabled() -> bool {
    std::env::var_os("DORCH_UDP_DEBUG").is_some()
}

fn hex_prefix(buf: &[u8], max: usize) -> String {
    use std::fmt::Write;

    let n = buf.len().min(max);
    let mut out = String::with_capacity(n * 3);
    for (i, b) in buf[..n].iter().enumerate() {
        if i != 0 {
            out.push(' ');
        }
        let _ = write!(&mut out, "{:02x}", b);
    }
    if buf.len() > n {
        out.push_str(" …");
    }
    out
}

struct PlayerSessionTasks {
    sender: tokio::task::JoinHandle<Result<()>>,
    receiver: tokio::task::JoinHandle<Result<()>>,
}

#[derive(Clone)]
struct PlayerSession {
    cancel: CancellationToken,
    tx_to_udp: mpsc::Sender<Arc<Vec<u8>>>, // LK -> UDP
    tasks: Arc<Mutex<Option<PlayerSessionTasks>>>,
    created_at: Instant,
    sock: Arc<UdpSocket>,
}

/// Packets from UDP receivers back to the LK publisher
struct UdpToLk {
    player_id: String,
    payload: Arc<Vec<u8>>, // UDP -> LK
}

async fn cancel_all_sessions(sessions: &mut HashMap<String, PlayerSession>) {
    sessions
        .iter_mut()
        .map(|(_, sess)| &sess.cancel)
        .for_each(|cancel| {
            cancel.cancel();
        });
    for (_, sess) in sessions.drain() {
        sess.cancel.cancel();
        let tasks = sess.tasks.lock().unwrap().take();
        if let Some(tasks) = tasks {
            let _ = tasks.sender.await;
            let _ = tasks.receiver.await;
        }
    }
}

pub async fn run(args: ServerArgs) -> Result<()> {
    if udp_debug_enabled() {
        eprintln!("DORCH_UDP_DEBUG enabled: will log LiveKit/UDP packets");
    }
    let api_key = std::env::var("LIVEKIT_API_KEY").context("LIVEKIT_API_KEY not set")?;
    let api_secret = std::env::var("LIVEKIT_API_SECRET").context("LIVEKIT_API_SECRET not set")?;
    let room_name = args.game_id.to_string();
    let identity = "server";
    let token = make_token(&room_name, identity, &api_key, &api_secret)?;
    let options = RoomOptions::default();
    println!("connecting to LiveKit at {}", args.livekit_url);
    let (room, mut events) = Room::connect(&args.livekit_url, &token, options).await?;
    eprintln!("connected: identity={identity} room={room_name}");
    let (tx_udp_to_lk, mut rx_udp_to_lk) = mpsc::channel::<UdpToLk>(1024);
    let mut sessions: HashMap<String, PlayerSession> = HashMap::new();
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        println!("SIGINT received, shutting down...");
        cancel_clone.cancel();
    });
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            maybe = rx_udp_to_lk.recv() => {
                let Some(msg) = maybe else { continue; };

                // During initial handshake we prefer reliable delivery; afterwards use lossy
                // to avoid head-of-line blocking for real-time game packets.
                let reliable = sessions
                    .get(&msg.player_id)
                    .map(|s| s.created_at.elapsed() < Duration::from_secs(5))
                    .unwrap_or(true);

                publish_to_livekit(&room, &msg.player_id, msg.payload, reliable)
                    .await
                    .context("failed to publish to livekit")?;
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
                        ).await.context("failed to ensure player session")?;
                        println!("participant connected: {pid}");
                    }
                    RoomEvent::ParticipantDisconnected(participant) => {
                        let pid = participant.identity().to_string();
                        eprintln!("⛓️‍💥 participant disconnected: {pid}");
                        if let Some(sess) = sessions.remove(&pid) {
                            sess.cancel.cancel();
                            let task = sess.tasks.lock().unwrap().take();
                            if let Some(tasks) = task {
                                let _ = tasks.sender.await;
                                let _ = tasks.receiver.await;
                            }
                            // Send a fabricated disconnect packet to the game server
                            // to help it recognize the disconnect.
                            let game_addr = SocketAddr::from(([127, 0, 0, 1], args.game_port));
                            sess.sock.send_to(&[0x00, 0xd9], game_addr).await.ok();
                        }
                    }
                    RoomEvent::DataReceived { payload, topic, participant, .. } => {
                        let Some(p) = participant.as_ref() else { continue };
                        if udp_debug_enabled() {
                            println!(
                                "LiveKit DataReceived: from={} topic={:?} len={} hex={}",
                                p.identity(),
                                topic,
                                payload.len(),
                                hex_prefix(payload.as_slice(), 48)
                            );
                        }

                        // Some clients/SDK signatures may omit the topic.
                        // Treat missing topic as UDP for this app.
                        if topic.as_deref() != Some("udp") && topic.is_some() {
                            continue;
                        }
                        let pid = p.identity().to_string();
                        ensure_player_session(
                            &mut sessions,
                            &pid,
                            args.game_port,
                            tx_udp_to_lk.clone(),
                        ).await.context("failed to ensure player session")?;
                        let sess = sessions.get_mut(&pid).unwrap();
                        sess.tx_to_udp.send(payload)
                            .await
                            .context("failed to send LK->UDP payload")?;
                    }
                    RoomEvent::Disconnected { reason } => {
                        // print the last payload as hex for debugging
                        bail!("disconnected from room: reason={reason:?}");
                    }
                    _ => {}
                }
            }
        }
    }
    println!("Graceful shutdown initiated");
    cancel_all_sessions(&mut sessions).await;
    println!("Proxy server gracefully shut down");
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
    let game_addr = SocketAddr::from(([127, 0, 0, 1], game_port));
    // Bind explicitly to loopback since the game server is in the same pod.
    // This avoids any weirdness where packets to 127.0.0.1 might otherwise
    // pick a non-loopback source address.
    let sock = UdpSocket::bind("127.0.0.1:0")
        .await
        .context("failed to bind UDP socket")?;
    let local_addr = sock.local_addr().context("failed to get UDP local addr")?;

    let sock = Arc::new(sock);
    let (tx_to_udp, rx_to_udp) = mpsc::channel::<Arc<Vec<u8>>>(256);
    let cancel = CancellationToken::new();
    let pid = player_id.to_string();
    let sender = tokio::spawn({
        let cancel = cancel.clone();
        let sock = sock.clone();
        async move {
            let res = player_udp_sender(cancel.clone(), sock.clone(), game_addr, rx_to_udp).await;
            println!(
                "⛔ UDP sender for player {} exiting (udp_local={}): {:?}",
                pid, local_addr, res
            );
            res
        }
    });
    let receiver = tokio::spawn({
        let pid = player_id.to_string();
        let cancel = cancel.clone();
        let sock = sock.clone();
        async move {
            let res = player_udp_receiver(cancel, sock, &pid, tx_udp_to_lk).await;
            println!(
                "⛔ UDP receiver for player {} exiting (udp_local={}): {:?}",
                pid, local_addr, res
            );
            res.context("player UDP receiver failed")
        }
    });
    sessions.insert(
        player_id.to_string(),
        PlayerSession {
            cancel,
            tx_to_udp,
            tasks: Arc::new(Mutex::new(Some(PlayerSessionTasks { sender, receiver }))),
            created_at: Instant::now(),
            sock,
        },
    );
    eprintln!(
        "🧩 created per-player UDP session: {player_id} udp_local={local_addr} udp_peer={game_addr}"
    );
    Ok(())
}

async fn player_udp_sender(
    cancel: CancellationToken,
    sock: Arc<UdpSocket>,
    game_addr: SocketAddr,
    mut rx: mpsc::Receiver<Arc<Vec<u8>>>,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                eprintln!("🧩 UDP sender cancelled, shutting down");
                break Ok(())
            },
            Some(buf) = rx.recv() => {
                if udp_debug_enabled() {
                    println!(
                        "Sending UDP packet to game server, dst={} len={} hex={}",
                        game_addr,
                        buf.len(),
                        hex_prefix(buf.as_slice(), 48)
                    );
                }
                sock.send_to(buf.as_slice(), game_addr)
                    .await
                    .context("failed to send UDP packet")?;
            }
        }
    }
}

async fn player_udp_receiver(
    cancel: CancellationToken,
    sock: Arc<UdpSocket>,
    player_id: &str,
    tx_udp_to_lk: mpsc::Sender<UdpToLk>,
) -> Result<()> {
    let mut buf = vec![0u8; 2048];
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                eprintln!("🧩 UDP receiver for player {player_id} cancelled, shutting down");
                break Ok(())
            },
            res = sock.recv_from(&mut buf) => {
                let (n, from) = res.context("failed to receive UDP packet")?;
                if n == 0 { continue; }

                // One unavoidable copy here
                let payload = Arc::new(buf[..n].to_vec());
                if udp_debug_enabled() {
                    println!(
                        "Received UDP packet from game server, from={} len={} hex={}",
                        from,
                        n,
                        hex_prefix(&buf[..n], 48)
                    );
                }
                if tx_udp_to_lk.send(UdpToLk {
                    player_id: player_id.to_string(),
                    payload,
                }).await.is_err() {
                    eprintln!("🧩 UDP receiver for player {player_id} shutting down (udp->lk channel closed)");
                    break Ok(());
                }
            }
        }
    }
}

async fn publish_to_livekit(
    room: &Room,
    player_id: &str,
    payload: Arc<Vec<u8>>,
    reliable: bool,
) -> Result<()> {
    let datapacket = DataPacket {
        // Dwasm expects topic to be exactly "udp".
        topic: Some("udp".to_string()),
        payload: payload.as_ref().clone(), // unavoidable copy
        reliable,
        // Don't broadcast game UDP to all participants; only deliver to the owning player.
        destination_identities: vec![ParticipantIdentity(player_id.to_string())],
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
