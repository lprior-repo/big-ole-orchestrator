#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wtf_cli::admin::{run_rebuild_views, RebuildViewsConfig};
use wtf_cli::serve::{run_serve, run_serve_loop, ServeConfig};

#[derive(Parser)]
#[command(name = "wtf")]
#[command(version = "0.1.0")]
#[command(about = "wtf workflow engine CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, default_value_t = 4222)]
        port: u16,
        #[arg(long, default_value = "nats://127.0.0.1:4222")]
        nats_url: String,
        #[arg(long)]
        embedded_nats: bool,
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long, default_value_t = 100)]
        max_concurrent: usize,
    },
    Lint {
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        #[arg(long, default_value = "human")]
        format: String,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommands,
    },
}

#[derive(Subcommand)]
enum AdminCommands {
    RebuildViews {
        #[arg(long)]
        view: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long, default_value_t = true)]
        progress: bool,
        #[arg(long)]
        dry_run: bool,
    },
}

impl From<&AdminCommands> for RebuildViewsConfig {
    fn from(cmd: &AdminCommands) -> Self {
        match cmd {
            AdminCommands::RebuildViews {
                view,
                namespace,
                progress,
                dry_run,
            } => RebuildViewsConfig {
                view: view.clone(),
                namespace: namespace.clone(),
                show_progress: *progress,
                dry_run: *dry_run,
            },
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    let cli = Cli::parse();
    tracing_subscriber::fmt::init();
    handle_command(cli.command).await
}

async fn handle_command(cmd: Commands) -> anyhow::Result<std::process::ExitCode> {
    match cmd {
        Commands::Serve {
            port,
            nats_url,
            embedded_nats,
            data_dir,
            max_concurrent,
        } => {
            let config = ServeConfig {
                port,
                nats_url,
                embedded_nats,
                data_dir,
                max_concurrent,
            };
            handle_serve(config).await
        }
        Commands::Lint { paths, .. } => handle_lint(paths).await,
        Commands::Admin { command } => handle_admin(command).await,
    }
}

async fn handle_serve(config: ServeConfig) -> anyhow::Result<std::process::ExitCode> {
    let nats = run_serve(config.clone())
        .await
        .context("failed to provision NATS storage")?;

    run_serve_loop(config, nats).await
}

async fn handle_lint(paths: Vec<String>) -> anyhow::Result<std::process::ExitCode> {
    if paths.is_empty() {
        anyhow::bail!("at least one path required");
    }
    anyhow::bail!("lint command not yet implemented in this bead")
}

async fn handle_admin(cmd: AdminCommands) -> anyhow::Result<std::process::ExitCode> {
    let config = RebuildViewsConfig::from(&cmd);
    run_rebuild_views(config).await.context("rebuild-views command failed")
}