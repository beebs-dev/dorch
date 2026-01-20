use crate::{auth, common::AppState, payload::WebsockAuthPayload};
use anyhow::{Context, Result, anyhow, bail};
use async_nats::Subscriber;
use axum::{
    extract::{Query, State, WebSocketUpgrade, ws},
    response::IntoResponse,
};
use bytes::Bytes;
use dorch_common::{
    response,
    streams::{WebsockMessageType, subjects},
};
use futures::{SinkExt, StreamExt};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::Instant,
};
use tokio::{
    select,
    sync::{Mutex, mpsc},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Duration after which, if no pong is received, the connection is considered dead.
const PING_TIMEOUT: i64 = 300_000; // 5 minutes

pub async fn upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(payload): Query<WebsockAuthPayload>,
) -> impl IntoResponse {
    let conn_id: Uuid = payload.conn_id;
    let handshake = match auth::auth_conn(&state.pool, &state.kc, payload).await {
        Ok(v) => v,
        Err(e) => {
            return response::unauthorized(e.context("WebSocket authentication failed"));
        }
    };
    let user_id = handshake.user_id;
    let device_id = handshake.device_id;
    ws.on_upgrade(move |socket| handle_ws(socket, state, user_id, device_id, conn_id))
}

async fn start_transient_subscriber(state: &AppState, user_id: Uuid) -> Result<Subscriber> {
    state
        .nats
        .subscribe(subjects::user(user_id))
        .await
        .context("subscribe to user subject")
}

async fn handle_ws(
    mut socket: ws::WebSocket,
    state: AppState,
    user_id: Uuid,
    device_id: Uuid,
    conn_id: Uuid,
) {
    let master = state.master_tx.subscribe();
    let transients = match start_transient_subscriber(&state, user_id).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{}",
                format!(
                    "⚠️ Failed to start transient subscriber for user {}: {:?}",
                    user_id, e
                )
                .yellow()
            );
            let _ = socket
                .close()
                .await
                .context("Failed to close WebSocket after transient subscriber failure");
            return;
        }
    };
    if let Err(e) = ConnWrapper::new(user_id, device_id, conn_id, state.nats.clone())
        .run(socket, transients, master)
        .await
        .close()
        .await
    {
        eprintln!(
            "{}",
            format!(
                "⚠️ Error during WebSocket connection for user {}: {:?}",
                user_id, e
            )
            .yellow()
        );
    }
}

struct ConnWrapper {
    cancel: CancellationToken,
    user_id: Uuid,
    device_id: Uuid,
    conn_id: Uuid,
    last_pong: Arc<AtomicI64>,
    nats: async_nats::Client,
    enable_self_transients: Arc<AtomicBool>,
    party_id: Arc<Mutex<Option<Uuid>>>,
}

impl ConnWrapper {
    fn new(user_id: Uuid, device_id: Uuid, conn_id: Uuid, nats: async_nats::Client) -> Self {
        Self {
            cancel: CancellationToken::new(),
            conn_id,
            user_id,
            device_id,
            last_pong: Arc::new(AtomicI64::new(Instant::now().elapsed().as_millis() as i64)),
            nats,
            enable_self_transients: Arc::new(AtomicBool::new(false)),
            party_id: Arc::new(Mutex::new(None)),
        }
    }

    async fn close(self) -> Result<Self> {
        self.cancel.cancel();
        Ok(self)
    }

    async fn process_messages(
        &mut self,
        cancel: CancellationToken,
        conn: ws::WebSocket,
        transients: Subscriber,
        master: tokio::sync::broadcast::Receiver<Bytes>,
    ) -> Result<()> {
        let (send_tx, send_rx) = mpsc::channel(100);
        let (msg_rx, ws_join) =
            start_ws_handler(cancel.clone(), send_rx, conn, self.last_pong.clone())
                .context("Failed to create message receiver")?;
        let cancel_clone = cancel.clone();
        let send_tx_clone = send_tx.clone();

        let nats_join = tokio::spawn(proxy_messages(
            self.user_id,
            transients,
            master,
            cancel_clone,
            send_tx_clone,
            self.enable_self_transients.clone(),
        ));

        let result = self
            .process_messages_inner(cancel.clone(), send_tx, msg_rx)
            .await
            .inspect_err(|e| {
                eprintln!(
                    "{}{}{}{}",
                    "❌ Error processing messages • user_id=".red(),
                    self.user_id.red().dimmed(),
                    " • error=".red(),
                    format!("{:?}", e).red().dimmed(),
                );
            });
        cancel.cancel();
        tokio::pin!(ws_join);
        tokio::pin!(nats_join);
        tokio::select! {
            res = &mut ws_join => {
                cancel.cancel();
                res.context("WS handler task join failed")?.context("WS handler task error")?;
                nats_join.await.context("NATS handler task join failed")?.context("NATS handler task error")?;
            },
            res = &mut nats_join => {
                cancel.cancel();
                res.context("NATS handler task join failed")?.context("NATS handler task error")?;
                ws_join.await.context("WS handler task join failed")?.context("WS handler task error")?;
            },
        }
        result
    }

