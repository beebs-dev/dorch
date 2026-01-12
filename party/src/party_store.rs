use anyhow::{Context, Result, bail};
use deadpool_redis::Connection;
use dorch_common::types::Party;
use owo_colors::OwoColorize;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const EXPIRE_SECONDS: i64 = 60 * 60 * 24 * 7; // 7 days

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Invite {
    pub recipient_id: Uuid,

    #[serde(default, skip_serializing_if = "Uuid::is_nil")]
    pub sender_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AcceptInvite {
    pub user_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Kick {
    #[serde(default, skip_serializing_if = "Uuid::is_nil")]
    pub kicker_id: Uuid,

    pub user_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LeaveParty {
    pub user_id: Uuid,
}

mod scripts {
    pub const GET_PARTY: &str = include_str!("get_party.lua");
    pub const REMOVE_MEMBER: &str = include_str!("remove_member.lua");
}

#[derive(Clone)]
pub struct PartyInfoStore {
    pool: deadpool_redis::Pool,
}

impl PartyInfoStore {
    pub fn new(pool: deadpool_redis::Pool) -> Self {
        Self { pool }
    }

    pub async fn accept_invite(&self, party_id: Uuid, user_id: Uuid) -> Result<()> {
        let invite_key = keys::user::invites(user_id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let invite: Option<String> = conn
            .hmget(&invite_key, party_id.to_string())
            .await
            .context("Failed to check for invite in Redis")?;
        if invite.is_none() {
            bail!("No invite found for user to join party");
        }
        self.add_member_inner(party_id, user_id, &mut conn)
            .await
            .context("Failed to add member to party")?;
        let _: () = conn
            .hdel(&invite_key, party_id.to_string())
            .await
            .context("Failed to remove invite from Redis")?;
        Ok(())
    }

    pub async fn list_members(&self, party_id: Uuid) -> Result<Vec<Uuid>> {
        let members_key = keys::party::members(party_id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let members: Vec<String> = conn
            .smembers(&members_key)
            .await
            .context("Failed to get party members from Redis")?;
        let member_uuids: Vec<Uuid> = members
            .into_iter()
            .filter_map(|s| Uuid::parse_str(&s).ok())
            .collect();
        Ok(member_uuids)
    }

    pub async fn create_invite(
        &self,
        party_id: Uuid,
        recipient_id: Uuid,
        sender_id: Uuid,
    ) -> Result<()> {
        let key = keys::user::invites(recipient_id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let _: () = redis::pipe()
            .hset(&key, party_id.to_string(), sender_id.to_string())
            .ignore()
            .expire(&key, EXPIRE_SECONDS)
            .ignore()
            .query_async(&mut conn)
            .await
            .context("Failed to send invite in Redis")?;
        Ok(())
    }

    pub async fn create_party(
        &self,
        party_id: Uuid,
        leader_id: Uuid,
        name: Option<String>,
    ) -> Result<()> {
        let info_key = keys::party::info(party_id);
        let members_key = keys::party::members(party_id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let mut pipe = redis::pipe();
        pipe.atomic()
            .hset(&info_key, "leader", leader_id.to_string());
        if let Some(name) = name {
            pipe.hset(&info_key, "name", name);
        } else {
            pipe.hdel(&info_key, "name");
        }
        pipe.expire(&info_key, EXPIRE_SECONDS)
            .sadd(&members_key, leader_id.to_string())
            .expire(&members_key, EXPIRE_SECONDS);
        let _: () = pipe
            .query_async(&mut conn)
            .await
            .context("Failed to create party in Redis")?;
        if let Err(e) = conn
            .set::<_, _, ()>(keys::user::party(leader_id), party_id.to_string())
            .await
            .context("Failed to set user party in Redis")
        {
            // best-effort rollback of party keys (same slot, so safe)
            let mut pipe = redis::pipe();
            pipe.del(&info_key).ignore().del(&members_key).ignore();
            let _: std::result::Result<(), _> = pipe.query_async(&mut conn).await;
            return Err(e.context("Failed to assign leader to party; rolled back party creation"));
        }
        Ok(())
    }

    pub async fn update_info(&self, party: &Party) -> Result<()> {
        let key = keys::party::info(party.id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let mut pipe = redis::pipe();
        pipe.atomic()
            .hset(&key, "leader", party.leader_id.to_string())
            .ignore();
        if let Some(name) = &party.name {
            pipe.hset(&key, "name", name).ignore();
        } else {
            pipe.hdel(&key, "name").ignore();
        }
        pipe.expire(&key, EXPIRE_SECONDS).ignore();
        if let Some(members) = &party.members {
            let members_key = keys::party::members(party.id);
            pipe.del(&members_key).ignore();
            for member in members {
                pipe.sadd(&members_key, member.to_string()).ignore();
            }
            pipe.expire(&members_key, EXPIRE_SECONDS).ignore();
        }
        let _: () = pipe
            .query_async(&mut conn)
            .await
            .context("Failed to update party info in Redis")?;
        Ok(())
    }

    pub async fn user_is_in_party(&self, party_id: Uuid, user_id: Uuid) -> Result<bool> {
        let members_key = keys::party::members(party_id);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let is_member: bool = conn
            .sismember(&members_key, user_id.to_string())
            .await
            .context("Failed to check if user is member of party in Redis")?;
        Ok(is_member)
    }

    pub async fn get_party(&self, party_id: Uuid) -> Result<Option<Party>> {
        let info_key = keys::party::info(party_id);
        let members_key = keys::party::members(party_id);
        let script = redis::Script::new(scripts::GET_PARTY);
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        let raw: Option<Vec<String>> = script
            .key(&info_key)
            .key(&members_key)
            .arg(EXPIRE_SECONDS)
            .invoke_async(&mut conn)
            .await
            .context("Failed to invoke Redis script to get party info")?;
        // raw == None means "not found"
        let Some(mut raw) = raw else {
            return Ok(None);
        };
        let leader = raw.remove(0);
        let name = raw.remove(0);
        let name = if name.is_empty() { None } else { Some(name) };
        let leader = Uuid::parse_str(&leader).context("Failed to parse leader UUID")?;
        let members: Vec<Uuid> = raw
            .into_iter()
            .filter_map(|s| Uuid::parse_str(&s).ok())
            .collect();
        Ok(Some(Party {
            id: party_id,
            name,
            leader_id: leader,
            members: if members.is_empty() {
                None
            } else {
                Some(members)
            },
        }))
    }

    pub async fn add_member_inner(
        &self,
        party_id: Uuid,
        user_id: Uuid,
        conn: &mut Connection,
    ) -> Result<()> {
        let user_party_key = keys::user::party(user_id);
        let old_party_id: Option<Uuid> = conn
            .get::<_, Option<String>>(&user_party_key)
            .await
            .context("Failed to get old_party_id from Redis")?
            .map(|s: String| Uuid::parse_str(&s))
            .transpose()
            .inspect_err(|e| {
                eprintln!(
                    "{}{}{}{}",
                    "⚠️ Failed to parse old_party_id UUID for user ".yellow(),
                    user_id.yellow().dimmed(),
                    ": ".yellow(),
                    format!("{:?}", e).yellow().dimmed()
                );
            })
            .unwrap_or_default();
        if let Some(old_party_id) = old_party_id {
            self.remove_member_inner(old_party_id, user_id, conn)
                .await
                .context("Failed to remove user from old party before adding to new party")?;
        }
        let members_key = keys::party::members(party_id);
        let _: () = redis::pipe()
            .sadd(&members_key, user_id.to_string())
            .expire(&members_key, EXPIRE_SECONDS)
            .query_async(conn)
            .await
            .context("Failed to add member to Redis set")?;
        let _: () = conn
            .set_ex(&user_party_key, party_id.to_string(), EXPIRE_SECONDS as u64)
            .await
            .context("Failed to set user party in Redis")?;
        Ok(())
    }

    pub async fn add_member(&self, party_id: Uuid, user_id: Uuid) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        self.add_member_inner(party_id, user_id, &mut conn)
            .await
            .context("Failed to add member to party")
    }

    pub async fn remove_member(&self, party_id: Uuid, user_id: Uuid) -> Result<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .context("Failed to get Redis connection")?;
        self.remove_member_inner(party_id, user_id, &mut conn).await
    }

    pub async fn remove_member_inner(
        &self,
        party_id: Uuid,
        user_id: Uuid,
        conn: &mut deadpool_redis::Connection,
    ) -> Result<()> {
        let script = redis::Script::new(scripts::REMOVE_MEMBER);
        let remaining: i64 = script
            .key(keys::party::members(party_id))
            .key(keys::party::info(party_id))
            .arg(user_id.to_string())
            .invoke_async(conn)
            .await
            .context("Failed to invoke Redis script to remove member")?;
        if remaining == 0 {
            println!(
                "{}{}",
                "🗑️ Disposed of party ".cyan(),
                party_id.to_string().cyan().dimmed()
            );
        }
        Ok(())
    }
}

pub(crate) mod keys {
    pub mod user {
        use uuid::Uuid;

        pub fn invites(user_id: Uuid) -> String {
            format!("user_id:{{{}}}:invites", user_id)
        }

        pub fn party(user_id: Uuid) -> String {
            format!("user_id:{{{}}}:party", user_id)
        }
    }

    pub mod party {
        use uuid::Uuid;

        pub fn info(party_id: Uuid) -> String {
            format!("party:{{{}}}:info", party_id)
        }

        pub fn members(party_id: Uuid) -> String {
            format!("party:{{{}}}:members", party_id)
        }
    }
}
