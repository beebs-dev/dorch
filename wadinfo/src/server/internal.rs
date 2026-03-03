use crate::{
    app::App,
    avatar::MAX_AVATAR_UPLOAD_BYTES,
    client::{
        GetWadMetasRequest, GetWadMetasResponse, ListWadsRequest, MapAnalysis,
        PutUserProfileRequest, ResolveMapThumbnailsRequest, ResolveMapThumbnailsResponse,
        ResolveWadURLsRequest, ResolveWadURLsResponse, UserProfileView, WadAnalysis, WadImage,
        WadSearchRequest,
    },
};
use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use dorch_common::{
    access_log,
    rate_limit::{RateLimiter, middleware::RateLimitLayer},
    response,
    types::wad::InsertWad,
};
use owo_colors::OwoColorize;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FeaturedRequest {
    /// Pagination for the main list portion (same semantics as GET /wad).
    #[serde(flatten)]
    pub pagination: dorch_common::Pagination,

    /// If true, sort descending. Otherwise, sort ascending.
    #[serde(rename = "d", default)]
    pub sort_desc: bool,

    /// Number of featured items to return.
    pub featured_limit: Option<i64>,
}

pub async fn run_server(
    cancel: CancellationToken,
    port: u16,
    app_state: App,
    rate_limiter: RateLimiter,
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
        .route("/thumbnails", post(resolve_map_thumbnails))
        .route(
            "/user/profile/{user_id}",
            get(get_user_profile_internal).put(put_user_profile_internal),
        )
        .route(
            "/user/profile/{user_id}/avatar",
            post(put_user_profile_avatar)
                .put(put_user_profile_avatar)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES)),
        )
        .route("/user/activity/{user_id}", post(post_user_activity))
        .route("/wad", get(list_wads))
        .route("/wad_metas", post(get_wad_metas))
        .route("/wad_urls", post(resolve_wad_s3_urls))
        .route("/featured", get(featured_wads))
        .route("/wad/{id}", get(get_wad))
        .route("/wad/{id}/analysis", post(post_wad_analysis))
        .route("/wad/{id}/map_analyses", get(list_wad_analyses))
        .route(
            "/wad/{id}/map/{map}/analysis",
            post(post_map_analysis).get(get_map_analysis_exists),
        )
        .route("/wad/{id}/map/{map}", get(get_wad_map))
        .route(
            "/wad/{id}/maps/{map}/images",
            get(list_wad_map_images).put(put_wad_map_images),
        )
        .route("/search", get(search))
        .with_state(app_state)
        .layer(RateLimitLayer::new(rate_limiter))
        .layer(middleware::from_fn(access_log::internal));
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

pub async fn put_user_profile_avatar(
    State(state): State<App>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    put_user_profile_avatar_common(state, user_id, headers, body).await
}

pub async fn delete_user_profile_avatar(
    State(state): State<App>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    delete_user_profile_avatar_common(state, user_id).await
}

pub async fn delete_user_profile_avatar_common(
    state: App,
    user_id: Uuid,
) -> axum::response::Response {
    match state
        .db
        .update_user_profile(
            user_id,
            &PutUserProfileRequest {
                avatar_url: Some(None),
                player_color: None,
                username: None,
                display_name: None,
                privacy_hide_activity: None,
            },
        )
        .await
    {
        Ok(Some(profile)) => (StatusCode::OK, Json(profile)).into_response(),
        Ok(None) => response::not_found(anyhow!("User profile not found")),
        Err(e) => response::error(e.context("Failed to update user profile avatar deletion")),
    }
}

pub async fn get_wad_metas(
    State(state): State<App>,
    Json(req): Json<GetWadMetasRequest>,
) -> impl IntoResponse {
    match state.db.get_wad_metas(&req.wad_ids).await {
        Ok(items) => (StatusCode::OK, Json(GetWadMetasResponse { items })).into_response(),
        Err(e) => response::error(e.context("Failed to get wad metas")),
    }
}
pub async fn get_map_analysis_exists(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    match state.db.map_analysis_exists(wad_id, &map_name).await {
        Ok(true) => StatusCode::OK.into_response(),
        Ok(false) => response::not_found(anyhow::anyhow!("Map analysis not found")),
        Err(e) => response::error(e.context("Failed to get map analysis")),
    }
}

