use std::collections::BTreeMap;
use std::path::PathBuf;

use vo_types::workspace::{WorkspaceId, WorkspaceName, WorkspacePath};

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
    Compensate(#[from] crate::commands::compensate::CompensateError),
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
    Status(#[from] crate::commands::status::StatusError),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Purge {
        instance: String,
        storage_path: PathBuf,
    },
    Check {
        workflow: bool,
        path: PathBuf,
    },
    Compensate {
        engine_url: String,
        workflow_id: String,
        force: bool,
    },
    Gc {
        engine_url: String,
        versions_dir: PathBuf,
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
    Rebuild {
        project_dir: PathBuf,
        projection_id: Option<String>,
        list_projections: bool,
        force: bool,
    },
    Status {
        engine_url: String,
        instance: String,
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
            clap::Command::new("purge")
                .arg(
                    clap::Arg::new("instance")
                        .long("instance")
                        .required(true)
                        .value_name("ID")
                        .help("The instance ID to purge"),
                )
                .arg(
                    clap::Arg::new("storage-path")
                        .long("storage-path")
                        .default_value(".vo/storage")
                        .help("Storage path for fjall database"),
                ),
        )
        .subcommand(
            clap::Command::new("check")
                .arg(clap::Arg::new("path").required(true).index(1))
                .arg(
                    clap::Arg::new("workflow")
                        .long("workflow")
                        .action(clap::ArgAction::SetTrue)
                        .help("Validate workflow spec JSON instead of binary header"),
                ),
        )
        .subcommand(
            clap::Command::new("compensate")
                .about("Compensate a workflow instance")
                .arg(
                    clap::Arg::new("workflow-id")
                        .required(true)
                        .index(1)
                        .help("The workflow instance ID to compensate"),
                )
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .env("VO_ENGINE_URL")
                        .default_value("http://localhost:3000"),
                )
                .arg(
                    clap::Arg::new("force")
                        .long("force")
                        .action(clap::ArgAction::SetTrue)
                        .help("Skip confirmation prompt"),
                ),
        )
        .subcommand(
            clap::Command::new("gc")
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .env("VO_ENGINE_URL")
                        .default_value("http://localhost:3000"),
                )
                .arg(
                    clap::Arg::new("versions-dir")
                        .long("versions-dir")
                        .default_value("/var/wtf/versions")
                        .help("Versions directory to garbage collect"),
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
            clap::Command::new("rebuild")
                .about("Rebuild projection from canonical event log")
                .arg(
                    clap::Arg::new("project-dir")
                        .long("project-dir")
                        .default_value(".")
                        .help("Project directory"),
                )
                .arg(
                    clap::Arg::new("projection-id")
                        .long("projection-id")
                        .help("Projection ID to rebuild"),
                )
                .arg(
                    clap::Arg::new("list")
                        .long("list")
                        .action(clap::ArgAction::SetTrue)
                        .help("List all registered projections"),
                )
                .arg(
                    clap::Arg::new("force")
                        .long("force")
                        .action(clap::ArgAction::SetTrue)
                        .help("Force rebuild even if projection is not stale"),
                ),
        )
        .subcommand(
            clap::Command::new("status")
                .about("Query workflow lineage status")
                .arg(
                    clap::Arg::new("instance")
                        .required(true)
                        .index(1)
                        .help("Workflow instance ID (e.g., namespace/01ARZ3NDEKTSV4RRFFQ69G5FAV)"),
                )
                .arg(
                    clap::Arg::new("engine-url")
                        .long("engine-url")
                        .env("VO_ENGINE_URL")
                        .default_value("http://localhost:3000")
                        .help("Engine URL"),
                ),
        );

    let matches = cmd.try_get_matches_from(args)?;

    match matches.subcommand() {
        Some(("purge", purge_matches)) => {
            let instance = purge_matches
                .get_one::<String>("instance")
                .cloned()
                .unwrap_or_default();
            let storage_path = purge_matches
                .get_one::<String>("storage-path")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".vo/storage"));
            Ok(Cli {
                command: Command::Purge {
                    instance,
                    storage_path,
                },
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
            let workflow = sub_matches.get_flag("workflow");
            Ok(Cli {
                command: Command::Check { workflow, path },
            })
        }
        Some(("compensate", sub_matches)) => {
            let workflow_id = match sub_matches.get_one::<String>("workflow-id") {
                Some(id) => id.clone(),
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
            let force = sub_matches.get_flag("force");
            Ok(Cli {
                command: Command::Compensate {
                    engine_url,
                    workflow_id,
                    force,
                },
            })
        }
        Some(("gc", sub_matches)) => {
            let engine_url = match sub_matches.get_one::<String>("engine-url") {
                Some(u) => u.clone(),
                None => "http://localhost:3000".to_string(),
            };
            let versions_dir = sub_matches
                .get_one::<String>("versions-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/var/wtf/versions"));
            let dry_run = sub_matches.get_flag("dry-run");
            Ok(Cli {
                command: Command::Gc {
                    engine_url,
                    versions_dir,
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
        Some(("rebuild", sub_matches)) => {
            let project_dir = sub_matches
                .get_one::<String>("project-dir")
                .map(PathBuf::from)
                .unwrap_or_default();
            let projection_id = sub_matches.get_one::<String>("projection-id").cloned();
            let list_projections = sub_matches.get_flag("list");
            let force = sub_matches.get_flag("force");
            Ok(Cli {
                command: Command::Rebuild {
                    project_dir,
                    projection_id,
                    list_projections,
                    force,
                },
            })
        }
        Some(("status", sub_matches)) => {
            let instance = sub_matches
                .get_one::<String>("instance")
                .cloned()
                .unwrap_or_default();
            let engine_url = sub_matches
                .get_one::<String>("engine-url")
                .cloned()
                .unwrap_or_else(|| "http://localhost:3000".to_string());
            Ok(Cli {
                command: Command::Status {
                    engine_url,
                    instance,
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
        | CliError::Compensate(_)
        | CliError::Gc(_)
        | CliError::Init(_)
        | CliError::Lock(_)
        | CliError::Doctor(_)
        | CliError::Rebuild(_)
        | CliError::Status(_) => 1,
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
                instance: "123".to_string(),
                storage_path: PathBuf::from(".vo/storage"),
            }
        );
    }

    #[test]
    fn cli_compensate_matches_with_workflow_id() {
        let args: Vec<OsString> = vec!["vo".into(), "compensate".into(), "wf-abc123".into()];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Compensate {
                engine_url: "http://localhost:3000".to_string(),
                workflow_id: "wf-abc123".to_string(),
                force: false,
            }
        );
    }

    #[test]
    fn cli_compensate_with_force_flag() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "compensate".into(),
            "wf-xyz789".into(),
            "--force".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Compensate {
                engine_url: "http://localhost:3000".to_string(),
                workflow_id: "wf-xyz789".to_string(),
                force: true,
            }
        );
    }

    #[test]
    fn cli_compensate_with_custom_engine_url() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "compensate".into(),
            "wf-custom".into(),
            "--engine-url".into(),
            "http://localhost:9000".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Compensate {
                engine_url: "http://localhost:9000".to_string(),
                workflow_id: "wf-custom".to_string(),
                force: false,
            }
        );
    }

    #[test]
    fn cli_status_matches_with_instance_id() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "status".into(),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Status {
                engine_url: "http://localhost:3000".to_string(),
                instance: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
            }
        );
    }

    #[test]
    fn cli_status_with_custom_engine_url() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "status".into(),
            "instance-123".into(),
            "--engine-url".into(),
            "http://localhost:9000".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::Status {
                engine_url: "http://localhost:9000".to_string(),
                instance: "instance-123".to_string(),
            }
        );
    }

    #[test]
    fn cli_status_without_instance_returns_error() {
        let args: Vec<OsString> = vec!["vo".into(), "status".into()];
        let result = interpret_cli_from(args);
        assert!(result.is_err());
    }
}
