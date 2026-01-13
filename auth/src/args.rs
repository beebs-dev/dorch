use clap::{Parser, Subcommand};
use dorch_common::args::{KeycloakArgs, NatsArgs, RedisArgs};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Server(ServerArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct ServerArgs {
    #[arg(long, env = "PORT", default_value_t = 6000)]
    pub port: u16,

    #[command(flatten)]
    pub redis: RedisArgs,
}
