use serde::{Deserialize, Serialize};

/// Client -> Server
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Start SRP login: send username + A (client ephemeral public)
    Start {
        username: String,
        a_b64: String,
        /// Optional: client metadata (proxy can pass along)
        #[serde(default)]
        client_id: Option<String>,
    },

    /// Finish SRP login: send M1 (client proof)
    Proof { m1_b64: String },
}

/// Server -> Client
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// SRP challenge: send salt + B (server ephemeral public)
    Challenge {
        salt_b64: String,
        b_b64: String,

        /// Optional: server nonce for logging/correlation
        session_id: String,
    },

    /// Success: send M2 (server proof) and an auth token you mint
    Ok {
        m2_b64: String,
        token: String,
        expires_in_seconds: u64,
    },

    /// Failure
    Err { code: String, message: String },
}
