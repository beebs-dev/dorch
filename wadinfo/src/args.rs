use clap::{Parser, Subcommand};
use dorch_common::args::{KeycloakArgs, NatsArgs, PostgresArgs};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Server(ServerArgs),

    /// Dispatch pending WAD image jobs to JetStream.
    DispatchImages(DispatchImagesArgs),

    /// Dispatch pending WAD analysis jobs to JetStream.
    DispatchAnalysis(DispatchAnalysisArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct ServerArgs {
    #[arg(long, env = "INTERNAL_PORT", default_value_t = 80)]
    pub internal_port: u16,

    #[arg(long, env = "PUBLIC_PORT", default_value_t = 3000)]
    pub public_port: u16,

    #[command(flatten)]
    pub kc: KeycloakArgs,

    #[command(flatten)]
    pub postgres: PostgresArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchImagesArgs {
    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub postgres: PostgresArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchAnalysisArgs {
    #[command(flatten)]
    pub nats: NatsArgs,

    #[command(flatten)]
    pub postgres: PostgresArgs,
}
