use crate::{
    app::App,
    client::{InsertWadRequest, Pagination},
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use dorch_common::{access_log, response};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let router = Router::new()
        .route("/upsert_wad", post(upsert_wad))
        .route("/wad", get(list_wads))
        .route("/wad/{wad_id}", get(get_wad))
        .route("/wad/{wad_id}/{map_name}", get(get_wad_map))
        .with_state(app_state)
        .layer(middleware::from_fn(access_log::internal));
    let port = args.internal_port;
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting internal server • port=".green(),
        format!("{}", port).green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, router.merge(health_router))
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve internal router")?;
    println!(
        "{} {} {} {} {}",
        "🛑 Internal server on port".red(),
        format!("{}", port).red().dimmed(),
        "shut down gracefully".red(),
        "• uptime was".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

async fn health() -> impl IntoResponse {
    StatusCode::OK.into_response()
}

pub async fn upsert_wad(
    State(state): State<App>,
    Json(req): Json<InsertWadRequest>,
) -> impl IntoResponse {
    match state.db.insert_wad(&req.meta, &req.maps).await {
        Ok(wad_id) => (StatusCode::OK, Json(wad_id)).into_response(),
        Err(e) => response::error(e.context("Failed to insert wad")),
    }
}

pub async fn get_wad(State(state): State<App>, Path(wad_id): Path<Uuid>) -> impl IntoResponse {
    match state.db.get_wad(wad_id).await {
        Ok(Some(wad)) => (StatusCode::OK, Json(wad)).into_response(),
        Ok(None) => response::not_found(anyhow!("Wad not found")),
        Err(e) => response::error(e.context("Failed to get wad")),
    }
}

pub async fn get_wad_map(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    match state.db.get_wad_map(wad_id, &map_name).await {
        Ok(Some(map)) => (StatusCode::OK, Json(map)).into_response(),
        Ok(None) => response::not_found(anyhow!("Wad map not found")),
        Err(e) => response::error(e.context("Failed to get wad map")),
    }
}

pub async fn list_wads(
    State(state): State<App>,
    Query(opts): Query<Pagination>,
) -> impl IntoResponse {
    match state
        .db
        .list_wads(opts.offset, opts.limit.unwrap_or(100))
        .await
    {
        Ok(maps) => (StatusCode::OK, Json(maps)).into_response(),
        Err(e) => response::error(e.context("Failed to get wad maps")),
    }
}
