use std::collections::HashMap;

use crate::{keys::user_record_key, server::UserRecord};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use dorch_auth::client::UserRecordJson;
use redis::AsyncCommands;

#[derive(Clone)]
pub struct UserRecordStore {
    pool: deadpool_redis::Pool,
}

impl UserRecordStore {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, username: &str) -> Result<Option<UserRecord>> {
        if username.is_empty() {
            bail!("username must not be empty")
        }

        let mut conn = self.pool.get().await.context("redis get conn")?;
        let key = user_record_key(username);

        // Backward compatible: allow a JSON blob at the key.
        if let Ok(Some(blob)) = conn.get::<_, Option<String>>(&key).await {
            let parsed: UserRecordJson = serde_json::from_str(&blob)
                .with_context(|| format!("parse user json for {username}"))?;
            let record = decode_user_record(parsed, username)?;
            return Ok(Some(record));
        }

        // Normal: Redis hash at the key.
        let map: HashMap<String, String> = conn
            .hgetall(&key)
            .await
            .with_context(|| format!("Failed to fetch user record for {username}"))?;

        if map.is_empty() {
            return Ok(None);
        }

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

        Ok(Some(decode_user_record(parsed, username)?))
    }

    pub async fn set(&self, record: &UserRecord) -> Result<()> {
        if record.username.is_empty() {
            bail!("record.username must not be empty")
        }
        if record.salt.is_empty() {
            bail!("record.salt must not be empty")
        }
        if record.verifier.is_empty() {
            bail!("record.verifier must not be empty")
        }
        let key = user_record_key(&record.username);
        let salt_b64 = B64.encode(&record.salt);
        let verifier_b64 = B64.encode(&record.verifier);
        let disabled = if record.disabled { "1" } else { "0" };
        let mut conn = self.pool.get().await.context("redis get conn")?;
        let _: () = redis::pipe()
            .del(&key)
            .hset_multiple(
                &key,
                &[
                    ("username", record.username.as_str()),
                    ("salt_b64", salt_b64.as_str()),
                    ("verifier_b64", verifier_b64.as_str()),
                    ("disabled", disabled),
                ],
            )
            .query_async(&mut conn)
            .await
            .with_context(|| format!("redis hset user record for {}", record.username))?;
        Ok(())
    }

    pub async fn set_json(&self, json: UserRecordJson) -> Result<()> {
        let record: UserRecord = json
            .try_into()
            .context("convert UserRecordJson to UserRecord")?;
        self.set(&record).await
    }

    pub async fn store_session_token(
        &self,
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

        let mut conn = self.pool.get().await.context("redis get conn")?;
        let key = session_token_key(token);
        conn.set_ex::<_, _, ()>(key, username, ttl_secs)
            .await
            .context("redis set_ex session token")?;
        Ok(())
    }
}

fn session_token_key(token: &str) -> String {
    format!("auth:token:{}", token)
}

fn decode_user_record(parsed: UserRecordJson, requested_username: &str) -> Result<UserRecord> {
    let username = if !parsed.username.is_empty() {
        parsed.username
    } else {
        requested_username.to_string()
    };

    if username.is_empty() {
        bail!("empty username in record")
    }

    let salt = B64.decode(parsed.salt_b64).context("decode salt")?;
    let verifier = B64.decode(parsed.verifier_b64).context("decode verifier")?;

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
