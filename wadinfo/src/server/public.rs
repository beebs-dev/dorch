use crate::{
    app::App,
    avatar::MAX_AVATAR_UPLOAD_BYTES,
    client::{
        ListDraftsResponse, ListUserWadsResponse, PutUserProfileRequest, ResolveWadURLsRequest, ResolveWadURLsResponse,
        UpdateDraftRequest, UpdateWadRequest, UploadResponse,
    },
    server::internal,
    wad_upload::MAX_WAD_UPLOAD_BYTES,
};
use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
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

    // Upload route needs larger body limit
    let upload_router = Router::new()
        .route("/upload", post(upload_wad))
        .layer(DefaultBodyLimit::max(MAX_WAD_UPLOAD_BYTES))
        .with_state(app_state.clone())
        .layer(keycloak_layer.clone())
        .layer(RateLimitLayer::new(rate_limiter.clone()))
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::prod(&allowed_origins));

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
        .route("/wad/{id}", get(internal::get_wad).put(update_wad).delete(delete_wad))
        .route("/wad/{id}/map/{map}", get(internal::get_wad_map))
        .route("/search", get(internal::search))
        // Draft management endpoints
        .route("/draft", get(list_drafts).post(create_draft))
        .route("/draft/resume", get(resume_or_create_draft))
        .route(
            "/draft/{draft_id}",
            get(get_draft).put(update_draft).delete(delete_draft),
        )
        .route("/draft/{draft_id}/publish", post(publish_draft))
        // User's published WADs
        .route("/my-wads", get(list_user_wads))
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
    axum::serve(listener, upload_router.merge(protected_router).merge(router))
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

// ----------------------------
// WAD Upload Endpoint
// ----------------------------

pub async fn upload_wad(
    State(state): State<App>,
    UserId(_user_id): UserId,
    mut multipart: Multipart,
) -> impl IntoResponse {
    eprintln!("📥 upload_wad: starting to process multipart");
    
    // Find the file field
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                eprintln!("📥 upload_wad: got field name={:?}", name);
                
                if name == "file" {
                    let filename = match field.file_name().map(|s| s.to_string()) {
                        Some(f) => f,
                        None => return response::error(anyhow!("No filename provided")),
                    };
                    eprintln!("📥 upload_wad: filename={:?}, starting stream upload", filename);

                    // Stream the upload directly to S3
                    match state
                        .wad_upload_store
                        .upload_draft_stream(&filename, field)
                        .await
                    {
                        Ok((hash, sha1, upload_id, size)) => {
                            eprintln!("📥 upload_wad: SUCCESS hash={} sha1={} size={}", hash, sha1, size);
                            return (
                                StatusCode::OK,
                                Json(UploadResponse {
                                    hash,
                                    sha1,
                                    id: upload_id,
                                    size,
                                }),
                            )
                                .into_response();
                        }
                        Err(e) => {
                            eprintln!("📥 upload_wad: FAILED {:?}", e);
                            return response::error(e.context("Failed to upload WAD file"));
                        }
                    }
                }
            }
            Ok(None) => {
                eprintln!("📥 upload_wad: no more fields");
                break;
            }
            Err(e) => {
                eprintln!("📥 upload_wad: next_field error: {:?}", e);
                return response::error(anyhow!("Multipart field error: {}", e));
            }
        }
    }

    response::error(anyhow!("No file field provided"))
}

// ----------------------------
// Draft Management Endpoints
// ----------------------------

pub async fn list_drafts(
    State(state): State<App>,
    UserId(user_id): UserId,
) -> impl IntoResponse {
    match state.db.list_drafts(user_id).await {
        Ok(items) => (StatusCode::OK, Json(ListDraftsResponse { items })).into_response(),
        Err(e) => response::error(e.context("Failed to list drafts")),
    }
}

pub async fn list_user_wads(
    State(state): State<App>,
    UserId(user_id): UserId,
) -> impl IntoResponse {
    match state.db.list_user_wads(user_id).await {
        Ok(items) => (StatusCode::OK, Json(ListUserWadsResponse { items })).into_response(),
        Err(e) => response::error(e.context("Failed to list user WADs")),
    }
}

pub async fn create_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
) -> impl IntoResponse {
    match state.db.create_draft(user_id).await {
        Ok(draft) => (StatusCode::CREATED, Json(draft)).into_response(),
        Err(e) => response::error(e.context("Failed to create draft")),
    }
}

pub async fn resume_or_create_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
) -> impl IntoResponse {
    // Try to find an existing unpublished draft first
    match state.db.get_unpublished_draft(user_id).await {
        Ok(Some(draft)) => return (StatusCode::OK, Json(draft)).into_response(),
        Ok(None) => {}
        Err(e) => return response::error(e.context("Failed to check for existing draft")),
    }

    // No existing draft, create a new one
    match state.db.create_draft(user_id).await {
        Ok(draft) => (StatusCode::CREATED, Json(draft)).into_response(),
        Err(e) => response::error(e.context("Failed to create draft")),
    }
}

