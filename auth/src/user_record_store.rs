use std::collections::HashMap;

use crate::{
    keys::user_record_key,
    server::{UserRecord, UserRecordJson},
};
use anyhow::{Context, Result};

#[derive(Clone)]
pub struct UserRecordStore {
    pool: deadpool_redis::Pool,
}

impl UserRecordStore {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, username: &str) -> Result<Option<UserRecord>> {
        let mut conn = self.pool.get().await?;
        let key = user_record_key(username);
        let record_map: Option<HashMap<String, String>> = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("Failed to fetch user record for {}", username))?;
        if let Some(map) = record_map {
        } else {
            Ok(None)
        }
    }

    pub async fn set(&self, record: &UserRecord) -> Result<()> {
        let mut conn = self.pool.get().await?;
        let key = user_record_key(&record.username);
        Ok(())
    }
}
