use dorch_common::{Pagination, types::GameInfo};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Default)]
pub struct NewGameRequest {
    #[serde(default)]
    pub creator_id: Uuid,

    pub name: String,

    pub user_ids: Vec<Uuid>,

    pub iwad: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<i32>,
}

#[derive(Serialize)]
pub struct NewGameResponse {
    pub game_id: Uuid,
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

    pub iwad: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,

    #[serde(flatten)]
    pub info: Option<GameInfo>,
}

#[derive(Serialize, Default, Clone)]
pub struct ListGamesResponse {
    pub games: Vec<GameSummary>,
}

#[derive(Deserialize, Default)]
pub struct UpdateGameRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
