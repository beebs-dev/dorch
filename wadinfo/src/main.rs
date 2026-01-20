use anyhow::{Context, Result};
use clap::Parser;
use dorch_common::{rate_limit::RateLimiter, shutdown::shutdown_signal};
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
        Commands::Dispatch(dispatch_args) => match dispatch_args.command {
            crate::args::DispatchCommands::Images(cmd) => match cmd.action {
                None => {
                    let run_args = crate::args::DispatchImagesRunArgs {
                        nats: cmd.nats,
                        postgres: cmd.postgres,
                    };
                    dispatch::images::run(run_args).await
                }
                Some(crate::args::DispatchImagesAction::Clear) => {
                    let deleted = dispatch::images::clear(cmd.postgres).await?;
                    println!("Deleted {deleted} rows from wad_dispatch_images");
                    Ok(())
                }
                Some(crate::args::DispatchImagesAction::Prune(s3)) => {
                    let deleted = dispatch::images::prune(cmd.postgres, s3).await?;
                    println!("Pruned {deleted} rows from wad_dispatch_images");
                    Ok(())
                }
            },
            crate::args::DispatchCommands::Analysis(cmd) => match cmd.action {
                None => {
                    let run_args = crate::args::DispatchAnalysisRunArgs {
                        nats: cmd.nats,
                        postgres: cmd.postgres,
                    };
                    dispatch::analysis::run(run_args).await
                }
                Some(crate::args::DispatchAnalysisAction::Clear) => {
                    let deleted = dispatch::analysis::clear(cmd.postgres).await?;
                    println!("Deleted {deleted} rows from wad_dispatch_analysis");
                    Ok(())
                }
                Some(crate::args::DispatchAnalysisAction::Prune(s3)) => {
                    let deleted = dispatch::analysis::prune(cmd.postgres, s3).await?;
                    println!("Pruned {deleted} rows from wad_dispatch_analysis");
                    Ok(())
                }
            },
        },
        Commands::DispatchImages(args) => dispatch::images::run(args).await,
        Commands::DispatchAnalysis(args) => dispatch::analysis::run(args).await,
    }
}

async fn run_servers(args: args::ServerArgs) -> Result<()> {
    println!("{}", "Connecting to postgres...".green());
    let pool = dorch_common::postgres::create_pool(args.postgres.clone()).await;
    println!(
        "{}",
        "Connected to postgres. Initializing database...".green()
    );
    let db = Database::new(pool)
        .await
        .context("Failed to create database")?;
    println!("{}", "Database initialized.".green());
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let app_state = App::new(cancel.clone(), db);
    let pool = dorch_common::redis::init_redis(&args.redis).await;
    let rate_limiter = RateLimiter::new(pool, args.rate_limiter.into());
    let mut internal_join = Box::pin(tokio::spawn({
        let port = args.internal_port;
        let cancel = cancel.clone();
        let rate_limiter = rate_limiter.clone();
        let app_state = app_state.clone();
        async move { server::internal::run_server(cancel, port, app_state, rate_limiter).await }
    }));
    let mut pub_join = Box::pin(tokio::spawn({
        let port = args.public_port;
        let cancel = cancel.clone();
        async move { server::public::run_server(cancel, port, args.kc, app_state, rate_limiter).await }
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
