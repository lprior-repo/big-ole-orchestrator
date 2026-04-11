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
        Command::Init { project_dir, engine_url, storage_path } => {
            let config = crate::commands::init::InitConfig {
                project_dir,
                engine_url,
                storage_path,
            };
            let vo_dir = crate::commands::init::run_init(&config)?;
            println!("Initialized veloxide project at {}", vo_dir.display());
            Ok(())
        }
        Command::Lock { project_dir } => {
            let config = crate::commands::lock::LockConfig { project_dir };
            let lockmap = crate::commands::lock::run_lock(&config)?;
            println!("Locked {} workflow(s):", lockmap.len());
            for (name, hash) in &lockmap {
                println!("  {name} {hash}");
            }
            Ok(())
        }
        Command::Doctor { project_dir } => {
            let config = crate::commands::doctor::DoctorConfig { project_dir };
            let report = crate::commands::doctor::run_doctor(&config)?;
            if report.healthy {
                println!("Project is healthy.");
            } else {
                eprintln!("Found {} issue(s):", report.issues.len());
                for issue in &report.issues {
                    eprintln!("  - {issue}");
                }
            }
            Ok(())
        }
    }
}
