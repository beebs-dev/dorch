use anyhow::{Context, Result};
use async_nats::ConnectOptions;
use clap::Parser;
use dorch_common::shutdown::shutdown_signal;
use kube::client::Client;
use owo_colors::OwoColorize;
use tokio_util::sync::CancellationToken;

use crate::{app::App, args::Commands};

pub mod app;
pub mod args;
pub mod client;
pub mod server;
pub mod store;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = args::Cli::parse();
    dorch_common::init();
    dorch_common::metrics::maybe_spawn_metrics_server();
    match cli.command {
        Commands::Server(args) => run_servers(args).await,
    }
}

async fn run_servers(args: args::ServerArgs) -> Result<()> {
    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");
    let pool = dorch_common::redis::init_redis(&args.redis).await;
    let store = store::GameInfoStore::new(pool);
    let nats = async_nats::connect_with_options(
        &args.nats.nats_url,
        ConnectOptions::new()
            .user_and_password(args.nats.nats_user.clone(), args.nats.nats_password.clone()),
    )
    .await
    .context("Failed to connect to NATS")?;
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let cancel_clone = cancel.clone();
    let args_clone = args.clone();
    let args_clone2 = args.clone();
    let app_state = App::new(cancel.clone(), nats, client, args.namespace, store);
    let app_state_clone = app_state.clone();
    let mut internal_join = Box::pin(tokio::spawn(async move {
        server::internal::run_server(cancel_clone, args_clone2, app_state_clone).await
    }));
    let cancel_clone = cancel.clone();
    let app_state_clone = app_state.clone();
    let mut pub_join = Box::pin(tokio::spawn(async move {
        server::public::run_server(cancel_clone, args_clone, app_state_clone).await
    }));
    tokio::select! {
        res = &mut internal_join => {
            cancel.cancel();
            pub_join
                .await
                .context("Failed to join public server task")?
                .context("Public server task failed")?;
            res.context("Internal server task failed")?.context("Failed to join internal server task")?;
        }
        res = &mut pub_join => {
            cancel.cancel();
            internal_join
                .await
                .context("Failed to join internal server task")?
                .context("Internal server task failed")?;
            res.context("Public server task failed")?.context("Failed to join public server task")?;
        }
    }
    println!("{}", "🛑 All servers shut down gracefully.".red());
    Ok(())
}