pub async fn post_map_analysis(
    State(state): State<App>,
    Path((wad_id, map_name)): Path<(Uuid, String)>,
    Json(analysis): Json<MapAnalysis>,
) -> impl IntoResponse {
    if wad_id.is_nil() || map_name.is_empty() {
        return response::error(anyhow::anyhow!("WAD ID or map name in path is invalid"));
    }
    if wad_id != analysis.wad_id || map_name != analysis.map_name {
        return response::error(anyhow::anyhow!(
            "WAD ID or map name in path does not match analysis payload"
        ));
    }
    if let Err(e) = state.db.insert_map_analysis(&analysis).await {
        return response::error(e.context("Failed to insert map analysis"));
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn list_wad_analyses(
    State(state): State<App>,
    Path(wad_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.list_map_analyses(wad_id).await {
        Ok(analyses) => (StatusCode::OK, Json(analyses)).into_response(),
        Err(e) => response::error(e.context("Failed to list map analyses")),
    }
}

pub async fn post_wad_analysis(
    State(state): State<App>,
    Path(wad_id): Path<Uuid>,
    Json(req): Json<WadAnalysis>,
) -> impl IntoResponse {
    if wad_id.is_nil() {
        return response::error(anyhow::anyhow!("WAD ID in path is invalid"));
    }
    if wad_id != req.wad_id {
        return response::error(anyhow::anyhow!(
            "WAD ID in path does not match analysis payload"
        ));
    }
    if let Err(e) = state.db.insert_wad_analysis(&req).await {
        return response::error(e.context("Failed to insert wad analysis"));
    }
    StatusCode::NO_CONTENT.into_response()
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

pub async fn resolve_wad_s3_urls_inner(
    state: App,
    req: &ResolveWadURLsRequest,
) -> Result<ResolveWadURLsResponse> {
    let items = state
        .db
        .resolve_wad_urls(&req.wad_ids)
        .await
        .context("Failed to resolve wad URLs")?;
    if items.len() != req.wad_ids.len() {
        let resolved = items.iter().map(|item| item.wad_id).collect::<Vec<Uuid>>();
        let unresolved = req
            .wad_ids
            .iter()
            .filter(|id| !resolved.contains(id))
            .collect::<Vec<&Uuid>>();
        bail!(
            "Some WAD IDs could not be resolved: {}",
            serde_json::to_string(&unresolved).unwrap_or_default(),
        );
    }
    Ok(ResolveWadURLsResponse { items })
}

pub async fn resolve_wad_s3_urls(
    State(state): State<App>,
    Json(req): Json<ResolveWadURLsRequest>,
) -> impl IntoResponse {
    match resolve_wad_s3_urls_inner(state, &req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => response::error(e),
    }
}

pub async fn resolve_map_thumbnails(
    State(state): State<App>,
    Json(req): Json<ResolveMapThumbnailsRequest>,
) -> impl IntoResponse {
    println!(
        "{}{}",
        "🔍 Resolving map thumbnails • items=".green(),
        format!("{:?}", req.items).green().dimmed()
    );
    match state.db.resolve_map_thumbnails(&req.items).await {
        Ok(items) => {
            println!(
                "{}{}{}{}",
                "✅ Resolved map thumbnails • wanted=".green(),
                req.items.len().to_string().green().dimmed(),
                " • items=".green(),
                format!("{:?}", items).green().dimmed()
            );
            (StatusCode::OK, Json(ResolveMapThumbnailsResponse { items })).into_response()
        }
        Err(e) => response::error(e.context("Failed to resolve map thumbnails")),
    }
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
    let offset = req.pagination.offset.max(0);
    let limit = req.pagination.limit.unwrap_or(25).clamp(1, 100);
    let featured_limit = req
        .featured_limit
        .unwrap_or(if offset == 0 { 6 } else { 0 })
        .clamp(0, 100);
    let start = std::time::Instant::now();
    match state
        .db
        .featured_view(offset, limit, req.sort_desc, featured_limit)
        .await
    {
        Ok(view) => {
            println!(
                "{}{}{}{}{}{}{}{}",
                "✅ Featured view • offset=".green(),
                offset.green().dimmed(),
                " • limit=".green(),
                limit.green().dimmed(),
                " • featured_limit=".green(),
                featured_limit.green().dimmed(),
                " • elapsed=".green(),
                humantime::format_duration(start.elapsed()).green().dimmed()
            );
            (StatusCode::OK, Json(view)).into_response()
        }
        Err(e) => response::error(e.context("Failed to build featured view")),
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

pub async fn get_user_profile_internal(
    State(state): State<App>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_user_profile(user_id).await {
        Ok(Some(profile)) => (StatusCode::OK, Json(UserProfileView::Full(profile))).into_response(),
        Ok(None) => response::not_found(anyhow!("User profile not found")),
        Err(e) => response::error(e.context("Failed to get user profile")),
    }
}

pub async fn put_user_profile_internal(
    State(state): State<App>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<PutUserProfileRequest>,
) -> impl IntoResponse {
    match state.db.update_user_profile(user_id, &req).await {
        Ok(Some(profile)) => (StatusCode::OK, Json(profile)).into_response(),
        Ok(None) => match state.db.create_user_profile(user_id, &req).await {
            Ok(profile) => (StatusCode::OK, Json(profile)).into_response(),
            Err(e) => response::error(e.context("Failed to create user profile")),
        },
        Err(e) => response::error(e.context("Failed to update user profile")),
    }
}

pub async fn post_user_activity(
    State(state): State<App>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.touch_user_activity(user_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => response::not_found(anyhow!("User profile not found")),
        Err(e) => response::error(e.context("Failed to update user activity")),
    }
}

pub async fn put_user_profile_avatar_common(
    state: App,
    user_id: Uuid,
    headers: HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    if body.is_empty() {
        return response::bad_request(anyhow!("Avatar payload is empty"));
    }
    if body.len() > MAX_AVATAR_UPLOAD_BYTES {
        return response::bad_request(anyhow!(
            "Avatar exceeds max size of {} bytes",
            MAX_AVATAR_UPLOAD_BYTES
        ));
    }

    if let Some(content_type) = headers.get("content-type").and_then(|v| v.to_str().ok())
        && !content_type.to_ascii_lowercase().starts_with("image/")
    {
        return response::bad_request(anyhow!("Avatar must be an image upload"));
    }

    let avatar_url = match state.avatar_store.upload_avatar(&body).await {
        Ok(url) => url,
        Err(e) => return response::error(e.context("Failed to upload avatar")),
    };

    match state
        .db
        .update_user_profile(
            user_id,
            &PutUserProfileRequest {
                avatar_url: Some(Some(avatar_url)),
                player_color: None,
                username: None,
                display_name: None,
                privacy_hide_activity: None,
            },
        )
        .await
    {
        Ok(Some(profile)) => (StatusCode::OK, Json(profile)).into_response(),
        Ok(None) => response::not_found(anyhow!("User profile not found")),
        Err(e) => response::error(e.context("Failed to update user profile avatar")),
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
