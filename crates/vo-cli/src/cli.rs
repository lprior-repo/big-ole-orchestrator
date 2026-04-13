use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Clap(#[from] clap::Error),
    #[error("invalid numeric: {0}")]
    InvalidNumeric(String),
    #[error("dispatch error: {0}")]
    Dispatch(String),
    #[error(transparent)]
    Check(#[from] crate::commands::check::CheckError),
    #[error(transparent)]
    Gc(#[from] crate::commands::gc::GcError),
    #[error(transparent)]
    Init(#[from] crate::commands::init::InitError),
    #[error(transparent)]
    Lock(#[from] crate::commands::lock::LockError),
    #[error(transparent)]
    Doctor(#[from] crate::commands::doctor::DoctorError),
    #[error(transparent)]
    Rebuild(#[from] crate::commands::rebuild::RebuildError),
    #[error(transparent)]
    Unquarantine(#[from] crate::commands::unquarantine::UnquarantineError),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Purge {
        instance: String,
    },
    Check {
        path: PathBuf,
    },
    Gc {
        engine_url: String,
        dry_run: bool,
    },
    Init {
        project_dir: PathBuf,
        engine_url: String,
        storage_path: PathBuf,
    },
    Lock {
        project_dir: PathBuf,
    },
    Doctor {
        project_dir: PathBuf,
    },
    Unquarantine {
        workflow_name: String,
        operator: String,
        engine_url: String,
    },
    Rebuild {
        projection_id: String,
        storage_path: PathBuf,
        from_sequence: Option<u64>,
        to_sequence: Option<u64>,
        cancel_file: Option<PathBuf>,
        dry_run: bool,
    },
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
        )
        .subcommand(
            clap::Command::new("init")
                .arg(
                    clap::Arg::new("project-dir")
                        .long("project-dir")
                        .default_value(".")
                        .help("Project directory to initialize"),
                )
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .default_value("http://localhost:3000")
                        .help("Engine URL"),
                )
                .arg(
                    clap::Arg::new("storage-path")
                        .long("storage-path")
                        .default_value(".vo/storage")
                        .help("Storage path"),
                ),
        )
        .subcommand(
            clap::Command::new("lock").arg(
                clap::Arg::new("project-dir")
                    .long("project-dir")
                    .default_value(".")
                    .help("Project directory"),
            ),
        )
        .subcommand(
            clap::Command::new("doctor").arg(
                clap::Arg::new("project-dir")
                    .long("project-dir")
                    .default_value(".")
                    .help("Project directory to diagnose"),
            ),
        )
        .subcommand(
            clap::Command::new("unquarantine")
                .about("Manually unquarantine a workflow (ADR-026)")
                .arg(
                    clap::Arg::new("workflow-name")
                        .required(true)
                        .value_name("WORKFLOW_NAME")
                        .help("The workflow name to unquarantine"),
                )
                .arg(
                    clap::Arg::new("operator")
                        .long("operator")
                        .required(true)
                        .value_name("OPERATOR")
                        .help("The operator performing the unquarantine"),
                )
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .env("VO_ENGINE_URL")
                        .default_value("http://localhost:3000"),
                ),
        )
        .subcommand(
            clap::Command::new("rebuild")
                .about("Rebuild a projection from event log")
                .arg(
                    clap::Arg::new("projection-id")
                        .required(true)
                        .value_name("PROJECTION_ID")
                        .help("The projection ID to rebuild"),
                )
                .arg(
                    clap::Arg::new("storage-path")
                        .long("storage-path")
                        .default_value(".vo/storage")
                        .help("Storage path"),
                )
                .arg(
                    clap::Arg::new("from-sequence")
                        .long("from-sequence")
                        .value_name("SEQ")
                        .help("Start sequence number (inclusive)"),
                )
                .arg(
                    clap::Arg::new("to-sequence")
                        .long("to-sequence")
                        .value_name("SEQ")
                        .help("End sequence number (inclusive)"),
                )
                .arg(
                    clap::Arg::new("cancel-file")
                        .long("cancel-file")
                        .value_name("PATH")
                        .help("File path to signal cancellation"),
                )
                .arg(
                    clap::Arg::new("dry-run")
                        .long("dry-run")
                        .action(clap::ArgAction::SetTrue),
                ),
        );

    let matches = cmd.try_get_matches_from(args)?;

    match matches.subcommand() {
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
        Some(("init", sub_matches)) => {
            let project_dir = sub_matches
                .get_one::<String>("project-dir")
                .map(PathBuf::from)
                .unwrap_or_default();
            let engine_url = sub_matches
                .get_one::<String>("engine-url")
                .cloned()
                .unwrap_or_default();
            let storage_path = sub_matches
                .get_one::<String>("storage-path")
                .map(PathBuf::from)
                .unwrap_or_default();
            Ok(Cli {
                command: Command::Init {
                    project_dir,
                    engine_url,
                    storage_path,
                },
            })
        }
        Some(("lock", sub_matches)) => {
            let project_dir = sub_matches
                .get_one::<String>("project-dir")
                .map(PathBuf::from)
                .unwrap_or_default();
            Ok(Cli {
                command: Command::Lock { project_dir },
            })
        }
        Some(("doctor", sub_matches)) => {
            let project_dir = sub_matches
                .get_one::<String>("project-dir")
                .map(PathBuf::from)
                .unwrap_or_default();
            Ok(Cli {
                command: Command::Doctor { project_dir },
            })
        }
        Some(("unquarantine", sub_matches)) => {
            let workflow_name = match sub_matches.get_one::<String>("workflow-name") {
                Some(w) => w.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let operator = match sub_matches.get_one::<String>("operator") {
                Some(o) => o.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let engine_url = match sub_matches.get_one::<String>("engine-url") {
                Some(u) => u.clone(),
                None => "http://localhost:3000".to_string(),
            };
            Ok(Cli {
                command: Command::Unquarantine {
                    workflow_name,
                    operator,
                    engine_url,
                },
            })
        }
        Some(("rebuild", sub_matches)) => {
            let projection_id = match sub_matches.get_one::<String>("projection-id") {
                Some(p) => p.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let storage_path = sub_matches
                .get_one::<String>("storage-path")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".vo/storage"));
            let from_sequence = sub_matches
                .get_one::<String>("from-sequence")
                .and_then(|s| s.parse::<u64>().ok());
            let to_sequence = sub_matches
                .get_one::<String>("to-sequence")
                .and_then(|s| s.parse::<u64>().ok());
            let cancel_file = sub_matches
                .get_one::<String>("cancel-file")
                .map(PathBuf::from);
            let dry_run = sub_matches.get_flag("dry-run");
            Ok(Cli {
                command: Command::Rebuild {
                    projection_id,
                    storage_path,
                    from_sequence,
                    to_sequence,
                    cancel_file,
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
        CliError::Dispatch(_)
        | CliError::Check(_)
        | CliError::Gc(_)
        | CliError::Init(_)
        | CliError::Lock(_)
        | CliError::Doctor(_)
        | CliError::Rebuild(_)
        | CliError::Unquarantine(_) => 1,
        CliError::InvalidNumeric(_) => 2,
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
