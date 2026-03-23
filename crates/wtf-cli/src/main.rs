#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wtf_cli::admin::{run_rebuild_views, RebuildViewsConfig};
use wtf_cli::lint::{explain_rule, run_lint, OutputFormat};
use wtf_cli::serve::{run_serve, ServeConfig};

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
        #[arg(long)]
        check: bool,
        #[arg(long, value_name = "RULE")]
        explain: Option<String>,
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
        Commands::Lint {
            paths,
            format,
            check,
            explain,
        } => handle_lint(paths, format, check, explain).await,
        Commands::Admin { command } => handle_admin(command).await,
    }
}

async fn handle_serve(config: ServeConfig) -> anyhow::Result<std::process::ExitCode> {
    run_serve(config).await.context("serve command failed")?;
    Ok(std::process::ExitCode::SUCCESS)
}

async fn handle_lint(
    paths: Vec<String>,
    format_str: String,
    check: bool,
    explain: Option<String>,
) -> anyhow::Result<std::process::ExitCode> {
    if let Some(rule) = explain {
        if let Some(explanation) = explain_rule(&rule) {
            println!("{explanation}");
            return Ok(std::process::ExitCode::SUCCESS);
        } else {
            eprintln!("unknown rule: {rule}");
            return Ok(std::process::ExitCode::from(2));
        }
    }

    if paths.is_empty() {
        anyhow::bail!("at least one path required");
    }

    let output_format = match format_str.as_str() {
        "human" | "text" => OutputFormat::Human,
        "json" => OutputFormat::Json,
        _ => OutputFormat::Human,
    };

    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let exit_code =
        run_lint(&path_bufs, output_format, check).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(exit_code)
}

async fn handle_admin(cmd: AdminCommands) -> anyhow::Result<std::process::ExitCode> {
    let config = RebuildViewsConfig::from(&cmd);
    run_rebuild_views(config)
        .await
        .context("rebuild-views command failed")
}
