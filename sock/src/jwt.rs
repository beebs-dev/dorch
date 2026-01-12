use anyhow::{Context, Result, anyhow, bail};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use once_cell::sync::OnceCell;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use url::Url;

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: String,
    e: String,
}

#[derive(Clone)]
struct JwksCache {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
}

static JWKS_CACHE: OnceCell<tokio::sync::RwLock<Option<JwksCache>>> = OnceCell::new();

async fn jwks_decoding_key(jwks_url: &Url, kid: &str) -> Result<DecodingKey> {
    let lock = JWKS_CACHE.get_or_init(|| tokio::sync::RwLock::new(None));

    // 10 min cache
    {
        let g = lock.read().await;
        if let Some(c) = &*g
            && c.fetched_at.elapsed() < Duration::from_secs(600)
            && let Some(k) = c.keys.get(kid)
        {
            return Ok(k.clone());
        }
    }

    // refresh
    let set: JwkSet = reqwest::get(jwks_url.clone())
        .await
        .context("JWKS fetch failed")?
        .json()
        .await
        .context("JWKS parse failed")?;

    let mut map = HashMap::new();
    for k in set.keys {
        if k.kty == "RSA" {
            let dk = DecodingKey::from_rsa_components(&k.n, &k.e)
                .context("Failed to build RSA decoding key")?;
            map.insert(k.kid, dk);
        }
    }

    {
        let mut g = lock.write().await;
        *g = Some(JwksCache {
            keys: map,
            fetched_at: Instant::now(),
        });
        if let Some(c) = &*g
            && let Some(k) = c.keys.get(kid)
        {
            return Ok(k.clone());
        }
    }

    bail!("kid not found in JWKS")
}

fn aud_contains(aud: &Value, expected: &str) -> bool {
    match aud {
        Value::String(s) => s == expected,
        Value::Array(arr) => arr.iter().any(|v| v.as_str() == Some(expected)),
        _ => false,
    }
}

/// Validates a Keycloak access token (RS256) locally.
///
/// Returns the verified claims (as JSON) if valid.
pub async fn validate_keycloak_access_token(
    access_token: &str,
    issuer: &str,       // e.g. "https://keycloak.beebs.dev/realms/dorch"
    expected_aud: &str, // e.g. "dorch"
    jwks_url: &Url,     // e.g. https://.../protocol/openid-connect/certs
) -> Result<Value> {
    // Header: get kid
    let header = decode_header(access_token).context("JWT header decode failed")?;
    let kid = header.kid.context("JWT missing kid")?;

    // Key
    let dk = jwks_decoding_key(jwks_url, &kid).await?;

    // Validate signature + exp + iss. Handle aud manually (Keycloak uses string OR array).
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.validate_aud = false;
    validation.leeway = 30; // tolerate small skew

    let data = decode::<Value>(access_token, &dk, &validation)
        .context("JWT signature/claims validation failed")?;

    // aud check
    let aud = data
        .claims
        .get("aud")
        .ok_or_else(|| anyhow!("JWT missing aud"))?;
    if !aud_contains(aud, expected_aud) {
        return Err(anyhow!("JWT audience mismatch (expected {})", expected_aud));
    }

    // Optional: ensure typ == Bearer
    if data.claims.get("typ").and_then(|v| v.as_str()) != Some("Bearer") {
        return Err(anyhow!("JWT typ is not Bearer"));
    }

    Ok(data.claims)
}
