use clap::Parser;

/// Command line arguments for the websocket server.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[arg(long, env = "PORT", default_value_t = 3000)]
    pub port: u16,

    #[command(flatten)]
    pub nats: dorch_common::args::NatsArgs,

    #[command(flatten)]
    pub redis: dorch_common::args::RedisArgs,

    #[command(flatten)]
    pub kc: dorch_common::args::KeycloakArgs,
}
