use anyhow::{Context, Result};
use dorch_common::types::GameInfo;
use redis::AsyncCommands;
use uuid::Uuid;

#[derive(Clone)]
pub struct GameInfoStore {
    pool: deadpool_redis::Pool,
}

impl GameInfoStore {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn get_game_info(&self, game_id: Uuid) -> Result<Option<GameInfo>> {
        let key = key_game_info(game_id);
        let info_json: Option<String> = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?
            .get(key)
            .await
            .context("Failed to get value from Redis")?;
        let info: Option<GameInfo> = match info_json {
            Some(json) => {
                Some(serde_json::from_str(&json).context("Failed to deserialize GameInfo")?)
            }
            None => None,
        };
        Ok(info)
    }

    pub async fn set_game_info(&self, game_id: Uuid, info: &GameInfo) -> Result<()> {
        let key = key_game_info(game_id);
        let info_json = serde_json::to_string(info).context("Failed to serialize GameInfo")?;
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let _: () = redis::pipe()
            .cmd("SET")
            .arg(key)
            .arg(&info_json)
            .cmd("PUBLISH")
            .arg(dorch_common::MASTER_TOPIC)
            .arg(info_json)
            .query_async(&mut conn)
            .await
            .context("Failed to set game info in Redis")?;
        Ok(())
    }
}

fn key_game_info(game_id: Uuid) -> String {
    format!("game_info:{}", game_id)
}

struct GameInfoEvent {
    game_id: Uuid,
    info: GameInfo,
}
