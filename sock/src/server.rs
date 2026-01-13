use anyhow::{Context, Result};
use axum::{
    Router, middleware,
    routing::{get, post},
};
use axum_keycloak_auth::{
    PassthroughMode,
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
};
use dorch_common::{access_log, args::KeycloakArgs, cors};
use owo_colors::OwoColorize;
use reqwest::Url;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use crate::common::AppState;

pub async fn run(
    cancel: CancellationToken,
    port: u16,
    app_state: AppState,
    kc: KeycloakArgs,
) -> Result<()> {
    dorch_common::metrics::maybe_spawn_metrics_server();
    println!("Using Keycloak endpoint: {}", kc.endpoint);
    println!("Expecting Keycloak audience: {}", kc.client_id);
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
        .expected_audiences(vec![kc.client_id, "account".to_string()])
        .build();
    let health_router = Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(health));
    let auth_router = Router::new()
        .route("/ws/auth", post(crate::auth::begin_handshake))
        .layer(keycloak_layer)
        .layer(middleware::from_fn(access_log::public))
        .with_state(app_state.clone());
    let ws_router = Router::new()
        .route("/ws", get(crate::ws::upgrade))
        .layer(middleware::from_fn(access_log::public))
        .with_state(app_state);
    let addr: SocketAddr = format!("0.0.0.0:{}", port)
        .parse()
        .expect("Invalid address");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "{}{}",
        "🚀 Starting sock server • port=".green(),
        format!("{}", port).green().dimmed()
    );
    let router = auth_router.merge(ws_router).layer(cors::dev());
    let start = std::time::Instant::now();
    axum::serve(listener, router.merge(health_router))
        .with_graceful_shutdown(async move { cancel.cancelled().await })
        .await
        .context("Failed to serve internal router")?;
    println!(
        "{} {} {} {} {}",
        "🛑 Server on port".red(),
        format!("{}", port).red().dimmed(),
        "shut down gracefully".red(),
        "• uptime was".red(),
        humantime::format_duration(start.elapsed()).red().dimmed()
    );
    Ok(())
}

async fn health() -> &'static str {
    "OK"
}
