use clap::Parser;
use dorch_common::args::{NatsArgs, RedisArgs};

/// Command line arguments for the proxy
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) enum Cli {
    Wad(WadArgs),
    Map(MapArgs),
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct SummaryOpts {
    #[arg(long, env = "MAX_TEXT_KB", default_value_t = 50)]
    pub max_text_kb: usize,

    #[arg(long, env = "MAX_MESSAGES", default_value_t = 60)]
    pub max_messages: usize,

    #[arg(long, env = "MAX_TOOL_CALLS", default_value_t = 15)]
    pub max_tool_calls: usize,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct WadArgs {
    #[arg(long, env = "NODE_ID", required = true)]
    pub node_id: String,

    #[arg(long, env = "ENDPOINT", required = true)]
    pub endpoint: String,

    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub redis: RedisArgs,

    #[command(flatten)]
    pub summary: SummaryOpts,

    #[arg(env = "OPENAI_API_KEY")]
    pub openai_api_key: String,

    #[arg(env = "OPENAI_BASE_URL", default_value = None)]
    pub openai_base_url: Option<String>,

    #[arg(long, env = "MODEL", default_value = "gpt-4.1-mini")]
    pub model: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct MapArgs {
    #[arg(long, env = "NODE_ID", required = true)]
    pub node_id: String,

    #[arg(long, env = "ENDPOINT", required = true)]
    pub endpoint: String,

    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub redis: RedisArgs,

    #[command(flatten)]
    pub summary: SummaryOpts,

    #[arg(env = "OPENAI_API_KEY")]
    pub openai_api_key: String,

    #[arg(env = "OPENAI_BASE_URL", default_value = None)]
    pub openai_base_url: Option<String>,

    #[arg(long, env = "MODEL", default_value = "gpt-4.1-mini")]
    pub model: String,
}
