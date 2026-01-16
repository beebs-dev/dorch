use crate::{
    app::App,
    client::{ListWadsRequest, WadImage, WadSearchRequest},
};
use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use dorch_common::{access_log, response, types::wad::InsertWad};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FeaturedRequest {
    pub limit: Option<i64>,
}

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let upsert_router = Router::new()
        .route("/upsert_wad", post(upsert_wad))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100 MiB
        .with_state(app_state.clone())
        .layer(middleware::from_fn(access_log::internal));
    let router = Router::new()
        .route("/wad", get(list_wads))
        .route("/featured", get(featured_wads))
        .route("/wad/{id}", get(get_wad))
        .route("/wad/{id}/map/{map}", get(get_wad_map))
        .route(
            "/wad/{id}/maps/{map}/images",
            get(list_wad_map_images).put(put_wad_map_images),
        )
        .route("/search", get(search))
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
        port.green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, upsert_router.merge(router).merge(health_router))
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve internal router")?;

    println!(
        "{}{}{}{}",
        "🛑 Internal server on port ".red(),
        port.red().dimmed(),
        " shut down gracefully • uptime=".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

async fn health() -> impl IntoResponse {
    StatusCode::OK.into_response()
}

pub async fn upsert_wad(State(state): State<App>, Json(req): Json<InsertWad>) -> impl IntoResponse {
    let wad_id = match state.db.upsert_wad(&req).await {
        Ok(wad_id) => wad_id,
        Err(e) => return response::error(e.context("Failed to insert wad")),
    };
    println!(
        "{}{}{}{}{}{}{}{}",
        "✅ Upserted WAD • wad_id=".green(),
        wad_id.green().dimmed(),
        " • size=".green(),
        req.meta
            .file
            .size
            .map(|s| s.to_string())
            .as_deref()
            .unwrap_or("<null>")
            .green()
            .dimmed(),
        " • title=".green(),
        req.meta
            .title
            .as_deref()
            .unwrap_or("<null>")
            .green()
            .dimmed(),
        " • sha1=".green(),
        req.meta.sha1.green().dimmed()
    );
    (StatusCode::OK, Json(wad_id)).into_response()
}

pub async fn list_wads(
    State(state): State<App>,
    Query(req): Query<ListWadsRequest>,
) -> impl IntoResponse {
    let offset = req.pagination.offset.max(0);
    let limit = req.pagination.limit.unwrap_or(10).clamp(1, 100);
    match state.db.list_wads(offset, limit, req.sort_desc).await {
        Ok(wads) => (StatusCode::OK, Json(wads)).into_response(),
        Err(e) => response::error(e.context("Failed to list wads")),
    }
}

pub async fn featured_wads(
    State(state): State<App>,
    Query(req): Query<FeaturedRequest>,
) -> impl IntoResponse {
    let limit = req.limit.unwrap_or(6).clamp(1, 100);
    match state.db.featured_wads(limit).await {
        Ok(wads) => (StatusCode::OK, Json(wads)).into_response(),
        Err(e) => response::error(e.context("Failed to list featured wads")),
    }
}

pub async fn get_wad_map(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    match state.db.get_wad_map(wad_id, &map_name).await {
        Ok(Some(resp)) => (StatusCode::OK, Json(resp)).into_response(),
        Ok(None) => response::not_found(anyhow::anyhow!("WAD map not found")),
        Err(e) => response::error(e.context("Failed to get wad map")),
    }
}

pub async fn get_wad(State(state): State<App>, Path(wad_id): Path<Uuid>) -> impl IntoResponse {
    match state.db.get_wad(wad_id).await {
        Ok(Some(wad)) => (StatusCode::OK, Json(wad)).into_response(),
        Ok(None) => response::not_found(anyhow::anyhow!("WAD not found")),
        Err(e) => response::error(e.context("Failed to get wad")),
    }
}

pub async fn search(
    State(state): State<App>,
    Query(req): Query<WadSearchRequest>,
) -> impl IntoResponse {
    let request_id = Uuid::new_v4();
    let offset = req.offset.max(0);
    let limit = req.limit.unwrap_or(10).min(100);
    println!(
        "{}{}{}{}{}{}{}{}",
        "🔍 Searching wads • query=".green(),
        req.query.green().dimmed(),
        " • offset=".green(),
        offset.green().dimmed(),
        " • limit=".green(),
        limit.green().dimmed(),
        " • request_id=".green(),
        request_id.green().dimmed()
    );
    let start = std::time::Instant::now();
    match state
        .db
        .search_wads(request_id, &req.query, offset, limit)
        .await
    {
        Ok(resp) => {
            println!(
                "{}{}{}{}{}{}{}{}",
                "✅ Searched WADs • query=".green(),
                req.query.green().dimmed(),
                " • elapsed=".green(),
                humantime::format_duration(start.elapsed()).green().dimmed(),
                " • results=".green(),
                resp.items.len().green().dimmed(),
                " • request_id=".green(),
                request_id.green().dimmed()
            );
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => response::error(e.context("Failed to search wads")),
    }
}

pub async fn list_wad_map_images(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    match state.db.list_wad_map_images(wad_id, &map_name).await {
        Ok(images) => (StatusCode::OK, Json(images)).into_response(),
        Err(e) => response::error(e.context("Failed to list wad map images")),
    }
}

pub async fn put_wad_map_images(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
    Json(images): Json<Vec<WadImage>>,
) -> impl IntoResponse {
    match state
        .db
        .replace_wad_map_images(wad_id, &map_name, &images)
        .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => response::error(e.context("Failed to replace wad map images")),
    }
}
