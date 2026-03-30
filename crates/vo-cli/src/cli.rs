use clap::Command;

#[derive(Debug)]
pub enum CliError {
    Clap(clap::Error),
    InvalidNumeric(String),
    InvalidNatsUrl(String),
    Dispatch(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct NatsUrl {
    pub host: String,
    pub port: Option<u16>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Cli {
    pub command: String,
}

/// Interpret CLI arguments from an iterator.
///
/// # Errors
/// Returns `clap::Error` if the arguments fail to parse.
pub fn interpret_cli_from<I, T>(args: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cmd = Command::new("vo")
        .version("0.1.0")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("start"));

    let matches = cmd.try_get_matches_from(args)?;

    if let Some((name, _)) = matches.subcommand() {
        Ok(Cli {
            command: name.to_string(),
        })
    } else {
        Ok(Cli {
            command: String::new(),
        })
    }
}

#[must_use]
pub fn map_error_to_exit_code(err: &CliError) -> i32 {
    match err {
        CliError::Clap(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            | clap::error::ErrorKind::DisplayVersion => 0,
            _ => 2,
        },
        CliError::Dispatch(_) => 1,
        CliError::InvalidNumeric(_) | CliError::InvalidNatsUrl(_) => 2,
    }
}
