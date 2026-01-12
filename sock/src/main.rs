use anyhow::Result;
use clap::Parser;
use common::AppState;

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
    let app_state = AppState::new(cli).await;
    server::run(port, app_state, keycloak).await
}
