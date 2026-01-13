use anyhow::{Context, Result, bail};
use rand::RngCore;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::user_record_store::UserRecordStore;
use crate::zandronum_srp_sha256::{SrpServerSession, UserSecrets};

const AUTH_PROTOCOL_VERSION: u8 = 2;

const SERVER_AUTH_NEGOTIATE: u32 = 0xD003CA01;
const SERVER_AUTH_SRP_STEP_ONE: u32 = 0xD003CA02;
const SERVER_AUTH_SRP_STEP_THREE: u32 = 0xD003CA03;

const AUTH_SERVER_NEGOTIATE: u32 = 0xD003CA10;
const AUTH_SERVER_SRP_STEP_TWO: u32 = 0xD003CA20;
const AUTH_SERVER_SRP_STEP_FOUR: u32 = 0xD003CA30;
const AUTH_SERVER_USER_ERROR: u32 = 0xD003CAFF;
const AUTH_SERVER_SESSION_ERROR: u32 = 0xD003CAEE;

const USER_TRY_LATER: u8 = 0;
const USER_NO_EXIST: u8 = 1;
const USER_OUTDATED_PROTOCOL: u8 = 2;
const USER_WILL_NOT_AUTH: u8 = 3;

const SESSION_NO_EXIST: u8 = 1;
const SESSION_VERIFIER_UNSAFE: u8 = 2;
const SESSION_AUTH_FAILED: u8 = 3;

const MAX_PACKET: usize = 2048;
const SESSION_TTL: Duration = Duration::from_secs(30);

#[derive(Debug)]
struct Session {
    created_at_unix: u64,
    #[allow(dead_code)]
    client_session_id: u32,
    srp: SrpServerSession,
}

pub async fn run_udp(
    bind_addr: &str,
    store: UserRecordStore,
    cancel: CancellationToken,
) -> Result<()> {
    let sock = UdpSocket::bind(bind_addr)
        .await
        .with_context(|| format!("bind udp {bind_addr}"))?;

    let sessions: Mutex<HashMap<i32, Session>> = Mutex::new(HashMap::new());

    let mut buf = [0u8; MAX_PACKET];

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            recv = sock.recv_from(&mut buf) => {
                let (n, peer) = recv.context("udp recv_from")?;
                let pkt = &buf[..n];
                if let Err(e) = handle_packet(&sock, &store, &sessions, pkt, peer).await {
                    // Intentionally don't spam; Zandronum can be chatty.
                    eprintln!("zandronum auth: packet from {peer} failed: {e:#}");
                }
            }
        }
    }
}