    async fn process_messages_inner(
        &mut self,
        cancel: CancellationToken,
        tx: mpsc::Sender<Bytes>,
        mut rx: mpsc::Receiver<WebsockRxMessage>,
    ) -> Result<()> {
        loop {
            let msg = select! {
                _ = cancel.cancelled() => {
                    return Ok(());
                }
                msg = rx.recv() => msg,
            };
            match msg {
                Some(msg) => {
                    let should_close = self
                        .handle_ws_message(msg, &tx)
                        .await
                        .context("Failed to handle WS message")?;
                    if should_close {
                        return Ok(());
                    } else {
                        continue;
                    }
                }
                None => {
                    return Err(anyhow!("No more messages from WebSocket receiver channel"));
                }
            }
        }
    }

    async fn run(
        mut self,
        conn: ws::WebSocket,
        transients: Subscriber,
        master: tokio::sync::broadcast::Receiver<Bytes>,
    ) -> Self {
        let cancel_token = CancellationToken::new();
        let user_id = self.user_id;
        let device_id = self.device_id;
        let conn_id = self.conn_id;
        let timeout_handle = tokio::spawn(poll_timeout(
            cancel_token.clone(),
            self.last_pong.clone(),
            user_id,
            device_id,
            conn_id,
        ));
        println!(
            "{}{}{}{}{}{}",
            "🔌 Client connected via WebSocket • user_id=".cyan(),
            self.user_id.to_string().cyan().dimmed(),
            " • device_id=".cyan(),
            self.device_id.to_string().cyan().dimmed(),
            " • conn_id=".cyan(),
            self.conn_id.to_string().cyan().dimmed()
        );
        if let Err(e) = self
            .process_messages(cancel_token.clone(), conn, transients, master)
            .await
        {
            eprintln!(
                "{}",
                format!("❌ Failed to process websocket messages: {}", e).red()
            );
        }
        println!(
            "{}{}{}{}{}{}",
            "🚫 Client disconnected • user_id=".yellow(),
            self.user_id.to_string().yellow().dimmed(),
            " • device_id=".yellow(),
            self.device_id.to_string().yellow().dimmed(),
            " • conn_id=".yellow(),
            self.conn_id.to_string().yellow().dimmed()
        );
        cancel_token.cancel();
        let _ = timeout_handle.await;
        self
    }

    async fn handle_message(&mut self, party_id: Uuid, content: String) -> Result<()> {
        if Some(party_id) != self.party_id().await {
            bail!("Invalid party_id for this connection")
        };
        println!(
            "{}{}{}{}",
            "💬 User sent message • user_id=".cyan(),
            self.user_id.to_string().cyan().dimmed(),
            " • party_id=".cyan(),
            party_id.cyan().dimmed(),
        );
        let payload: Vec<u8> = {
            let mut payload = Vec::with_capacity(33 + content.len());
            payload.push(WebsockMessageType::Message.into()); // 1
            payload.extend(party_id.as_bytes()); // 16
            payload.extend(self.user_id.as_bytes()); // 16
            payload.extend(content.as_bytes()); // message content
            payload
        };
        self.nats
            .publish(subjects::party(party_id), payload.into())
            .await
            .context("Failed to publish typing notification to NATS")
    }

    async fn handle_typing(&mut self) -> Result<()> {
        let Some(party_id) = self.party_id().await else {
            bail!("No party_id associated with this connection")
        };
        println!(
            "{}{}{}{}",
            "💬 User is typing • user_id=".cyan(),
            self.user_id.to_string().cyan().dimmed(),
            " • party_id=".cyan(),
            party_id.cyan().dimmed(),
        );
        let payload: Vec<u8> = {
            let mut payload = Vec::with_capacity(33);
            payload.push(WebsockMessageType::Typing.into()); // 1
            payload.extend(party_id.as_bytes()); // 16
            payload.extend(self.user_id.as_bytes()); // 16
            payload
        };
        self.nats
            .publish(subjects::party(party_id), payload.into())
            .await
            .context("Failed to publish typing notification to NATS")
    }

