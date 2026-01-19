use anyhow::Result;
use clap::Parser;

mod args;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    dorch_common::init();
    match args::Cli::parse() {
        args::Cli::Wad(args) => worker::wad::run(args).await,
        args::Cli::Map(args) => worker::map::run(args).await,
    }
}
