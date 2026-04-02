use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Clap(#[from] clap::Error),
    #[error("invalid numeric: {0}")]
    InvalidNumeric(String),
    #[error("invalid NATS URL: {0}")]
    InvalidNatsUrl(String),
    #[error("dispatch error: {0}")]
    Dispatch(String),
    #[error(transparent)]
    Check(#[from] crate::commands::check::CheckError),
    #[error(transparent)]
    Gc(#[from] crate::commands::gc::GcError),
}

#[derive(Debug, PartialEq, Clone)]
pub struct NatsUrl {
    pub host: String,
    pub port: Option<u16>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Start,
    Purge { instance: String },
    Check { path: PathBuf },
    Gc { engine_url: String, dry_run: bool },
}

#[derive(Debug, PartialEq, Clone)]
pub struct Cli {
    pub command: Command,
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
    let cmd = clap::Command::new("vo")
        .version("0.1.0")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(clap::Command::new("start"))
        .subcommand(
            clap::Command::new("purge").arg(
                clap::Arg::new("instance")
                    .long("instance")
                    .required(true)
                    .value_name("ID")
                    .help("The instance ID to purge"),
            ),
        )
        .subcommand(clap::Command::new("check").arg(clap::Arg::new("path").required(true).index(1)))
        .subcommand(
            clap::Command::new("gc")
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .env("VO_ENGINE_URL")
                        .default_value("http://localhost:3000"),
                )
                .arg(
                    clap::Arg::new("dry-run")
                        .long("dry-run")
                        .action(clap::ArgAction::SetTrue),
                ),
        );

    let matches = cmd.try_get_matches_from(args)?;

    match matches.subcommand() {
        Some(("start", _)) => Ok(Cli {
            command: Command::Start,
        }),
        Some(("purge", purge_matches)) => {
            let instance = purge_matches
                .get_one::<String>("instance")
                .cloned()
                .unwrap_or_default();
            Ok(Cli {
                command: Command::Purge { instance },
            })
        }
        Some(("check", sub_matches)) => {
            let path = match sub_matches.get_one::<String>("path") {
                Some(p) => PathBuf::from(p),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            Ok(Cli {
                command: Command::Check { path },
            })
        }
        Some(("gc", sub_matches)) => {
            let engine_url = match sub_matches.get_one::<String>("engine-url") {
                Some(u) => u.clone(),
                None => "http://localhost:3000".to_string(),
            };
            let dry_run = sub_matches.get_flag("dry-run");
            Ok(Cli {
                command: Command::Gc {
                    engine_url,
                    dry_run,
                },
            })
        }
        _ => Err(clap::Error::new(clap::error::ErrorKind::InvalidSubcommand)),
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
        CliError::Dispatch(_) | CliError::Check(_) | CliError::Gc(_) => 1,
        CliError::InvalidNumeric(_) | CliError::InvalidNatsUrl(_) => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn cli_purges_matches_when_purge_subcommand_provided() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "purge".into(),
            "--instance".into(),
            "123".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Purge {
                instance: "123".to_string()
            }
        );
    }
}
