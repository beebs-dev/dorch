use clap::{Parser, Subcommand};
use dorch_common::args::{KeycloakArgs, PostgresArgs};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Server(ServerArgs),

    /// Dispatch management and dispatcher runners.
    Dispatch(DispatchArgs),

    /// Dispatch pending WAD image jobs to JetStream.
    #[command(name = "dispatch-images", hide = true)]
    DispatchImages(DispatchImagesRunArgs),

    /// Dispatch pending WAD analysis jobs to JetStream.
    #[command(name = "dispatch-analysis", hide = true)]
    DispatchAnalysis(DispatchAnalysisRunArgs),
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
pub struct DispatchArgs {
    #[command(subcommand)]
    pub command: DispatchCommands,
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchImagesCommand {
    /// NATS connection settings (required for the dispatcher runner).
    #[command(flatten)]
    pub nats: DispatchNatsArgs,

    /// Postgres connection settings.
    #[command(flatten)]
    pub postgres: PostgresArgs,

    #[command(subcommand)]
    pub action: Option<DispatchImagesAction>,
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchAnalysisCommand {
    /// NATS connection settings (required for the dispatcher runner).
    #[command(flatten)]
    pub nats: DispatchNatsArgs,

    /// Postgres connection settings.
    #[command(flatten)]
    pub postgres: PostgresArgs,

    #[command(subcommand)]
    pub action: Option<DispatchAnalysisAction>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DispatchCommands {
    Images(DispatchImagesCommand),
    Analysis(DispatchAnalysisCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum DispatchImagesAction {
    /// Deletes all rows from the wad_dispatch_images table.
    Clear,

    /// Deletes any rows from wad_dispatch_images that don't have a corresponding folder in S3.
    Prune(S3PruneArgs),
}

#[derive(Subcommand, Debug, Clone)]
pub enum DispatchAnalysisAction {
    /// Deletes all rows from the wad_dispatch_analysis table.
    Clear,

    /// Deletes any rows from wad_dispatch_analysis that don't have a corresponding folder in S3.
    Prune(S3PruneArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct S3PruneArgs {
    /// S3 bucket name.
    #[arg(long)]
    pub bucket: String,

    /// S3 region.
    #[arg(long)]
    pub s3_region: String,

    /// Optional S3 endpoint URL (for S3-compatible services).
    #[arg(long)]
    pub s3_endpoint: Option<String>,
}

/// NATS CLI args for dispatchers.
///
/// These are optional at the clap layer so `clear`/`prune` can run without NATS.
#[derive(Parser, Debug, Clone)]
pub struct DispatchNatsArgs {
    #[arg(long, env = "NATS_URL")]
    pub nats_url: Option<String>,

    #[arg(long, env = "NATS_USER", default_value = "app")]
    pub nats_user: String,

    #[arg(long, env = "NATS_PASSWORD", default_value = "devpass")]
    pub nats_password: String,
}

impl DispatchNatsArgs {
    pub fn require(self) -> anyhow::Result<dorch_common::args::NatsArgs> {
        let Some(nats_url) = self.nats_url else {
            anyhow::bail!("NATS_URL is required to run the dispatcher");
        };
        Ok(dorch_common::args::NatsArgs {
            nats_url,
            nats_user: self.nats_user,
            nats_password: self.nats_password,
        })
    }
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchImagesRunArgs {
    #[command(flatten)]
    pub nats: DispatchNatsArgs,

    #[command(flatten)]
    pub postgres: PostgresArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct DispatchAnalysisRunArgs {
    #[command(flatten)]
    pub nats: DispatchNatsArgs,

    #[command(flatten)]
    pub postgres: PostgresArgs,
}
