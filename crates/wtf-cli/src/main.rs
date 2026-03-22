#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use wtf_cli::admin::{run_rebuild_views, RebuildViewsConfig};

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
    match cli.command {
        Commands::Lint { paths, .. } => {
            if paths.is_empty() {
                anyhow::bail!("at least one path required");
            }
            tracing_subscriber::fmt::init();
            anyhow::bail!("lint command not yet implemented in this bead")
        }
        Commands::Admin { command } => {
            tracing_subscriber::fmt::init();
            let config = RebuildViewsConfig::from(&command);
            run_rebuild_views(config).await.context("rebuild-views command failed")
        }
    }
}
