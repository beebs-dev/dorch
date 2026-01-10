use anyhow::Result;
use clap::Parser;

mod args;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    dorch_common::init();
    match args::Cli::parse().command {
        args::Commands::Server(args) => server::run(*args).await,
    }
}
