use crate::{
    common::{AppState, UserId},
    jwt::validate_keycloak_access_token,
    keycloak::Keycloak,
    payload::WebsockAuthPayload,
};
use aes_gcm::{KeyInit, aead::Aead};
use anyhow::{Context, Error, Result, anyhow};
use axum::{Json, extract::State, response::IntoResponse};
use base64::Engine;
use deadpool_redis::Pool;
use dorch_common::response;
use owo_colors::OwoColorize;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthHandshake {
    pub user_id: Uuid,
    pub key: Vec<u8>,
    pub nonce: Vec<u8>,
    pub device_id: Uuid,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BeginHandshakeRequest {
    key: String,
    nonce: String,
    device_id: Uuid,
}

fn handshake_key(conn_id: Uuid) -> String {
    format!("wsa:{}", conn_id)
}

pub async fn begin_handshake(
    State(state): State<AppState>,
    UserId(user_id): UserId,
    Json(req): Json<BeginHandshakeRequest>,
) -> impl IntoResponse {
    let engine = base64::engine::general_purpose::STANDARD;
    let key = match engine.decode(&req.key) {
        Ok(v) => v,
        Err(e) => {
            return response::bad_request(Error::from(e).context("Invalid key"));
        }
    };
    let nonce = match engine.decode(&req.nonce) {
        Ok(v) => v,
        Err(e) => {
            return response::bad_request(Error::from(e).context("Invalid nonce"));
        }
    };
    println!(
        "{}{}{}{}",
        "🔐 Starting handshake • user_id=".cyan(),
        user_id.cyan().dimmed(),
        " • device_id=".cyan(),
        req.device_id.cyan().dimmed()
    );
    let handshake = AuthHandshake {
        user_id,
        key,
        nonce,
        device_id: req.device_id,
    };
    let conn_id = Uuid::new_v4();
    let value = serde_json::to_vec(&handshake).expect("Failed to encode handshake");
    let set_result = match state.redis.get().await {
        Ok(mut conn) => conn.set(handshake_key(conn_id), value).await,
        Err(e) => {
            return response::error(Error::from(e).context("Failed to get Redis connection"));
        }
    };
    match set_result {
        Ok(()) => (axum::http::StatusCode::OK, conn_id.to_string()).into_response(),
        Err(e) => response::error(Error::from(e).context("Failed to store handshake")),
    }
}

async fn retrieve_handshake(redis: &Pool, conn_id: Uuid) -> Result<AuthHandshake> {
    let script = r#"local val = redis.call("GET", KEYS[1])
if val then
    redis.call("DEL", KEYS[1])
    return val
else
    return nil
end"#;
    let script = redis::Script::new(script);
    let (_, stored): ((), Option<Vec<u8>>) = redis::pipe()
        .load_script(&script)
        .invoke_script(&script.key(handshake_key(conn_id)))
        .query_async(
            &mut redis
                .get()
                .await
                .context("Failed to get Redis connection")?,
        )
        .await
        .context("Failed to run Lua script")?;
    match stored {
        Some(v) => Ok(serde_json::from_slice(&v).context("Failed to decode stored handshake")?),
        None => Err(anyhow::anyhow!("handshake not found")),
    }
}

pub async fn auth_conn(
    redis: &Pool,
    kc: &Keycloak,
    payload: WebsockAuthPayload,
) -> Result<AuthHandshake> {
    let handshake = retrieve_handshake(redis, payload.conn_id)
        .await
        .context("Failed to retrieve handshake")?;
    let cipher =
        aes_gcm::Aes256Gcm::new_from_slice(&handshake.key).context("Failed to create cipher")?;
    let nonce = aes_gcm::Nonce::<aes_gcm::aead::consts::U12>::from_slice(&handshake.nonce);
    let encrypted_access_token = base64::engine::general_purpose::URL_SAFE
        .decode(&payload.base64_encrypted_access_token)
        .context("Failed to decode base64 encrypted access token")?;
    let decrypted_access_token = cipher
        .decrypt(nonce, encrypted_access_token.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to decrypt access token: {:?}", e))?;
    let access_token = String::from_utf8(decrypted_access_token)
        .context("Failed to decode decrypted access token")?;
    let issuer = format!(
        "{}/realms/{}",
        kc.args.endpoint.trim_end_matches('/'),
        kc.args.realm
    );
    let jwks_url = Url::parse(&format!("{}/protocol/openid-connect/certs", issuer))?;
    let claims = validate_keycloak_access_token(
        &access_token,
        &issuer,
        &kc.args.client_id, // expected aud = "synapse"
        &jwks_url,
    )
    .await
    .context("Failed to validate access token")?;
    let user_id = claims
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("JWT missing sub"))?
        .to_string();
    if user_id != handshake.user_id.to_string() {
        return Err(anyhow!(
            "user_id mismatch: expected {}, got {}",
            handshake.user_id,
            user_id
        ));
    }
    if let Some(c) = decode_jwt_claims_unverified(&access_token) {
        println!(
            "{}\n  iss: {}\n  aud: {}\n  azp: {}\n  typ: {}\n  exp: {}\n  iat: {} \n  (ws token claims)",
            "🔑 Decrypted access token claims:".green(),
            c.get("iss").and_then(|v| v.as_str()).unwrap_or("<none>"),
            c.get("aud").unwrap_or(&serde_json::Value::Null),
            c.get("azp").and_then(|v| v.as_str()).unwrap_or("<none>"),
            c.get("typ").and_then(|v| v.as_str()).unwrap_or("<none>"),
            c.get("exp").and_then(|v| v.as_i64()).unwrap_or(-1),
            c.get("iat").and_then(|v| v.as_i64()).unwrap_or(-1),
        );
    } else {
        println!("ws token is not a 3-part JWT (are you passing refresh_token?)");
    }

    Ok(handshake)
}
fn decode_jwt_claims_unverified(token: &str) -> Option<serde_json::Value> {
    let mid = token.split('.').nth(1)?;
    let mid = mid.trim();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(mid)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}