pub async fn get_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(draft_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_draft(draft_id).await {
        Ok(Some(draft)) => {
            // Check ownership
            if draft.uploader_id != user_id {
                return response::forbidden(anyhow!("Not authorized to view this draft"));
            }
            (StatusCode::OK, Json(draft)).into_response()
        }
        Ok(None) => response::not_found(anyhow!("Draft not found")),
        Err(e) => response::error(e.context("Failed to get draft")),
    }
}

pub async fn update_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(draft_id): Path<Uuid>,
    Json(req): Json<UpdateDraftRequest>,
) -> impl IntoResponse {
    match state.db.update_draft(draft_id, user_id, &req).await {
        Ok(Some(draft)) => (StatusCode::OK, Json(draft)).into_response(),
        Ok(None) => response::not_found(anyhow!("Draft not found or not authorized")),
        Err(e) => response::error(e.context("Failed to update draft")),
    }
}

pub async fn delete_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(draft_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.delete_draft(draft_id, user_id).await {
        Ok(Some((_, upload_id))) => {
            // If there was an uploaded file, delete it from storage
            if let Some(upload_id) = upload_id {
                if let Err(e) = state.wad_upload_store.delete_draft(upload_id).await {
                    eprintln!("Warning: Failed to delete draft file from storage: {}", e);
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(None) => response::not_found(anyhow!("Draft not found or not authorized")),
        Err(e) => response::error(e.context("Failed to delete draft")),
    }
}

pub async fn delete_wad(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(wad_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.delete_wad(wad_id, user_id).await {
        Ok(Some((_, file_sha256, filename))) => {
            // If there was a file in permanent storage, delete it
            if let Some(sha256) = file_sha256 {
                if let Err(e) = state
                    .wad_upload_store
                    .delete_permanent(&sha256, &filename)
                    .await
                {
                    eprintln!("Warning: Failed to delete WAD file from storage: {}", e);
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(None) => response::not_found(anyhow!("WAD not found or not authorized")),
        Err(e) => response::error(e.context("Failed to delete WAD")),
    }
}

pub async fn update_wad(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(wad_id): Path<Uuid>,
    Json(request): Json<UpdateWadRequest>,
) -> impl IntoResponse {
    match state.db.update_wad(wad_id, user_id, request.title.as_deref()).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => response::not_found(anyhow!("WAD not found or not authorized")),
        Err(e) => response::error(e.context("Failed to update WAD")),
    }
}

pub async fn publish_draft(
    State(state): State<App>,
    UserId(user_id): UserId,
    Path(draft_id): Path<Uuid>,
) -> impl IntoResponse {
    // First, get the draft to validate and get file info
    let draft = match state.db.get_draft(draft_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return response::not_found(anyhow!("Draft not found")),
        Err(e) => return response::error(e.context("Failed to get draft")),
    };

    // Check ownership
    if draft.uploader_id != user_id {
        return response::forbidden(anyhow!("Not authorized to publish this draft"));
    }

    // Ensure upload_id is set
    let upload_id = match draft.upload_id {
        Some(id) => id,
        None => return response::error(anyhow!("Cannot publish draft without uploaded file")),
    };

    // Get sha256 for permanent storage key
    let sha256 = match &draft.file_sha256 {
        Some(h) => h.clone(),
        None => return response::error(anyhow!("Draft missing file hash")),
    };

    // Get sha1 for wads table lookup
    let sha1 = match &draft.file_sha1 {
        Some(h) => h.clone(),
        None => return response::error(anyhow!("Draft missing SHA1 hash")),
    };

    // Move file to permanent storage
    // Use the original filename for the permanent key extension
    let original_filename = draft.filename.as_deref().unwrap_or("upload.wad");
    let file_url = match state
        .wad_upload_store
        .publish_draft(upload_id, &sha256, original_filename)
        .await
    {
        Ok(url) => url,
        Err(e) => return response::error(e.context("Failed to move file to permanent storage")),
    };

    // Insert WAD, upsert status, and delete draft in a single transaction
    match state
        .db
        .publish_wad_from_draft(
            &sha1,
            Some(&sha256),
            draft.title.as_deref(),
            draft.filename.as_deref(),
            draft.file_size,
            &file_url,
            draft_id,
            user_id,
        )
        .await
    {
        Ok(wad_id) => {
            (StatusCode::OK, Json(serde_json::json!({ "wad_id": wad_id }))).into_response()
        }
        Err(e) => response::error(e.context("Failed to publish WAD from draft")),
    }
}