    async fn party_id(&self) -> Option<Uuid> {
        let guard = self.party_id.lock().await;
        *guard
    }

    async fn handle_stop_typing(&mut self) -> Result<()> {
        let Some(party_id) = self.party_id().await else {
            bail!("No party_id associated with this connection")
        };
        println!(
            "{}{}{}{}",
            "💬 User stopped typing • user_id=".cyan(),
            self.user_id.to_string().cyan().dimmed(),
            " • party_id=".cyan(),
            party_id.cyan().dimmed(),
        );
        let payload: Vec<u8> = {
            let mut payload = Vec::with_capacity(33);
            payload.push(WebsockMessageType::StopTyping.into()); // 1
            payload.extend(self.user_id.as_bytes()); // 16
            payload
        };
        self.nats
            .publish(subjects::party(party_id), payload.into())
            .await
            .context("Failed to publish typing notification to NATS")
    }

    async fn handle_ws_message(
        &mut self,
        msg: WebsockRxMessage,
        _send_tx: &mpsc::Sender<Bytes>,
    ) -> Result<bool> {
        match msg {
            WebsockRxMessage::EnableSelfTransients => {
                self.enable_self_transients.store(true, Ordering::SeqCst);
                println!(
                    "{}{}{}{}",
                    "🔄 Enabled proxying of self-sent transient messages • user_id=".cyan(),
                    self.user_id.to_string().cyan().dimmed(),
                    " • device_id=".cyan(),
                    self.device_id.to_string().cyan().dimmed(),
                );
            }
            WebsockRxMessage::Pong => {
                self.last_pong.store(
                    Instant::now().elapsed().as_millis() as i64,
                    Ordering::SeqCst,
                );
            }
            WebsockRxMessage::StopTyping => self.handle_stop_typing().await?,
            WebsockRxMessage::Typing => self.handle_typing().await?,
            WebsockRxMessage::Message { party_id, content } => {
                self.handle_message(party_id, content).await?
            }
        }
        Ok(false) // don't close the connection
    }
}

fn start_ws_handler(
    cancel: CancellationToken,
    send_rx: mpsc::Receiver<Bytes>,
    conn: ws::WebSocket,
    last_pong: Arc<AtomicI64>,
) -> Result<(
    mpsc::Receiver<WebsockRxMessage>,
    tokio::task::JoinHandle<Result<()>>,
)> {
    let (recv_tx, recv_rx) = mpsc::channel(100);
    let proxy_join = tokio::spawn(async move {
        let result = ws_handler_inner(cancel.clone(), send_rx, recv_tx, conn, last_pong).await;
        cancel.cancel();
        result
    });
    Ok((recv_rx, proxy_join))
}

async fn ws_handler_inner(
    cancel: CancellationToken,
    mut send_rx: mpsc::Receiver<Bytes>,
    recv_tx: mpsc::Sender<WebsockRxMessage>,
    mut conn: ws::WebSocket,
    last_pong: Arc<AtomicI64>,
) -> Result<()> {
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_millis(
        ((PING_TIMEOUT / 4) as u64).max(10000),
    ));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        select! {
            _ = cancel.cancelled() => bail!("Context cancelled"),
            _ = heartbeat.tick() => {
                conn.send(ws::Message::Ping(Default::default())).await
                    .context("Failed to send WebSocket ping")?;
            }
            msg = send_rx.recv() => match msg {
                Some(data) => {
                    conn.send(ws::Message::Binary(data))
                        .await
                        .context("Failed to send WebSocket message")?;
                }
                None => bail!("NATS/proxy send channel dropped"),
            },
            msg = conn.recv() => match msg {
                Some(Ok(msg)) => {
                    let close = handle_ws_message(msg, &recv_tx, &mut conn, &last_pong)
                        .await
                        .context("Failed to handle WebSocket message")?;
                    if close {
                        return Ok(());
                    }
                }
                Some(Err(e)) => bail!("WebSocket receive error: {:?}", e),
                None => bail!("No more messages from WebSocket"),
            }
        }
    }
}

