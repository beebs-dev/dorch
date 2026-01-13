use anyhow::Result;
use clap::Parser;
use common::AppState;
use dorch_common::shutdown::shutdown_signal;
use owo_colors::OwoColorize;
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
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let app_state = AppState::new(cancel.clone(), cli).await;
    let result = server::run(cancel, port, app_state.clone(), keycloak).await;
    if let Err(e) = app_state.shutdown().await {
        eprintln!(
            "{}{:?}",
            "Error during shutdown: ".red(),
            format!("{:?}", e).red().dimmed()
        );
    }
    result
}
