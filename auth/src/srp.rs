use anyhow::{Result, bail};

use srp6::prelude::*;

pub struct ServerSession {
    inner: Inner,
}

type Inner = Srp6Inner;

struct Srp6Inner {
    proof_verifier: HandshakeProofVerifier,
    a_pub: PublicKey,
}

impl ServerSession {
    pub fn start(
        username: &str,
        verifier_bytes: &[u8],
        salt: &[u8],
        a_bytes: &[u8],
    ) -> Result<(ServerSession, Vec<u8>)> {
        if username.is_empty() {
            bail!("SRP start: empty username");
        }
        if verifier_bytes.is_empty() {
            bail!("SRP start: empty verifier");
        }
        if salt.is_empty() {
            bail!("SRP start: empty salt");
        }
        if a_bytes.is_empty() {
            bail!("SRP start: empty A");
        }

        // srp6 expects these values as BigNumber-derived primitives.
        // Note: these are treated as big-endian integers; leading zeros are harmless.
        let user = UserSecrets {
            username: username.to_string(),
            salt: Salt::from_bytes_be(salt),
            verifier: PasswordVerifier::from_bytes_be(verifier_bytes),
        };

        // We use the RFC5054 2048-bit group defaults.
        // Hash function is SHA-512 (srp6 default feature `hash-sha512`).
        let srp = Srp6_2048::default();
        let (handshake, proof_verifier) = srp.start_handshake(&user);

        // Client already sent A in our wire protocol; keep it for the proof verification step.
        let a_pub = PublicKey::from_bytes_be(a_bytes);

        let b_pub = handshake.B.to_array_pad_zero::<{ Srp6_2048::KEY_LEN }>();

        Ok((
            ServerSession {
                inner: Srp6Inner {
                    proof_verifier,
                    a_pub,
                },
            },
            b_pub,
        ))
    }

    pub fn verify(self, m1_bytes: &[u8]) -> Result<Vec<u8>> {
        if m1_bytes.is_empty() {
            bail!("SRP verify: empty M1");
        }

        let proof = HandshakeProof::<{ Srp6_2048::KEY_LEN }, { Srp6_2048::SALT_LEN }> {
            A: self.inner.a_pub,
            M1: Proof::from_bytes_be(m1_bytes),
        };

        let (m2, _session_key) = self
            .inner
            .proof_verifier
            .verify_proof::<{ Srp6_2048::KEY_LEN }, { Srp6_2048::SALT_LEN }>(&proof)?;

        Ok(m2.to_array_pad_zero::<{ HASH_LENGTH }>())
    }
}
