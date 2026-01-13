use crate::{args::Commands, user_record_store::UserRecordStore};
use anyhow::{Context, Result};
use clap::Parser;
use dorch_common::shutdown::shutdown_signal;
use owo_colors::OwoColorize;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub mod admin;
pub mod args;
pub mod keys;
pub mod protocol;
pub mod server;
pub mod srp;
pub mod user_record_store;
pub mod zandronum;
pub mod zandronum_srp_sha256;

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
    let store = UserRecordStore::new(pool.clone());

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        cancel_clone.cancel();
    });

    // Start the Zandronum UDP auth protocol server.
    let udp_bind = format!("0.0.0.0:{}", args.zandronum_port);
    println!(
        "{}{}",
        "🕹️  Zandronum auth UDP listening on ".green(),
        udp_bind.green().dimmed(),
    );
    tokio::spawn({
        let store = store.clone();
        let cancel = cancel.clone();
        async move {
            if let Err(e) = zandronum::run_udp(&udp_bind, store, cancel).await {
                panic!("zandronum udp server error: {:?}", e);
            }
        }
    });
    tokio::spawn({
        let store = store.clone();
        let cancel = cancel.clone();
        async move {
            if let Err(e) = admin::run(store, args.admin_port, cancel).await {
                panic!("admin server error: {:?}", e);
            }
        }
    });

    // Start the legacy JSON/TCP auth server.
    let listen = format!("0.0.0.0:{}", args.client_port);
    let listener = TcpListener::bind(&listen)
        .await
        .with_context(|| format!("Failed to bind to {}", &listen))?;
    println!(
        "{}{}",
        "🔐 Auth TCP listening on ".green(),
        listen.green().dimmed(),
    );
    server::run_listener(listener, store, cancel).await?;
    println!("{}", "🛑 All servers shut down gracefully.".red());
    Ok(())
}
