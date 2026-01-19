use anyhow::{Result, bail};
use num_bigint::BigUint;
use num_traits::Zero;
use rand::RngCore;
use sha2::{Digest, Sha256};

// Minimal SRP-6a (RFC 5054) server side for the 2048-bit group with SHA-256.
//
// This exists because Zandronum uses libsrp with SRP_SHA256 + SRP_NG_2048.
// The existing `srp6` crate in this repo is configured for SHA-512 by default,
// so it won't interoperate with the Zandronum client.
//
// We only implement what Zandronum needs:
// - Step 1: receive A, compute B and keep server state
// - Step 3: receive M1, verify, return HAMK

// RFC 5054 2048-bit group (N,g)
const N_HEX: &str = "AC6BDB41324A9A9BF166DE5E1389582FAF72B6651987EE07FC3192943DB56050A37329CBB4A099ED8193E0757767A13DD52312AB4B03310DCD7F48A9DA04FD50E8083969EDB767B0CF6095179A163AB3661A05FBD5FAAAE82918A9962F0B93B855F97993EC975EEAA80D740ADBF4FF747359D041D5C33EA71D281E446B14773BCA97B43A23FB801676BD207A436C6481F1D2B9078717461A5B9D32E688F87748544523B524B0D57D5EA77A2775D2ECFA032CFBDBF52FB3786160279004E57AE6AF874E7303CE53299CCC041C7BC308D82A5698F3A8D0C38271AE35F8E9DBFBB694B5C803D89F7AE435DE236D525F54759B65E372FCD68EF20FA7111F9E4AFF73";
const G_DEC: u32 = 2;

