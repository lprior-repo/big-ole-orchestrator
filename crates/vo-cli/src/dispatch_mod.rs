use std::path::PathBuf;

use crate::cli::{Cli, CliError, Command};

/// Dispatch the parsed CLI command to the corresponding handler.
///
/// # Errors
/// Returns `CliError` if the underlying subcommand fails during execution.
#[allow(clippy::needless_pass_by_value)]
pub async fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Start => Ok(()),
        Command::Purge { instance: _ } => Ok(()),
        Command::Check { path } => {
            crate::commands::check::run_check(&path)?;
            Ok(())
        }
        Command::Gc { engine_url, dry_run } => {
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
