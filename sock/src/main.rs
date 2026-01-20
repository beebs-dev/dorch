use anyhow::Result;
use async_nats::ConnectOptions;
use clap::Parser;
use common::AppState;
use dorch_common::{rate_limit::RateLimiter, shutdown::shutdown_signal};
use owo_colors::OwoColorize;
use tokio::join;
use tokio_util::sync::CancellationToken;

mod args;
mod auth;
mod common;
#[allow(dead_code)]
mod jwt;
mod keycloak;
mod payload;
mod server;
mod ws;

#[tokio::main]
async fn main() -> Result<()> {
    dorch_common::init();
    let cli = args::Cli::parse();
    let port = cli.port;
    let keycloak = cli.kc.clone();
    let (pool, nats) = join!(dorch_common::redis::init_redis(&cli.redis), async move {
        println!(
            "{} {}",
            "🔌 Connecting to NATS • url=".green(),
            cli.nats.nats_url.green().dimmed()
        );
        async_nats::connect_with_options(
            &cli.nats.nats_url,
            ConnectOptions::new().user_and_password("app".into(), "devpass".into()),
        )
        .await
        .expect("Failed to connect to NATS")
    });
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let rate_limiter = RateLimiter::new(pool.clone(), cli.rate_limiter.into());
    let app_state = AppState::new(cancel.clone(), pool, nats, cli.redis, cli.kc).await;
    let result = server::run(cancel, port, app_state.clone(), keycloak, rate_limiter).await;
    if let Err(e) = app_state.shutdown().await {
        eprintln!(
            "{}{:?}",
            "Error during shutdown: ".red(),
            format!("{:?}", e).red().dimmed()
        );
    }
    result
}
