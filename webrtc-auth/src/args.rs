use clap::{Parser, Subcommand};

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
    #[arg(long, env = "PORT", default_value_t = 80)]
    pub port: u16,

    /// JWT API Key for authenticating users with LiveKit
    #[arg(long, env = "API_KEY", required = true)]
    pub api_key: String,

    /// JWT API Secret for authenticating users with LiveKit
    #[arg(long, env = "API_SECRET", required = true)]
    pub api_secret: String,

    /// External LiveKit URL to provide to clients
    #[arg(
        long,
        env = "EXTERNAL_LIVEKIT_URL",
        default_value = "wss://webrtc.beebs.dev"
    )]
    pub external_livekit_url: String,
}
