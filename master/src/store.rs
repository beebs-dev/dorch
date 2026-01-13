use anyhow::{Context, Result};
use dorch_common::types::GameInfo;
use std::collections::HashMap;
use uuid::Uuid;

const TTL_SECONDS: i64 = 86400; // 1 day

mod scripts {
    pub const SET_GAME_INFO_FIELDS: &str = include_str!("set_game_info_field.lua");
}

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
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let hash: HashMap<String, String> = redis::pipe()
            .hgetall(&key)
            .expire(&key, TTL_SECONDS)
            .ignore()
            .query_async(&mut conn)
            .await?;
        if hash.is_empty() {
            return Ok(None);
        }
        let name = hash
            .get("name")
            .cloned()
            .context("Missing 'name' in game info hash")?;
        let current_map = hash
            .get("current_map")
            .cloned()
            .context("Missing 'current_map' in game info hash")?;
        let max_players: i32 = hash
            .get("max_players")
            .context("Missing 'max_players' in game info hash")?
            .parse()
            .context("Invalid 'max_players' in game info hash")?;
        let player_count: i32 = hash
            .get("player_count")
            .context("Missing 'player_count' in game info hash")?
            .parse()
            .context("Invalid 'player_count' in game info hash")?;
        let skill: i32 = hash
            .get("skill")
            .context("Missing 'skill' in game info hash")?
            .parse()
            .context("Invalid 'skill' in game info hash")?;
        let monster_kill_count: i32 = hash
            .get("monster_kill_count")
            .context("Missing 'monster_kill_count' in game info hash")?
            .parse()
            .context("Invalid 'monster_kill_count' in game info hash")?;
        let monster_count: i32 = hash
            .get("monster_count")
            .context("Missing 'monster_count' in game info hash")?
            .parse()
            .context("Invalid 'monster_count' in game info hash")?;
        Ok(Some(GameInfo {
            game_id,
            name,
            max_players,
            player_count,
            skill,
            current_map,
            monster_kill_count,
            monster_count,
        }))
    }

    pub async fn update_game_info<T>(&self, game_id: Uuid, values: &[(&str, T)]) -> Result<()>
    where
        T: redis::ToRedisArgs,
    {
        let key = key_game_info(game_id);
        let script = redis::Script::new(scripts::SET_GAME_INFO_FIELDS);
        // Build invocation: KEYS[1] = key, ARGV = field/value pairs..., channel, ttl
        let mut inv = script.key(key);
        let mut inv2 = &mut inv;
        for (field, value) in values {
            inv2 = inv2.arg(*field).arg(value);
        }
        inv2 = inv2.arg(dorch_common::MASTER_TOPIC).arg(TTL_SECONDS);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let _: () = inv2
            .invoke_async(&mut conn)
            .await
            .context("Failed to invoke Redis script SET_GAME_INFO_FIELDS")?;
        Ok(())
    }
}

fn key_game_info(game_id: Uuid) -> String {
    format!("game_info:{}", game_id)
}
