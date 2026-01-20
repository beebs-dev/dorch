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
    #[arg(long, env = "INTERNAL_PORT", default_value_t = 80)]
    pub internal_port: u16,

    #[arg(long, env = "PUBLIC_PORT", default_value_t = 3000)]
    pub public_port: u16,

    #[command(flatten)]
    pub redis: RedisArgs,

    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub kc: KeycloakArgs,

    #[arg(long, env = "NAMESPACE", default_value = "default")]
    pub namespace: String,

    #[arg(long, env = "AUTH_ENDPOINT", required = true)]
    pub auth_endpoint: String,

    /// The prefix applied to Game resource names in Kubernetes
    /// For example, if the prefix is "game", a game with ID "1234" will correspond to a Kubernetes resource named "game-1234".
    #[arg(long, env = "GAME_RESOURCE_PREFIX", default_value = "game")]
    pub game_resource_prefix: String,
}
