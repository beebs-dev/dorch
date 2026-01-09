use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(CustomResource, Serialize, Deserialize, Default, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "game.beebs.dev",
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
pub struct GameSpec {}

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
