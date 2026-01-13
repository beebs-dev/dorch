use anyhow::{Context, Result};
use sha2::Sha256;

/// This module is a thin compatibility layer so you can swap SRP implementations
/// (or adjust to srp6 crate API differences) without touching the server logic.
///
/// You MUST implement:
/// - ServerSession::start(username, verifier, salt, A_bytes) -> (ServerSession, B_bytes)
/// - ServerSession::verify(m1_bytes) -> m2_bytes
///
/// Everything else in the auth server depends only on these functions.

pub struct ServerSession {
    inner: Inner,
}

// --- Choose ONE implementation block below and delete the other. ---

// ===============================================================
// Implementation A: srp6 crate (adjust type names to match your version)
// ===============================================================

type Inner = Srp6Inner;

struct Srp6Inner {
    // replace these fields with whatever the srp6 crate uses for a server session
    // Example placeholders:
    // server: srp6::SrpServer<srp6::groups::G_2048, Sha256>,
    _private: (),
}

impl ServerSession {
    pub fn start(
        _username: &str,
        verifier_bytes: &[u8],
        salt: &[u8],
        a_bytes: &[u8],
    ) -> Result<(ServerSession, Vec<u8>)> {
        // TODO: Adapt this to your srp6 crate version.
        //
        // The general idea you need:
        // 1) Parse verifier_bytes into the verifier type expected by the SRP server.
        // 2) Initialize SRP server session for group 2048 and hash Sha256.
        // 3) Produce server public ephemeral B as bytes.
        //
        // PSEUDOCODE (you must map to real types/functions):
        //
        // let verifier = srp6::SrpAuthVerifier::from_slice(verifier_bytes)?;
        // let mut rng = rand::thread_rng();
        // let mut srv = srp6::SrpServer::<srp6::Srp6_2048, Sha256>::new(verifier);
        // let b_pub = srv.start_authentication(a_bytes, salt.to_vec(), &mut rng)?;
        // Ok((ServerSession { inner: Srp6Inner { server: srv } }, b_pub.to_vec()))
        //
        // For now, error clearly so you know what to edit.
        let _ = (verifier_bytes, salt, a_bytes);
        anyhow::bail!(
            "SRP adapter not wired: implement ServerSession::start() for your srp6 crate version"
        );
    }

    pub fn verify(self, m1_bytes: &[u8]) -> Result<Vec<u8>> {
        // TODO: Use the stored server session to verify the client's proof M1 and produce M2.
        //
        // PSEUDOCODE:
        // let m2 = self.inner.server.verify_client(m1_bytes)?;
        // Ok(m2.to_vec())
        //
        let _ = m1_bytes;
        anyhow::bail!(
            "SRP adapter not wired: implement ServerSession::verify() for your srp6 crate version"
        );
    }
}
