use dorch_common::{Pagination, types::GameInfo};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct NewGameRequest {
    #[serde(default)]
    pub creator_id: Uuid,

    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,

    pub user_ids: Vec<Uuid>,

    pub iwad: String,

    #[serde(default)]
    pub private: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<String>,

    #[serde(default)]
    pub use_doom1_assets: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<i32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct WadReference {
    pub name: String,
    pub id: Uuid,
}

#[derive(Serialize)]
pub struct NewGameResponse {
    pub game_id: Uuid,
}

#[derive(Serialize)]
pub struct JoinGameResponse {
    pub game_id: Uuid,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Default)]
pub struct SearchGamesRequest {
    #[serde(rename = "q")]
    pub query: String,

    #[serde(flatten)]
    pub pagination: Pagination,

    #[serde(rename = "d", default)]
    pub sort_desc: Option<bool>,
}

#[derive(Serialize, Default, Clone)]
pub struct GameSummary {
    pub game_id: Uuid,

    /// Mirrors the Kubernetes Game resource's `status.phase`.
    ///
    /// Examples: "Pending", "Starting", "Active", "Terminating", "Error".
    pub status: String,

    pub iwad: Uuid,

    #[serde(default, skip_serializing_if = "Uuid::is_nil")]
    pub creator_id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<Uuid>>,

    /// A partial view of the Game resource's spec. Useful while the server is still
    /// provisioning and Redis game info has not yet been populated.
    pub spec: GameSpecSummary,

    pub info: Option<GameInfo>,
}

#[derive(Serialize, Default, Clone)]
pub struct GameSpecSummary {
    pub name: String,

    pub max_players: i32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<String>,

    #[serde(default)]
    pub private: bool,
}

#[derive(Serialize, Default, Clone)]
pub struct JumbotronItem {
    pub game_id: Uuid,
    pub hls: String,
    pub rtc: String,
    pub thumbnail: String,
    pub name: String,
    pub player_count: i32,
    pub max_players: i32,
    pub monster_kill_count: i32,
    pub monster_total: i32,
}

#[derive(Serialize, Default, Clone)]
pub struct ListJumbotronStreams {
    pub items: Vec<JumbotronItem>,
}

#[derive(Serialize, Default, Clone)]
pub struct ListGamesResponse {
    pub games: Vec<GameSummary>,
}

#[derive(Serialize)]
pub struct HomeResponse {
    pub games: ListGamesResponse,
    pub jumbotron: ListJumbotronStreams,
}

#[derive(Deserialize, Default)]
pub struct UpdateGameRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
