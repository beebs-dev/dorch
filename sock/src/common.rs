use crate::{args, keycloak::Keycloak};
use anyhow::Error;
use async_nats::ConnectOptions;
use axum::Extension;
use axum::RequestPartsExt;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_keycloak_auth::decode::KeycloakToken;
use deadpool_redis::Pool;
use dorch_common::response;
use owo_colors::OwoColorize;
use tokio::join;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub redis: Pool,
    pub nats: async_nats::Client,
    pub kc: Keycloak,
}

impl AppState {
    pub async fn new(cli: args::Cli) -> AppState {
        let (redis, nats) = join!(dorch_common::redis::init_redis(&cli.redis), async move {
            println!(
                "{} {}",
                "🔌 Connecting to NATS • url=".green(),
                cli.nats.nats_url.green().dimmed()
            );
            async_nats::connect_with_options(
                &cli.nats.nats_url,
                ConnectOptions::new().user_and_password("app".into(), "devpass".into()),
            )
            .await
            .expect("Failed to connect to NATS")
        });
        let kc = Keycloak {
            args: cli.kc,
            client: reqwest::Client::new(),
        };
        AppState { redis, nats, kc }
    }
}

pub struct UserId(pub Uuid);

impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = axum::response::Response;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Extension(token) = match parts.extract::<Extension<KeycloakToken<String>>>().await {
            Ok(ext) => ext,
            Err(e) => {
                return Err(response::unauthorized(
                    Error::from(e).context("Failed to extract Keycloak token from request"),
                ));
            }
        };
        match Uuid::parse_str(&token.subject) {
            Ok(id) => Ok(UserId(id)),
            Err(e) => Err(response::unauthorized(
                Error::from(e).context("Invalid user ID in token subject claim"),
            )),
        }
    }
}
