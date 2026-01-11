use crate::{app::App, token::make_livekit_token};
use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::get,
};
use dorch_common::{access_log, cors, rbac::UserId};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let router = Router::new()
        .route("/auth/{game_id}", get(handle_auth_game))
        .with_state(app_state)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());
    let port = args.port;
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting server • port=".green(),
        port.green().dimmed()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve public router")?;
    println!("{}", "🛑 Server shut down gracefully.".red());
    Ok(())
}

#[derive(serde::Deserialize)]
struct JoinQuery {
    identity: String,
}

async fn handle_auth_game(
    State(state): State<App>,
    UserId(_user_id): UserId,
    Path(game_id): Path<Uuid>,
    Query(JoinQuery { identity }): Query<JoinQuery>, // FIXME
) -> impl IntoResponse {
    // TODO: ensure the user can join the server
    let game_id = game_id.to_string();
    let token = make_livekit_token(&state.api_key, &state.api_secret, &identity, &game_id);
    let resp = serde_json::json!({
        "token": token,
        "url": &state.external_livekit_url
    });
    (StatusCode::OK, Json(resp))
}
