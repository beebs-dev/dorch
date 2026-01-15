use anyhow::{Context, Result};
use clap::Parser;
use dorch_common::shutdown::shutdown_signal;
use owo_colors::OwoColorize;
use tokio_util::sync::CancellationToken;

use crate::{app::App, args::Commands, db::Database};

pub mod app;
pub mod args;
pub mod client;
pub mod db;
pub mod dispatch;
pub mod server;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = args::Cli::parse();
    dorch_common::init();
    dorch_common::metrics::maybe_spawn_metrics_server();
    match cli.command {
        Commands::Server(args) => run_servers(args).await,
        Commands::DispatchImages(args) => dispatch::images::run(args).await,
        Commands::DispatchAnalysis(args) => dispatch::analysis::run(args).await,
    }
}

async fn run_servers(args: args::ServerArgs) -> Result<()> {
    let pool = dorch_common::postgres::create_pool(args.postgres.clone()).await;
    let db = Database::new(pool)
        .await
        .context("Failed to create database")?;
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let cancel_clone = cancel.clone();
    let args_clone = args.clone();
    let args_clone2 = args.clone();
    let app_state = App::new(cancel.clone(), db);
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
