use k8s_openapi::api::core::v1::ResourceRequirements;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(CustomResource, Serialize, Deserialize, Default, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "dorch.beebs.dev",
    version = "v1",
    kind = "Game",
    plural = "games",
    derive = "PartialEq",
    status = "GameStatus",
    namespaced
)]
#[kube(derive = "Default")]
#[kube(
    printcolumn = "{\"jsonPath\": \".status.phase\", \"name\": \"PHASE\", \"type\": \"string\" }"
)]
#[kube(
    printcolumn = "{\"jsonPath\": \".status.lastUpdated\", \"name\": \"AGE\", \"type\": \"date\" }"
)]
pub struct GameSpec {
    pub game_id: String,
    pub s3_secret_name: String,
    pub iwad: String,
    pub max_players: i32,
    pub files: Option<Vec<String>>,
    pub name: String,
    pub warp: Option<String>,
    pub skill: Option<i32>,

    /// If true, doom1.wad will be prepended to the file list automatically.
    /// This allows users to create modded games using Doom 1 assets while
    /// respecting IWAD licensing.
    pub use_doom1_assets: bool,

    /// If true, the game will only be visible to the creator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_udp: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceRequirements>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default, JsonSchema)]
pub struct WadReference {
    pub name: String,

    pub id: String,
}

/// Status object for the [`Game`] resource.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default, JsonSchema)]
pub struct GameStatus {
    /// A short description of the [`Game`] resource's current state.
    pub phase: GamePhase,

    /// A human-readable message indicating details about why the
    /// [`Game`] is in this phase.
    pub message: Option<String>,

    /// Timestamp of when the [`GameStatus`] object was last updated.
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
}

/// A short description of the [`Game`] resource's current state.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, JsonSchema, Default)]
pub enum GamePhase {
    #[default]
    Pending,
    Starting,
    Error,
    Active,
    Terminating,
}

impl FromStr for GamePhase {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(GamePhase::Pending),
            "Starting" => Ok(GamePhase::Starting),
            "Active" => Ok(GamePhase::Active),
            "Terminating" => Ok(GamePhase::Terminating),
            "Error" => Ok(GamePhase::Error),
            _ => Err(()),
        }
    }
}

impl fmt::Display for GamePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GamePhase::Pending => write!(f, "Pending"),
            GamePhase::Starting => write!(f, "Starting"),
            GamePhase::Active => write!(f, "Active"),
            GamePhase::Terminating => write!(f, "Terminating"),
            GamePhase::Error => write!(f, "Error"),
        }
    }
}
