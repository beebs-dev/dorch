use crate::{app::App, server::internal};
use anyhow::{Context, Result};
use axum::{Router, middleware, routing::get};
use axum_keycloak_auth::{
    PassthroughMode,
    instance::{KeycloakAuthInstance, KeycloakConfig},
    layer::KeycloakAuthLayer,
};
use dorch_common::{access_log, cors};
use owo_colors::OwoColorize;
use reqwest::Url;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

pub async fn run_server(
    cancel: CancellationToken,
    args: crate::args::ServerArgs,
    app_state: App,
) -> Result<()> {
    let keycloak_auth_instance = KeycloakAuthInstance::new(
        KeycloakConfig::builder()
            .server(Url::parse(&args.kc.endpoint).unwrap())
            .realm(args.kc.realm)
            .build(),
    );
    let keycloak_layer = KeycloakAuthLayer::<String>::builder()
        .instance(keycloak_auth_instance)
        .passthrough_mode(PassthroughMode::Block)
        .persist_raw_claims(true)
        .expected_audiences(vec![args.kc.client_id])
        .build();
    let protected_router = Router::new()
        .route("/wad", get(internal::list_wads))
        .route("/wad/{id}", get(internal::get_wad))
        .route("/wad/{id}/map/{map}", get(internal::get_wad_map))
        .route("/search", get(internal::search))
        .with_state(app_state.clone())
        .layer(keycloak_layer)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());

    // Unprotected endpoints (no Keycloak middleware)
    let router = Router::new()
        .route("/wad/{id}/images", get(internal::list_wad_images))
        .route(
            "/wad/{id}/maps/{map}/images",
            get(internal::list_wad_map_images),
        )
        .with_state(app_state)
        .layer(middleware::from_fn(access_log::public))
        .layer(cors::dev());
    let port = args.public_port;
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
