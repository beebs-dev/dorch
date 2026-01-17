use anyhow::{Context, Result, anyhow};
use dorch_common::types::GameInfo;
use std::collections::HashMap;
use uuid::Uuid;

const TTL_SECONDS: i64 = 86400; // 1 day

mod scripts {
    pub const SET_GAME_INFO_FIELDS: &str = include_str!("set_game_info_fields.lua");
}

#[derive(Clone)]
pub struct GameInfoStore {
    pool: deadpool_redis::Pool,
}

impl GameInfoStore {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn delete_game_info(&self, game_id: Uuid) -> Result<()> {
        let key = key_game_info(game_id.to_string().as_str());
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .context("Failed to delete game info from Redis")?;
        Ok(())
    }

    pub async fn get_game_info(&self, game_id: Uuid) -> Result<Option<GameInfo>> {
        let key = key_game_info(game_id.to_string().as_str());
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

        fn parse_bool_str(raw: &str, key: &str) -> Result<bool> {
            match raw {
                "1" | "true" | "TRUE" | "True" => Ok(true),
                "0" | "false" | "FALSE" | "False" => Ok(false),
                _ => Err(anyhow!("Invalid boolean value for '{}': {}", key, raw)),
            }
        }

        fn parse_bool(hash: &HashMap<String, String>, key: &str, default: bool) -> Result<bool> {
            match hash.get(key) {
                None => Ok(default),
                Some(raw) => parse_bool_str(raw, key),
            }
        }

        fn parse_opt_i32(hash: &HashMap<String, String>, key: &str) -> Result<Option<i32>> {
            match hash.get(key) {
                None => Ok(None),
                Some(raw) => {
                    Ok(Some(raw.parse().with_context(|| {
                        format!("Invalid '{}' in game info hash", key)
                    })?))
                }
            }
        }

        fn parse_opt_f32(hash: &HashMap<String, String>, key: &str) -> Result<Option<f32>> {
            match hash.get(key) {
                None => Ok(None),
                Some(raw) => {
                    Ok(Some(raw.parse().with_context(|| {
                        format!("Invalid '{}' in game info hash", key)
                    })?))
                }
            }
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

        let motd = hash.get("motd").cloned();
        let private = parse_bool(&hash, "private", false)?;

        // Defaults are chosen to be broadly compatible with typical server configs.
        let sv_cheats = parse_bool(&hash, "sv_cheats", false)?;
        let sv_allowchat = parse_bool(&hash, "sv_allowchat", true)?;
        let sv_allowvoicechat = parse_bool(&hash, "sv_allowvoicechat", true)?;
        let sv_fastmonsters = parse_bool(&hash, "sv_fastmonsters", false)?;
        let sv_monsters = parse_bool(&hash, "sv_monsters", true)?;
        let sv_nomonsters = parse_bool(&hash, "sv_nomonsters", false)?;
        let sv_itemsrespawn = parse_bool(&hash, "sv_itemsrespawn", false)?;
        let sv_itemrespawntime = parse_opt_i32(&hash, "sv_itemrespawntime")?;
        let sv_coop_damagefactor = parse_opt_f32(&hash, "sv_coop_damagefactor")?;
        let sv_nojump = parse_bool(&hash, "sv_nojump", false)?;
        let sv_nocrouch = parse_bool(&hash, "sv_nocrouch", false)?;
        let sv_nofreelook = parse_bool(&hash, "sv_nofreelook", false)?;
        let sv_respawnonexit = parse_bool(&hash, "sv_respawnonexit", false)?;
        let sv_timelimit = parse_opt_i32(&hash, "sv_timelimit")?;
        let sv_fraglimit = parse_opt_i32(&hash, "sv_fraglimit")?;
        let sv_scorelimit = parse_opt_i32(&hash, "sv_scorelimit")?;
        let sv_duellimit = parse_opt_i32(&hash, "sv_duellimit")?;
        let sv_roundlimit = parse_opt_i32(&hash, "sv_roundlimit")?;
        let sv_allowrun = parse_bool(&hash, "sv_allowrun", true)?;
        let sv_allowfreelook = parse_bool(&hash, "sv_allowfreelook", true)?;
        Ok(Some(GameInfo {
            private,
            name,
            max_players,
            player_count,
            skill,
            current_map,
            monster_kill_count,
            monster_count,
            motd,
            sv_cheats,
            sv_allowchat,
            sv_allowvoicechat,
            sv_fastmonsters,
            sv_monsters,
            sv_nomonsters,
            sv_itemsrespawn,
            sv_itemrespawntime,
            sv_coop_damagefactor,
            sv_nojump,
            sv_nocrouch,
            sv_nofreelook,
            sv_respawnonexit,
            sv_timelimit,
            sv_fraglimit,
            sv_scorelimit,
            sv_duellimit,
            sv_roundlimit,
            sv_allowrun,
            sv_allowfreelook,
        }))
    }

    pub async fn update_game_info<T>(&self, game_id: Uuid, values: &[(&str, T)]) -> Result<()>
    where
        T: redis::ToRedisArgs,
    {
        let game_id = game_id.to_string();
        let key = key_game_info(&game_id);
        let script = redis::Script::new(scripts::SET_GAME_INFO_FIELDS);
        // Build invocation: KEYS[1] = key, ARGV = field/value pairs..., game_id, channel, ttl
        let mut inv = script.key(key);
        let mut inv = &mut inv;
        for (field, value) in values {
            inv = inv.arg(*field).arg(value);
        }
        inv = inv
            .arg(game_id)
            .arg(dorch_common::MASTER_TOPIC)
            .arg(TTL_SECONDS);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let _: () = inv
            .invoke_async(&mut conn)
            .await
            .context("Failed to invoke Redis script SET_GAME_INFO_FIELDS")?;
        Ok(())
    }
}

fn key_game_info(game_id: &str) -> String {
    format!("gi:{}", game_id)
}
