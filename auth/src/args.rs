use clap::{Parser, Subcommand};
use dorch_common::args::RedisArgs;

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
    /// TCP port for the JSON/TCP auth server (non-Zandronum protocol).
    #[arg(long, env = "CLIENT_PORT", default_value_t = 3500)]
    pub client_port: u16,

    /// Admin http server port
    #[arg(long, env = "ADMIN_PORT", default_value_t = 2500)]
    pub admin_port: u16,

    /// UDP port for the Zandronum auth protocol (what the game server uses).
    /// Zandronum's default `authhostname` is `auth.zandronum.com:16666`.
    #[arg(long, env = "ZANDRONUM_AUTH_PORT", default_value_t = 16666)]
    pub zandronum_port: u16,

    #[command(flatten)]
    pub redis: RedisArgs,
}
