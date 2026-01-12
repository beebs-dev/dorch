use crate::{app::App, args::Commands};
use anyhow::Result;
use clap::Parser;
use dorch_common::shutdown::shutdown_signal;
use tokio_util::sync::CancellationToken;

pub mod app;
pub mod args;
pub mod server;
pub mod token;

#[tokio::main]
async fn main() -> Result<()> {
    dorch_common::init();
    dorch_common::metrics::maybe_spawn_metrics_server();
    let cli = args::Cli::parse();
    match cli.command {
        Commands::Server(args) => run_server(args).await,
    }
}

async fn run_server(args: args::ServerArgs) -> Result<()> {
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let app_state = App::new(
        cancel.clone(),
        args.api_key.clone(),
        args.api_secret.clone(),
        args.external_livekit_url.clone(),
    );
    server::run(cancel, args, app_state).await
}
