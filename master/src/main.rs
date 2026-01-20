use crate::{app::App, args::Commands};
use anyhow::{Context, Result};
use async_nats::ConnectOptions;
use clap::Parser;
use dorch_auth::client::Client as AuthClient;
use dorch_common::shutdown::shutdown_signal;
use kube::client::Client;
use owo_colors::OwoColorize;
use tokio_util::sync::CancellationToken;

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
    let auth = AuthClient::new(args.auth_endpoint);
    let pool = dorch_common::redis::init_redis(&args.redis).await;
    let store = store::GameInfoStore::new(pool);
    let nats = async_nats::connect_with_options(
        &args.nats.nats_url,
        ConnectOptions::new().user_and_password(args.nats.nats_user, args.nats.nats_password),
    )
    .await
    .context("Failed to connect to NATS")?;
    let cancel = CancellationToken::new();
    let state = App::new(
        cancel.clone(),
        nats,
        client,
        args.namespace,
        store,
        auth,
        args.game_resource_prefix,
    );
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            shutdown_signal().await;
            cancel.cancel();
        }
    });
    let mut internal_join = Box::pin(tokio::spawn({
        let cancel = cancel.clone();
        let port = args.internal_port;
        let state = state.clone();
        async move { server::internal::run_server(cancel, port, state).await }
    }));
    let mut pub_join = Box::pin(tokio::spawn({
        let cancel = cancel.clone();
        let port = args.public_port;
        let kc = args.kc;
        async move { server::public::run_server(cancel, port, kc, state).await }
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
