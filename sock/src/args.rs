use clap::Parser;
use dorch_common::args::{KeycloakArgs, NatsArgs, RateLimiterArgs, RedisArgs};

/// Command line arguments for the websocket server.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[arg(long, env = "PORT", default_value_t = 3000)]
    pub port: u16,

    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub redis: RedisArgs,

    #[command(flatten)]
    pub kc: KeycloakArgs,

    #[command(flatten)]
    pub rate_limiter: RateLimiterArgs,
}
