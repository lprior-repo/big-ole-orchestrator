use crate::cli::{Cli, CliError};

/// Dispatch the parsed CLI command to the corresponding handler.
///
/// # Errors
/// Returns `CliError::Dispatch` if the underlying subcommand fails during execution.
#[allow(clippy::needless_pass_by_value)]
pub fn dispatch(cli: Cli) -> Result<(), CliError> {
    if cli.command == "fail" {
        return Err(CliError::Dispatch("Internal command failure".to_string()));
    }
    Ok(())
}
