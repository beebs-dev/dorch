use crate::keys::user_record_key;
use crate::protocol::{ClientMsg, ServerMsg};
use crate::srp::ServerSession;
use crate::user_record_store::UserRecordStore;
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use rand::RngCore;
use redis::AsyncCommands;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// NOTE: use your real Redis pool type from dorch_common.
type RedisPool = deadpool_redis::Pool;

/// Stored per-user in Redis.
#[derive(Clone, Debug)]
pub struct UserRecord {
    pub username: String,
    pub salt: Vec<u8>,
    pub verifier: Vec<u8>,
    pub disabled: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct UserRecordJson {
    pub username: String,
    pub salt_b64: String,
    pub verifier_b64: String,
    #[serde(default)]
    pub disabled: bool,
}

struct SessionState {
    username: String,
    srp: ServerSession,
    created_at_unix: u64,
}

pub async fn run_listener(
    listener: TcpListener,
    store: UserRecordStore,
    cancel: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            accept_res = listener.accept() => {
                let (sock, addr) = accept_res.context("accept failed")?;
                let store = store.clone();
                let cancel = cancel.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(sock, store, cancel).await {
                        eprintln!("auth conn {} error: {:#}", addr, e);
                    }
                });
            }
        }
    }
}

async fn handle_conn(
    sock: TcpStream,
    store: UserRecordStore,
    cancel: CancellationToken,
) -> Result<()> {
    let (read_half, mut write_half) = sock.into_split();
    let mut reader = BufReader::new(read_half);

    let handshake_deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    let mut line = String::new();
    let mut state: Option<SessionState> = None;

    loop {
        line.clear();

        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = write_half.shutdown().await;
                return Ok(());
            }
            _ = tokio::time::sleep_until(handshake_deadline) => {
                send_err(&mut write_half, "timeout", "Handshake timed out").await?;
                return Ok(());
            }
            n = reader.read_line(&mut line) => {
                let n = n.context("read_line failed")?;
                if n == 0 { return Ok(()); }

                while line.ends_with('\n') || line.ends_with('\r') {
                    line.pop();
                    if line.ends_with('\r') { line.pop(); }
                }

                let msg: ClientMsg = match serde_json::from_str(&line) {
                    Ok(m) => m,
                    Err(_) => {
                        send_err(&mut write_half, "bad_request", "Invalid JSON").await?;
                        return Ok(());
                    }
                };

                match (msg, state.take()) {
                    (ClientMsg::Start { username, a_b64, .. }, None) => {
                        let user = store.get(&username).await
                            .with_context(|| format!("unknown user {username}"))?;

                        if user.disabled {
                            send_err(&mut write_half, "disabled", "Account disabled").await?;
                            return Ok(());
                        }

                        let a = B64.decode(a_b64).context("bad base64 A")?;
                        if a.len() < 64 {
                            send_err(&mut write_half, "bad_request", "Invalid A").await?;
                            return Ok(());
                        }

                        let (srp, b_pub) = ServerSession::start(
                            &username,
                            &user.verifier,
                            &user.salt,
                            &a
                        ).context("SRP start failed")?;

                        let session_id = Uuid::new_v4().to_string();
                        let created_at_unix = now_unix();

                        let resp = ServerMsg::Challenge {
                            salt_b64: B64.encode(&user.salt),
                            b_b64: B64.encode(&b_pub),
                            session_id: session_id.clone(),
                        };
                        write_msg(&mut write_half, &resp).await?;

                        state = Some(SessionState {
                            username,
                            srp,
                            created_at_unix,
                        });
                    }

                    (ClientMsg::Proof { m1_b64 }, Some(st)) => {
                        if now_unix().saturating_sub(st.created_at_unix) > 30 {
                            send_err(&mut write_half, "expired", "Session expired").await?;
                            return Ok(());
                        }

                        let m1 = B64.decode(m1_b64).context("bad base64 M1")?;

                        let m2 = match st.srp.verify(&m1) {
                            Ok(m2) => m2,
                            Err(_) => {
                                send_err(&mut write_half, "denied", "Invalid credentials").await?;
                                return Ok(());
                            }
                        };

                        let (token, ttl_secs) = mint_token(&st.username);

                        store_session_token(&pool, &token, &st.username, ttl_secs)
                            .await
                            .context("failed to store session token")?;

                        let resp = ServerMsg::Ok {
                            m2_b64: B64.encode(&m2),
                            token,
                            expires_in_seconds: ttl_secs,
                        };
                        write_msg(&mut write_half, &resp).await?;
                        return Ok(());
                    }

                    (ClientMsg::Start { .. }, Some(_)) => {
                        send_err(&mut write_half, "bad_state", "Already started").await?;
                        return Ok(());
                    }
                    (ClientMsg::Proof { .. }, None) => {
                        send_err(&mut write_half, "bad_state", "Start first").await?;
                        return Ok(());
                    }
                }
            }
        }
    }
}