async fn handle_packet(
    sock: &UdpSocket,
    store: &UserRecordStore,
    sessions: &Mutex<HashMap<i32, Session>>,
    pkt: &[u8],
    peer: std::net::SocketAddr,
) -> Result<()> {
    let now = now_unix();

    // Cleanup old sessions opportunistically.
    {
        let mut map = sessions.lock().await;
        map.retain(|_, s| now.saturating_sub(s.created_at_unix) <= SESSION_TTL.as_secs());
    }

    let mut r = Reader::new(pkt);
    let cmd = r.read_u32_le().context("read command")?;

    match cmd {
        SERVER_AUTH_NEGOTIATE => {
            let proto = r.read_u8().context("read proto")?;
            let client_session_id = r.read_u32_le().context("read clientSessionID")?;
            let username = r.read_cstring().context("read username")?;

            if proto != AUTH_PROTOCOL_VERSION {
                send_user_error(sock, peer, USER_OUTDATED_PROTOCOL, client_session_id).await?;
                return Ok(());
            }

            let user = match store.get(&username).await {
                Ok(Some(u)) => u,
                Ok(None) | Err(_) => {
                    send_user_error(sock, peer, USER_NO_EXIST, client_session_id).await?;
                    return Ok(());
                }
            };

            if user.disabled {
                send_user_error(sock, peer, USER_WILL_NOT_AUTH, client_session_id).await?;
                return Ok(());
            }

            if user.salt.is_empty() || user.salt.len() > 255 {
                send_user_error(sock, peer, USER_TRY_LATER, client_session_id).await?;
                return Ok(());
            }

            // Allocate a random session id (Zandronum uses a signed int).
            let session_id = loop {
                let mut rnd = [0u8; 4];
                rand::rng().fill_bytes(&mut rnd);
                let v = i32::from_le_bytes(rnd);
                if v > 0 {
                    let map = sessions.lock().await;
                    if !map.contains_key(&v) {
                        break v;
                    }
                }
            };

            let secrets = UserSecrets {
                username: user.username.clone(),
                salt: user.salt.clone(),
                verifier: user.verifier.clone(),
            };

            // Store a placeholder session; SRP state will be created when we receive A.
            {
                let mut map = sessions.lock().await;
                map.insert(
                    session_id,
                    Session {
                        created_at_unix: now,
                        client_session_id,
                        srp: SrpServerSession::new(secrets)?,
                    },
                );
            }

            // Respond: AUTH_SERVER_NEGOTIATE
            let mut w = Writer::new();
            w.u32_le(AUTH_SERVER_NEGOTIATE);
            w.u8(AUTH_PROTOCOL_VERSION);
            w.u32_le(client_session_id);
            w.i32_le(session_id);
            w.u8(user.salt.len() as u8);
            w.bytes(&user.salt);
            w.cstring(&user.username);

            sock.send_to(&w.finish(), peer)
                .await
                .context("udp send negotiate")?;
            Ok(())
        }

        SERVER_AUTH_SRP_STEP_ONE => {
            let session_id = r.read_i32_le().context("read session id")?;
            let len_a = r.read_u16_le().context("read lenA")? as usize;
            let a = r.read_bytes(len_a).context("read A")?;

            let mut map = sessions.lock().await;
            let Some(sess) = map.get_mut(&session_id) else {
                drop(map);
                send_session_error(sock, peer, SESSION_NO_EXIST, session_id).await?;
                return Ok(());
            };

            match sess.srp.step1_process_a(&a) {
                Ok(b_bytes) => {
                    let mut w = Writer::new();
                    w.u32_le(AUTH_SERVER_SRP_STEP_TWO);
                    w.i32_le(session_id);
                    let b_bytes: Vec<u8> = b_bytes;
                    w.u16_le(b_bytes.len() as u16);
                    w.bytes(&b_bytes);
                    sock.send_to(&w.finish(), peer)
                        .await
                        .context("udp send step2")?;
                    Ok(())
                }
                Err(_) => {
                    drop(map);
                    send_session_error(sock, peer, SESSION_VERIFIER_UNSAFE, session_id).await?;
                    Ok(())
                }
            }
        }

        SERVER_AUTH_SRP_STEP_THREE => {
            let session_id = r.read_i32_le().context("read session id")?;
            let len_m = r.read_u16_le().context("read lenM")? as usize;
            let m1 = r.read_bytes(len_m).context("read M")?;

            let mut map = sessions.lock().await;
            let Some(sess) = map.remove(&session_id) else {
                drop(map);
                send_session_error(sock, peer, SESSION_NO_EXIST, session_id).await?;
                return Ok(());
            };

            match sess.srp.step3_verify_m1_and_get_hamk(&m1) {
                Ok(hamk) => {
                    let mut w = Writer::new();
                    w.u32_le(AUTH_SERVER_SRP_STEP_FOUR);
                    w.i32_le(session_id);
                    let hamk: Vec<u8> = hamk;
                    w.u16_le(hamk.len() as u16);
                    w.bytes(&hamk);
                    sock.send_to(&w.finish(), peer)
                        .await
                        .context("udp send step4")?;
                    Ok(())
                }
                Err(_) => {
                    drop(map);
                    send_session_error(sock, peer, SESSION_AUTH_FAILED, session_id).await?;
                    Ok(())
                }
            }
        }

        _ => Ok(()),
    }
}

async fn send_user_error(
    sock: &UdpSocket,
    peer: std::net::SocketAddr,
    code: u8,
    client_session_id: u32,
) -> Result<()> {
    let mut w = Writer::new();
    w.u32_le(AUTH_SERVER_USER_ERROR);
    w.u8(code);
    w.u32_le(client_session_id);
    sock.send_to(&w.finish(), peer)
        .await
        .context("udp send user error")?;
    Ok(())
}

async fn send_session_error(
    sock: &UdpSocket,
    peer: std::net::SocketAddr,
    code: u8,
    session_id: i32,
) -> Result<()> {
    let mut w = Writer::new();
    w.u32_le(AUTH_SERVER_SESSION_ERROR);
    w.u8(code);
    w.i32_le(session_id);
    sock.send_to(&w.finish(), peer)
        .await
        .context("udp send session error")?;
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.pos + 1 > self.buf.len() {
            bail!("eof")
        }
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i32_le(&mut self) -> Result<i32> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        if self.pos + n > self.buf.len() {
            bail!("eof")
        }
        let out = self.buf[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(out)
    }

    fn read_cstring(&mut self) -> Result<String> {
        let start = self.pos;
        while self.pos < self.buf.len() {
            if self.buf[self.pos] == 0 {
                let bytes = &self.buf[start..self.pos];
                self.pos += 1;
                let s = String::from_utf8_lossy(bytes).to_string();
                return Ok(s);
            }
            self.pos += 1;
        }
        bail!("unterminated string")
    }
}

struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
        }
    }

    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    fn u16_le(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn u32_le(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn i32_le(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }

    fn cstring(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }

    fn finish(self) -> Vec<u8> {
        self.buf
    }
}
