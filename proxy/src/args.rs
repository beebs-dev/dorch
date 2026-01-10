use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Server(Box<ServerArgs>),
}

#[derive(Parser, Debug, Clone)]
pub struct ServerArgs {
    #[arg(long, env = "GAME_ID", required = true)]
    pub game_id: Uuid,

    #[arg(long, env = "GAME_PORT", default_value_t = 2342)]
    pub game_port: u16,

    #[arg(long, env = "LIVEKIT_URL", required = true)]
    pub livekit_url: String,
}