async fn handle_ws_message(
    msg: ws::Message,
    recv_tx: &mpsc::Sender<WebsockRxMessage>,
    socket: &mut ws::WebSocket,
    last_pong: &Arc<AtomicI64>,
) -> Result<bool> {
    let payload = match msg {
        ws::Message::Ping(payload) => {
            socket.send(ws::Message::Pong(payload)).await?;
            return Ok(false); // don't close the connection
        }
        ws::Message::Pong(_) => {
            last_pong.store(
                Instant::now().elapsed().as_millis() as i64,
                Ordering::SeqCst,
            );
            return Ok(false); // don't close the connection
        }
        ws::Message::Binary(payload) => payload,
        ws::Message::Text(payload) => payload.into(),
        ws::Message::Close(frame) => {
            eprintln!(
                "{}",
                format!("⚠️ Received WebSocket close frame: {:?}", frame).yellow()
            );
            return Ok(true); // close the connection
        }
    };
    let msg: WebsockRxMessage =
        serde_json::from_slice(&payload).context("Failed to deserialize WebSocket message")?;
    recv_tx
        .send(msg)
        .await
        .context("Failed to forward WS message")?;
    Ok(false) // don't close the connection
}

async fn poll_timeout(
    cancel: CancellationToken,
    last_pong: Arc<AtomicI64>,
    user_id: Uuid,
    device_id: Uuid,
    conn_id: Uuid,
) {
    let start = Instant::now();
    let timeout = std::time::Duration::from_millis(PING_TIMEOUT as u64);
    loop {
        select! {
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(timeout) => {
                let last = last_pong.load(Ordering::SeqCst);
                let now = Instant::now().elapsed().as_millis() as i64;
                let elapsed = now - last;
                if elapsed < PING_TIMEOUT {
                    continue;
                }
                eprintln!(
                    "{}{}{}{}{}{}{}{}",
                    "⚠️ WebSocket connection timed out due to inactivity • uptime=".yellow(),
                    format!("{:?}", start.elapsed()).yellow().dimmed(),
                    " • user_id=".yellow(),
                    user_id.to_string().yellow().dimmed(),
                    " • device_id=".yellow(),
                    device_id.to_string().yellow().dimmed(),
                    " • conn_id=".yellow(),
                    conn_id.to_string().yellow().dimmed()
                );
                cancel.cancel();
                return;
            }
        }
    }
}

async fn send_websocket(
    cancel: &CancellationToken,
    send_tx: &mpsc::Sender<Bytes>,
    payload: Bytes,
) -> Result<bool> {
    tokio::select! {
        _ = cancel.cancelled() => {
            Ok(true)
        },
        res = send_tx.send(payload) => {
            res.context("Failed to forward message to WebSocket")?;
            Ok(false)
        },
    }
}

async fn proxy_messages(
    user_id: Uuid,
    mut transients: Subscriber,
    mut master: tokio::sync::broadcast::Receiver<Bytes>,
    cancel: CancellationToken,
    send_tx: mpsc::Sender<Bytes>,
    enable_self_transients: Arc<AtomicBool>,
) -> Result<()> {
    loop {
        select! {
            _ = cancel.cancelled() => bail!("Connection cancelled"),
            result = master.recv() => match result {
                Ok(payload) => {
                    let payload = WebsockMessageType::game_info(&payload);
                    if send_websocket(&cancel, &send_tx, payload).await? {
                        return Ok(());
                    }
                }
                Err(e) => bail!("Master subscriber error: {:?}", e),
            },
            msg = transients.next() => match msg {
                None => bail!("Transient subscriber closed"),
                Some(msg) => {
                    // Peak the transient message, gating the user's own typing notifications.
                    let ty = msg.payload.first().ok_or_else(|| anyhow!("Received empty message payload from transient subscriber"))?;
                    let ty = WebsockMessageType::try_from(ty)
                        .context("Failed to parse message type from transient NATS message")?;
                    match ty {
                        WebsockMessageType::Typing |
                        WebsockMessageType::StopTyping => {
                            if msg.payload.len() < 33 {
                                eprintln!("Received malformed typing transient message from NATS");
                                continue;
                            }
                            // Skip the thread_id (first 16 bytes after type)
                            let sending_user_id = Uuid::from_slice(&msg.payload[17..33])
                                .context("Failed to parse user_id from transient message")?;
                            if sending_user_id == user_id {
                                let allow = enable_self_transients.load(Ordering::SeqCst);
                                if !allow {
                                    continue;
                                }
                            }
                        }
                        _ => {}
                    }
                    if send_websocket(&cancel, &send_tx, msg.payload).await? {
                        return Ok(());
                    }
                }
            },
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum WebsockRxMessage {
    Pong,
    Message { party_id: Uuid, content: String },
    Typing,
    StopTyping,
    EnableSelfTransients, // for debugging
}