async fn write_msg(w: &mut tokio::net::tcp::OwnedWriteHalf, msg: &ServerMsg) -> Result<()> {
    let s = serde_json::to_string(msg).context("serialize response")?;
    w.write_all(s.as_bytes()).await?;
    w.write_all(b"\n").await?;
    Ok(())
}

async fn send_err(
    w: &mut tokio::net::tcp::OwnedWriteHalf,
    code: &str,
    message: &str,
) -> Result<()> {
    let msg = ServerMsg::Err {
        code: code.to_string(),
        message: message.to_string(),
    };
    write_msg(w, &msg).await
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

fn mint_token(username: &str) -> (String, u64) {
    let mut rnd = [0u8; 32];
    rand::rng().fill_bytes(&mut rnd);
    let token = format!("u:{}:{}", username, B64.encode(rnd));
    (token, 600)
}

async fn get_user_record(pool: &RedisPool, username: &str) -> Result<UserRecord> {
    // Be deliberately tolerant of formats; we'll try a few common shapes.
    // 1) JSON blob at `auth:user:{username}`
    // 2) Redis hash at `auth:user:{username}` with fields `salt_b64`, `verifier_b64`, `disabled`, `username`
    //    (also accepts `salt` / `verifier` as aliases)

    let key = user_record_key(username);

    let mut conn = pool.get().await.context("redis get conn")?;

    if let Ok(Some(blob)) = conn.get::<_, Option<String>>(&key).await {
        let parsed: UserRecordJson = serde_json::from_str(&blob).context("parse user json")?;
        return decode_user_record(parsed, username);
    }

    let map: std::collections::HashMap<String, String> =
        conn.hgetall(&key).await.unwrap_or_default();

    if !map.is_empty() {
        let parsed = UserRecordJson {
            username: map
                .get("username")
                .cloned()
                .unwrap_or_else(|| username.to_string()),
            salt_b64: map
                .get("salt_b64")
                .cloned()
                .or_else(|| map.get("salt").cloned())
                .context("missing salt_b64")?,
            verifier_b64: map
                .get("verifier_b64")
                .cloned()
                .or_else(|| map.get("verifier").cloned())
                .context("missing verifier_b64")?,
            disabled: map
                .get("disabled")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        };
        return decode_user_record(parsed, username);
    }

    bail!("user not found")
}

async fn store_session_token(
    pool: &RedisPool,
    token: &str,
    username: &str,
    ttl_secs: u64,
) -> Result<()> {
    if token.is_empty() {
        bail!("empty token")
    }
    if username.is_empty() {
        bail!("empty username")
    }
    if ttl_secs == 0 {
        bail!("ttl_secs must be > 0")
    }

    let mut conn = pool.get().await.context("redis get conn")?;
    let key = session_token_key(token);
    conn.set_ex::<_, _, ()>(key, username, ttl_secs)
        .await
        .context("redis set_ex session token")?;
    Ok(())
}

fn session_token_key(token: &str) -> String {
    format!("auth:token:{}", token)
}

fn decode_user_record(parsed: UserRecordJson, requested_username: &str) -> Result<UserRecord> {
    let username = if parsed.username.is_empty() {
        requested_username.to_string()
    } else {
        parsed.username
    };

    let salt = B64.decode(parsed.salt_b64).context("decode salt")?;
    let verifier = B64.decode(parsed.verifier_b64).context("decode verifier")?;

    if username.is_empty() {
        bail!("empty username in record")
    }
    if salt.is_empty() {
        bail!("empty salt")
    }
    if verifier.is_empty() {
        bail!("empty verifier")
    }

    Ok(UserRecord {
        username,
        salt,
        verifier,
        disabled: parsed.disabled,
    })
}
