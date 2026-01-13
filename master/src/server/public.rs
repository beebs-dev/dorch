use crate::{
    app::App,
    client::{JoinGameResponse, NewGameRequest},
    server::internal::{self, CREATOR_USER_ID_ANNOTATION},
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post},
};
use axum_keycloak_auth::{
    PassthroughMode,
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use dorch_auth::client::UserRecordJson;
use dorch_common::{access_log, args::KeycloakArgs, cors, rbac::UserId, response};
use kube::Api;
use owo_colors::OwoColorize;
use reqwest::Url;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    port: u16,
    kc: KeycloakArgs,
    app_state: App,
) -> Result<()> {
    let keycloak_auth_instance = KeycloakAuthInstance::new(
        KeycloakConfig::builder()
            .server(Url::parse(&kc.endpoint).unwrap())
            .realm(kc.realm)
            .build(),
    );
    let keycloak_layer = KeycloakAuthLayer::<String>::builder()
        .instance(keycloak_auth_instance)
        .passthrough_mode(PassthroughMode::Block)
        .persist_raw_claims(true)
        .expected_audiences(vec![kc.client_id])
        .build();
    let protected_router = Router::new()
        .route("/game", post(new_game))
        .route("/game/{game_id}", delete(delete_game))
        .route("/game/{game_id}/join", post(join_game))
        .with_state(app_state.clone())
        .layer(keycloak_layer)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());
    let router = Router::new()
        .route("/game", get(internal::list_games))
        .route("/game/{game_id}", get(internal::get_game))
        .with_state(app_state)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting public server • port=".green(),
        format!("{}", port).green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, protected_router.merge(router))
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve public router")?;
    println!(
        "{} {} {} {} {}",
        "🛑 Public server on port".red(),
        format!("{}", port).red().dimmed(),
        "shut down gracefully".red(),
        "• uptime was".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

pub async fn new_game(
    State(state): State<App>,
    UserId(user_id): UserId,
    Json(mut req): Json<NewGameRequest>,
) -> impl IntoResponse {
    req.creator_id = user_id;
    internal::new_game(State(state), Json(req)).await
}

pub async fn join_game(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(game_id): Path<Uuid>,
) -> impl IntoResponse {
    let info = match state.store.get_game_info(game_id).await {
        Ok(Some(info)) => info,
        Ok(None) => return response::not_found(anyhow!("Game not found")),
        Err(e) => return response::error(e.context("Failed to get game info")),
    };
    if info.player_count >= info.max_players {
        return response::forbidden(anyhow!("Game is full"));
    }
    let username = user_id.to_string();
    let password = Uuid::new_v4().to_string();
    let secrets =
        match dorch_auth::zandronum_srp_sha256::generate_user_secrets(&username, &password) {
            Ok(v) => v,
            Err(e) => {
                return response::error(e.context("Failed to generate SRP secrets"));
            }
        };
    let record = UserRecordJson {
        disabled: false,
        username: username.clone(),
        salt_b64: B64.encode(&secrets.salt),
        verifier_b64: B64.encode(&secrets.verifier),
    };
    if let Err(e) = state.auth.post_user_record(&record).await {
        response::error(e.context("Failed to post user record"))
    } else {
        let resp = JoinGameResponse {
            game_id,
            password,
            username,
        };
        (StatusCode::OK, Json(resp)).into_response()
    }
}

pub async fn delete_game(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(game_id): Path<Uuid>,
) -> impl IntoResponse {
    let game = match Api::<dorch_types::Game>::namespaced(state.client.clone(), &state.namespace)
        .get(&game_id.to_string())
        .await
    {
        Ok(game) => game,
        Err(e) => {
            return match e {
                kube::Error::Api(ae) if ae.code == 404 => {
                    response::not_found(anyhow!("Game not found"))
                }
                _ => response::error(anyhow!("Failed to get game: {:?}", e)),
            };
        }
    };
    let is_creator = game
        .metadata
        .annotations
        .as_ref()
        .and_then(|m| m.get(CREATOR_USER_ID_ANNOTATION))
        .map(|s| s.parse::<Uuid>())
        .transpose()
        .ok()
        .flatten()
        .is_some_and(|s| s == user_id);
    if is_creator {
        // TODO: allow admins to delete any game
        internal::delete_game(State(state), Path(game_id))
            .await
            .into_response()
    } else {
        response::forbidden(anyhow!("Only the creator can delete the game"))
    }
}
