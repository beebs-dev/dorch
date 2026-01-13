use crate::args::Commands;
use anyhow::{Context, Result};
use clap::Parser;
use dorch_common::shutdown::shutdown_signal;
use owo_colors::OwoColorize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub mod args;
pub mod protocol;
pub mod server;
pub mod srp;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = args::Cli::parse();
    dorch_common::init();
    dorch_common::metrics::maybe_spawn_metrics_server();
    match cli.command {
        Commands::Server(args) => run(args).await,
    }
}

async fn run(args: args::ServerArgs) -> Result<()> {
    let pool = dorch_common::redis::init_redis(&args.redis).await;
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });
    let listen = format!("0.0.0.0:{}", args.port);
    let listener = TcpListener::bind(&listen)
        .await
        .with_context(|| format!("Failed to bind to {}", &listen))?;
    println!(
        "{}{}",
        "🔐 Auth server listening on ".green(),
        listen.green().dimmed(),
    );
    server::run_listener(listener, pool, cancel).await?;
    println!("{}", "🛑 All servers shut down gracefully.".red());
    Ok(())
}
