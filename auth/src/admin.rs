use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use dorch_auth::client::UserRecordJson;
use dorch_common::access_log;
use owo_colors::OwoColorize;
use tokio_util::sync::CancellationToken;

use crate::user_record_store::UserRecordStore;

pub async fn run(store: UserRecordStore, port: u16, cancel: CancellationToken) -> Result<()> {
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let router = Router::new()
        .route("/admin/user", post(post_user_record))
        .with_state(store)
        .layer(middleware::from_fn(access_log::admin));
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting admin server • port=".green(),
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
        "🛑 Admin server on port".red(),
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

async fn post_user_record(
    State(store): State<UserRecordStore>,
    Json(req): Json<UserRecordJson>,
) -> impl IntoResponse {
    match store.set_json(req).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => {
            eprintln!("admin POST /admin/user failed: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
