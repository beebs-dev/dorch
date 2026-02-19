use crate::{
    app::App,
    avatar::MAX_AVATAR_UPLOAD_BYTES,
    client::{PutUserProfileRequest, ResolveWadURLsRequest, ResolveWadURLsResponse},
    server::internal,
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use axum_keycloak_auth::{
    PassthroughMode,
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
};
use bytes::Bytes;
use dorch_common::{
    access_log,
    args::KeycloakArgs,
    cors,
    rate_limit::{RateLimiter, middleware::RateLimitLayer},
    rbac::UserId,
    response,
};
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
    rate_limiter: RateLimiter,
) -> Result<()> {
    let allowed_origins = vec!["https://gib.gg", "https://www.gib.gg"];
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
        .expected_audiences(vec!["account".to_string()])
        .build();
    let protected_router = Router::new()
        .route("/wad", get(internal::list_wads))
        .route("/featured", get(internal::featured_wads))
        .route(
            "/user/profile/{user_id}",
            get(internal::get_user_profile_public).put(put_user_profile),
        )
        .route(
            "/user/profile/{user_id}/avatar",
            post(put_user_profile_avatar)
                .put(put_user_profile_avatar)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_UPLOAD_BYTES)),
        )
        .route("/wad/{id}", get(internal::get_wad))
        .route("/wad/{id}/map/{map}", get(internal::get_wad_map))
        .route("/search", get(internal::search))
        .with_state(app_state.clone())
        .layer(keycloak_layer)
        .layer(RateLimitLayer::new(rate_limiter.clone()))
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::prod(&allowed_origins));

    // Unprotected endpoints (no Keycloak middleware)
    let router = Router::new()
        .route(
            "/wad/{id}/maps/{map}/images",
            get(internal::list_wad_map_images),
        )
        .route("/wad_urls", post(resolve_wad_public_urls))
        .with_state(app_state)
        .layer(RateLimitLayer::new(rate_limiter))
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::prod(&allowed_origins));
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting public server • port=".green(),
        port.green().dimmed()
    );
    let start = std::time::Instant::now();
    axum::serve(listener, protected_router.merge(router))
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .context("Failed to serve public router")?;
    println!(
        "{}{}{}{}",
        "🛑 Public server on port ".red(),
        port.red().dimmed(),
        " shut down gracefully • uptime=".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

pub async fn put_user_profile_avatar(
    State(state): State<App>,
    UserId(authenticated_user_id): UserId,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if authenticated_user_id != user_id {
        eprintln!(
            "{}{}{}",
            "⚠️  User ".yellow(),
            authenticated_user_id.to_string().yellow().dimmed(),
            " attempted to modify another user's profile".yellow()
        );
        return response::forbidden(anyhow!("Not allowed to modify another user's profile"));
    }
    internal::put_user_profile_avatar_common(state, user_id, headers, body).await
}

pub async fn delete_user_profile_avatar(
    State(state): State<App>,
    UserId(authenticated_user_id): UserId,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    if authenticated_user_id != user_id {
        return response::forbidden(anyhow!("Not allowed to modify another user's profile"));
    }
    internal::delete_user_profile_avatar_common(state, user_id).await
}

pub async fn resolve_wad_public_urls(
    State(state): State<App>,
    Json(req): Json<ResolveWadURLsRequest>,
) -> impl IntoResponse {
    let mut items = match internal::resolve_wad_s3_urls_inner(state, &req).await {
        Ok(resp) => resp.items,
        Err(e) => return response::error(e),
    };
    for item in items
        .iter_mut()
        .filter(|item| item.url.starts_with("s3://"))
    {
        // rewrite to be https://bucketname.nyc3.digitaloceanspaces.com/key
        let url = item.url.replace("s3://", "");
        let parts: Vec<&str> = url.splitn(2, '/').collect();
        let bucket = parts[0];
        let key = parts.get(1).unwrap_or(&"");
        item.url = format!("https://{}.nyc3.digitaloceanspaces.com/{}", bucket, key);
    }
    (StatusCode::OK, Json(ResolveWadURLsResponse { items })).into_response()
}

pub async fn put_user_profile(
    State(state): State<App>,
    UserId(authenticated_user_id): UserId,
    Path(user_id): Path<Uuid>,
    Json(req): Json<PutUserProfileRequest>,
) -> impl IntoResponse {
    if authenticated_user_id != user_id {
        return response::forbidden(anyhow!("Not allowed to modify another user's profile"));
    }
    match state.db.update_user_profile(user_id, &req).await {
        Ok(Some(profile)) => (StatusCode::OK, Json(profile)).into_response(),
        Ok(None) => response::not_found(anyhow!("User profile not found")),
        Err(e) => response::error(e.context("Failed to update user profile")),
    }
}
