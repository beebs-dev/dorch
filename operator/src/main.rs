use clap::{Parser, Subcommand};
use kube::client::Client;

mod games;
mod util;

#[cfg(feature = "metrics")]
mod metrics;

/// Top-level CLI configuration for the binary. Any command line
/// flags should go in here.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Prometheus metrics server scrape port. Disabled by default.
    #[cfg(feature = "metrics")]
    #[arg(long, env = "METRICS_PORT")]
    metrics_port: Option<u16>,
}

/// List of subcommands for the binary. Clap will convert the
/// name of each enum variant to kebab-case for the CLI.
/// e.g. `ManageConsumers` becomes `manage-consumers`.
#[derive(Subcommand)]
enum Command {
    ManageGames {
        #[arg(
            long,
            env = "DORCH_DOWNLOADER_IMAGE",
            default_value = "thavlik/dorch-downloader:latest"
        )]
        downloader_image: String,

        #[arg(
            long,
            env = "DORCH_PROXY_IMAGE",
            default_value = "thavlik/dorch-proxy:latest"
        )]
        proxy_image: String,

        #[arg(
            long,
            env = "DORCH_SERVER_IMAGE",
            default_value = "thavlik/zandronum-server:latest"
        )]
        server_image: String,

        #[arg(long, env = "LIVEKIT_URL", required = true)]
        livekit_url: String,

        #[arg(long, env = "LIVEKIT_SECRET", required = true)]
        livekit_secret: String,

        #[arg(long, env = "WADINFO_BASE_URL", required = true)]
        wadinfo_base_url: String,
    },
}

/// Secondary entrypoint that runs the appropriate subcommand.
async fn run(client: Client) {
    let cli = Cli::parse();

    #[cfg(feature = "metrics")]
    if let Some(metrics_port) = cli.metrics_port {
        tokio::spawn(metrics::run_server(metrics_port));
    }

    match cli.command {
        Command::ManageGames {
            proxy_image,
            server_image,
            livekit_url,
            livekit_secret,
            downloader_image,
            wadinfo_base_url,
        } => {
            games::run(
                client,
                proxy_image,
                downloader_image,
                server_image,
                livekit_url,
                livekit_secret,
                wadinfo_base_url,
            )
            .await
        }
    }
    .unwrap();

    panic!("exited unexpectedly");
}

/// Main entrypoint that sets up the environment before running the secondary entrypoint `run`.
#[tokio::main]
async fn main() {
    dorch_common::init();

    // Set the panic hook to exit the process with a non-zero exit code
    // when a panic occurs on any thread. This is desired behavior when
    // running in a container, as the metrics server or controller may
    // panic and we always want to restart the container in that case.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));

    // Create a kubernetes client using the default configuration.
    // In-cluster, the kubeconfig will be set by the service account.
    let client: Client = Client::try_default()
        .await
        .expect("Expected a valid KUBECONFIG environment variable.");

    // Run the secondary entrypoint.
    run(client).await;

    // This is an unreachable branch. The controllers and metrics
    // servers should never exit without a panic.
    panic!("exited prematurely");
}