fn from_be(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

fn to_be(v: &BigUint) -> Vec<u8> {
    v.to_bytes_be()
}

fn pad_to(bytes: &[u8], len: usize) -> Vec<u8> {
    if bytes.len() >= len {
        return bytes.to_vec();
    }
    let mut out = vec![0u8; len - bytes.len()];
    out.extend_from_slice(bytes);
    out
}

fn n() -> BigUint {
    BigUint::parse_bytes(N_HEX.as_bytes(), 16).expect("N parse")
}

fn g() -> BigUint {
    BigUint::from(G_DEC)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

fn h_bn(bytes: &[u8]) -> BigUint {
    from_be(&sha256(bytes))
}

fn hcat(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

fn xor(a: &[u8], b: &[u8]) -> Vec<u8> {
    a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
}

fn modexp(base: &BigUint, exp: &BigUint, modu: &BigUint) -> BigUint {
    base.modpow(exp, modu)
}

fn compute_k(n_bytes: &[u8], g_bytes: &[u8]) -> BigUint {
    // k = H(N || PAD(g))
    let n_pad = n_bytes;
    let g_pad = pad_to(g_bytes, n_bytes.len());
    h_bn(&[n_pad, &g_pad].concat())
}

fn compute_u(a_bytes: &[u8], b_bytes: &[u8]) -> BigUint {
    // u = H(PAD(A) || PAD(B))
    h_bn(&[a_bytes, b_bytes].concat())
}

fn compute_m1(
    username: &str,
    salt: &[u8],
    a_bytes: &[u8],
    b_bytes: &[u8],
    k_bytes: &[u8],
    n_bytes: &[u8],
    g_bytes: &[u8],
) -> [u8; 32] {
    // M1 = H( H(N) XOR H(g) || H(I) || s || A || B || K )
    let hn = sha256(n_bytes);
    let hg = sha256(g_bytes);
    let hng_xor_hg = xor(&hn, &hg);

    let hi = sha256(username.as_bytes());
    hcat(&[&hng_xor_hg, &hi, salt, a_bytes, b_bytes, k_bytes])
}

fn compute_hamk(a_bytes: &[u8], m1_bytes: &[u8], k_bytes: &[u8]) -> [u8; 32] {
    // HAMK = H(A || M1 || K)
    hcat(&[a_bytes, m1_bytes, k_bytes])
}

#[derive(Clone, Debug)]
pub struct UserSecrets {
    pub username: String,
    pub salt: Vec<u8>,
    pub verifier: Vec<u8>,
}

#[derive(Debug)]
pub struct SrpServerSession {
    secrets: UserSecrets,
    // ephemeral values
    b: BigUint,
    a: Option<BigUint>,
    b_pub: Option<BigUint>,
}

/// Generate a fresh SRP salt + verifier pair for the given username/password.
///
/// Zandronum uses libsrp with SRP_SHA256 + SRP_NG_2048, which matches the
/// RFC 5054 formula:
/// - x = H(s || H(I ":" P))
/// - v = g^x mod N
pub fn generate_user_secrets(username: &str, password: &str) -> Result<UserSecrets> {
    if username.is_empty() {
        bail!("empty username")
    }
    if password.is_empty() {
        bail!("empty password")
    }

    // Keep it small (Zandronum negotiate encodes salt length in a u8).
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    generate_user_secrets_with_salt(username, password, &salt)
}

pub fn generate_user_secrets_with_salt(
    username: &str,
    password: &str,
    salt: &[u8],
) -> Result<UserSecrets> {
    if username.is_empty() {
        bail!("empty username")
    }
    if password.is_empty() {
        bail!("empty password")
    }
    if salt.is_empty() {
        bail!("empty salt")
    }
    if salt.len() > 255 {
        bail!("salt too long")
    }

    let n_bn = n();
    let g_bn = g();

    let up = format!("{username}:{password}");
    let up_hash = sha256(up.as_bytes());
    let x_hash = sha256(&[salt, &up_hash].concat());
    let x_bn = from_be(&x_hash);
    if x_bn.is_zero() {
        bail!("x is zero")
    }

    let v_bn = modexp(&g_bn, &x_bn, &n_bn);
    let verifier = pad_to(&to_be(&v_bn), 256);

    Ok(UserSecrets {
        username: username.to_string(),
        salt: salt.to_vec(),
        verifier,
    })
}

impl SrpServerSession {
    pub fn new(secrets: UserSecrets) -> Result<Self> {
        if secrets.username.is_empty() {
            bail!("empty username")
        }
        if secrets.salt.is_empty() {
            bail!("empty salt")
        }
        if secrets.verifier.is_empty() {
            bail!("empty verifier")
        }
        // Generate a random b (256 bits is fine; group is 2048-bit).
        let mut rnd = [0u8; 32];
        rand::rng().fill_bytes(&mut rnd);
        let b = from_be(&rnd);

        Ok(Self {
            secrets,
            b,
            a: None,
            b_pub: None,
        })
    }

    pub fn step1_process_a(&mut self, a_bytes: &[u8]) -> Result<Vec<u8>> {
        let n_bn = n();
        let g_bn = g();
        let v_bn = from_be(&self.secrets.verifier);

        let a_bn = from_be(a_bytes);
        if a_bn.is_zero() {
            bail!("A is zero")
        }
        if (&a_bn % &n_bn).is_zero() {
            bail!("A mod N is zero")
        }

        let n_bytes = to_be(&n_bn);
        let g_bytes = to_be(&g_bn);
        let k_bn = compute_k(&n_bytes, &g_bytes);

        // B = (k*v + g^b) % N
        let gb = modexp(&g_bn, &self.b, &n_bn);
        let kv = (&k_bn * &v_bn) % &n_bn;
        let b_pub = (kv + gb) % &n_bn;

        let b_out = pad_to(&to_be(&b_pub), 256); // 2048-bit

        self.a = Some(a_bn);
        self.b_pub = Some(b_pub);

        Ok(b_out)
    }

    pub fn step3_verify_m1_and_get_hamk(&self, m1_bytes: &[u8]) -> Result<Vec<u8>> {
        let Some(a_bn) = self.a.as_ref() else {
            bail!("missing A")
        };
        let Some(b_pub) = self.b_pub.as_ref() else {
            bail!("missing B")
        };

        let n_bn = n();
        let g_bn = g();
        let v_bn = from_be(&self.secrets.verifier);

        let n_bytes = to_be(&n_bn);
        let g_bytes = to_be(&g_bn);

        let a_bytes = pad_to(&to_be(a_bn), 256);
        let b_bytes = pad_to(&to_be(b_pub), 256);

        let u_bn = compute_u(&a_bytes, &b_bytes);
        if u_bn.is_zero() {
            bail!("u is zero")
        }

        // S = (A * v^u) ^ b % N
        let vu = modexp(&v_bn, &u_bn, &n_bn);
        let avu = (from_be(&a_bytes) * vu) % &n_bn;
        let s_bn = modexp(&avu, &self.b, &n_bn);

        // K = H(S)
        let s_bytes = pad_to(&to_be(&s_bn), 256);
        let k_bytes = sha256(&s_bytes);

        let expected_m1 = compute_m1(
            &self.secrets.username,
            &self.secrets.salt,
            &a_bytes,
            &b_bytes,
            &k_bytes,
            &n_bytes,
            &g_bytes,
        );

        if m1_bytes.len() != expected_m1.len() {
            bail!("invalid M1 length")
        }
        if m1_bytes != expected_m1 {
            bail!("M1 mismatch")
        }

        let hamk = compute_hamk(&a_bytes, &expected_m1, &k_bytes);
        Ok(hamk.to_vec())
    }
}
