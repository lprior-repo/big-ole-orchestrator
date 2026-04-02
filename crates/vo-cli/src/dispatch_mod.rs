use std::path::PathBuf;

use crate::cli::{Cli, CliError, Command};
use vo_storage::codec::StorageError;
use vo_storage::purge::purge_instance;

/// Dispatch the parsed CLI command to the corresponding handler.
///
/// # Errors
/// Returns `CliError` if the underlying subcommand fails during execution.
#[allow(clippy::needless_pass_by_value)]
pub async fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Start {
            port,
            nats_url,
            embedded_nats,
            data_dir,
            max_concurrent,
        } => {
            let config = crate::commands::serve::ServeConfig {
                port,
                nats_url,
                embedded_nats,
                data_dir,
                max_concurrent,
            };
            let nats = crate::commands::serve::run_serve(config.clone())
                .await
                .map_err(|e| CliError::Dispatch(format!("Failed to provision storage: {e}")))?;

            crate::commands::serve::run_serve_loop(config, nats)
                .await
                .map_err(|e| CliError::Dispatch(format!("Server error: {e}")))?;
            Ok(())
        }
        Command::Purge { instance } => {
            let fjall_path = "/home/lewis/.gemini/tmp/veloxide/fjall";
            let keyspace = fjall::Config::new(fjall_path)
                .open()
                .map_err(|e| CliError::Dispatch(format!("Failed to open keyspace: {e}")))?;

            match purge_instance(&keyspace, &instance) {
                Ok(count) => {
                    println!("Purged {count} events for instance {instance}.");
                    Ok(())
                }
                Err(StorageError::InstanceRunning) => {
                    eprintln!("Cannot purge a running instance.");
                    Err(CliError::Dispatch("Instance is running".to_string()))
                }
                Err(e) => Err(CliError::Dispatch(format!("Purge failed: {e}"))),
            }
        }
        Command::Check { path } => {
            crate::commands::check::run_check(&path)?;
            Ok(())
        }
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            let config = crate::commands::gc::GcConfig {
                engine_url,
                versions_dir: PathBuf::from("/var/wtf/versions"),
                dry_run,
            };
            crate::commands::gc::run_gc(&config).await?;
            Ok(())
        }
    }
}
