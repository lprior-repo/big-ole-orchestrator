use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Clap(#[from] clap::Error),
    #[error(transparent)]
    Purge(#[from] crate::commands::purge::PurgeError),
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
    Serve(#[from] crate::commands::serve::ServeError),
    #[error(transparent)]
    Status(#[from] crate::commands::status::StatusError),
    #[error(transparent)]
    Workspace(#[from] crate::commands::workspace::WorkspaceError),
    #[error(transparent)]
    Serve(#[from] crate::commands::serve::ServeError),
    #[error("execute-node error: {0}")]
    ExecuteNode(String),
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
        workflow_id: String,
    },
    Hardline {
        target: String,
        engine_url: String,
        timeout: u64,
        force: bool,
        dry_run: bool,
    },
    Serve {
        host: String,
        port: u16,
        storage_path: PathBuf,
    },
    History {
        instance_id: String,
        engine_url: String,
        json: bool,
        canonical: bool,
    },
    Workspace {
        action: WorkspaceAction,
    },
    ExecuteNode {
        binary: PathBuf,
        node_name: String,
        instance_id: String,
        node_id: String,
        input: Option<String>,
        secrets: Vec<String>,
        timeout_ms: u64,
        node_kind: String,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum WorkspaceAction {
    List,
    Create { name: String },
    Delete { id: String },
    Show { id: String },
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
                        .help("The workflow instance ID to compensate")
                        .value_parser(|s: &str| {
                            if s.is_empty() {
                                return Err(clap::Error::new(clap::error::ErrorKind::InvalidValue));
                            }
                            Ok(s.to_string())
                        }),
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
                        .default_value(".vo/versions")
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
        )
        .subcommand(
            clap::Command::new("workspace")
                .about("Manage workspaces")
                .subcommand(clap::Command::new("list").about("List all workspaces"))
                .subcommand(
                    clap::Command::new("create")
                        .about("Create a new workspace")
                        .arg(
                            clap::Arg::new("name")
                                .required(true)
                                .index(1)
                                .help("Workspace name"),
                        ),
                )
                .subcommand(
                    clap::Command::new("delete")
                        .about("Delete a workspace")
                        .arg(
                            clap::Arg::new("id")
                                .required(true)
                                .index(1)
                                .help("Workspace ID"),
                        ),
                )
                .subcommand(
                    clap::Command::new("show")
                        .about("Show workspace details")
                        .arg(
                            clap::Arg::new("id")
                                .required(true)
                                .index(1)
                                .help("Workspace ID"),
                        ),
                ),
        )
        .subcommand(
            clap::Command::new("execute-node")
                .about("Execute a single workflow node in a subprocess (ADR-003)")
                .arg(
                    clap::Arg::new("binary")
                        .required(true)
                        .index(1)
                        .help("Path to the workflow binary"),
                )
                .arg(
                    clap::Arg::new("node-name")
                        .required(true)
                        .index(2)
                        .help("Name of the node to execute"),
                )
                .arg(
                    clap::Arg::new("instance-id")
                        .long("instance-id")
                        .required(true)
                        .value_name("ID")
                        .help("Workflow instance ID"),
                )
                .arg(
                    clap::Arg::new("node-id")
                        .long("node-id")
                        .required(true)
                        .value_name("ID")
                        .help("Node execution ID"),
                )
                .arg(
                    clap::Arg::new("input")
                        .long("input")
                        .value_name("JSON")
                        .help("JSON input payload for the node"),
                )
                .arg(
                    clap::Arg::new("secret")
                        .long("secret")
                        .action(clap::ArgAction::Append)
                        .value_name("KEY=VALUE")
                        .help("Secret key-value pairs (repeatable)"),
                )
                .arg(
                    clap::Arg::new("timeout")
                        .long("timeout")
                        .default_value("30000")
                        .value_name("MS")
                        .help("Timeout in milliseconds"),
                )
                .arg(
                    clap::Arg::new("node-kind")
                        .long("node-kind")
                        .default_value("pure")
                        .value_name("KIND")
                        .help("Node kind: pure, managed_effect, unsafe, wait, signal"),
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
                .unwrap_or_else(|| PathBuf::from(".vo/versions"));
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
            let workflow_id = sub_matches
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
                    workflow_id,
                },
            })
        }
        Some(("hardline", sub_matches)) => {
            let target = match sub_matches.get_one::<String>("target") {
                Some(t) => t.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let engine_url = sub_matches
                .get_one::<String>("engine-url")
                .cloned()
                .unwrap_or_else(|| "http://localhost:3000".to_string());
            let timeout_str = sub_matches
                .get_one::<String>("timeout")
                .map(|s| s.as_str())
                .unwrap_or("60");
            let timeout: u64 = timeout_str.parse().unwrap_or(60);
            let force = sub_matches.get_flag("force");
            let dry_run = sub_matches.get_flag("dry-run");
            Ok(Cli {
                command: Command::Hardline {
                    target,
                    engine_url,
                    timeout,
                    force,
                    dry_run,
                },
            })
        }
        Some(("serve", sub_matches)) => {
            let host = sub_matches
                .get_one::<String>("host")
                .cloned()
                .unwrap_or_else(|| "127.0.0.1".to_string());
            let port_str = sub_matches
                .get_one::<String>("port")
                .map(|s| s.as_str())
                .unwrap_or("3000");
            let port: u16 = port_str.parse().unwrap_or(3000);
            let storage_path = sub_matches
                .get_one::<String>("storage-path")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".vo/storage"));
            Ok(Cli {
                command: Command::Serve {
                    host,
                    port,
                    storage_path,
                },
            })
        }
        Some(("history", sub_matches)) => {
            let instance_id = sub_matches
                .get_one::<String>("instance")
                .cloned()
                .unwrap_or_default();
            let engine_url = sub_matches
                .get_one::<String>("engine-url")
                .cloned()
                .unwrap_or_else(|| "http://localhost:3000".to_string());
            let json = sub_matches.get_flag("json");
            let canonical = sub_matches.get_flag("canonical");
            Ok(Cli {
                command: Command::History {
                    instance_id,
                    engine_url,
                    json,
                    canonical,
                },
            })
        }
        Some(("workspace", sub_matches)) => match sub_matches.subcommand() {
            Some(("list", _)) => Ok(Cli {
                command: Command::Workspace {
                    action: WorkspaceAction::List,
                },
            }),
            Some(("create", create_matches)) => {
                let name = create_matches
                    .get_one::<String>("name")
                    .cloned()
                    .unwrap_or_default();
                Ok(Cli {
                    command: Command::Workspace {
                        action: WorkspaceAction::Create { name },
                    },
                })
            }
            Some(("delete", delete_matches)) => {
                let id = delete_matches
                    .get_one::<String>("id")
                    .cloned()
                    .unwrap_or_default();
                Ok(Cli {
                    command: Command::Workspace {
                        action: WorkspaceAction::Delete { id },
                    },
                })
            }
            Some(("show", show_matches)) => {
                let id = show_matches
                    .get_one::<String>("id")
                    .cloned()
                    .unwrap_or_default();
                Ok(Cli {
                    command: Command::Workspace {
                        action: WorkspaceAction::Show { id },
                    },
                })
            }
            _ => Err(clap::Error::new(clap::error::ErrorKind::InvalidSubcommand)),
        },
        Some(("execute-node", sub_matches)) => {
            let binary = match sub_matches.get_one::<String>("binary") {
                Some(b) => PathBuf::from(b),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let node_name = match sub_matches.get_one::<String>("node-name") {
                Some(n) => n.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let instance_id = match sub_matches.get_one::<String>("instance-id") {
                Some(id) => id.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let node_id = match sub_matches.get_one::<String>("node-id") {
                Some(id) => id.clone(),
                None => {
                    return Err(clap::Error::new(
                        clap::error::ErrorKind::MissingRequiredArgument,
                    ))
                }
            };
            let input = sub_matches.get_one::<String>("input").cloned();
            let secrets = sub_matches
                .get_many::<String>("secret")
                .map(|v| v.cloned().collect())
                .unwrap_or_default();
            let timeout_ms = sub_matches
                .get_one::<String>("timeout")
                .map(|s| s.parse().unwrap_or(30000))
                .unwrap_or(30000);
            let node_kind = sub_matches
                .get_one::<String>("node-kind")
                .cloned()
                .unwrap_or_else(|| "pure".to_string());
            Ok(Cli {
                command: Command::ExecuteNode {
                    binary,
                    node_name,
                    instance_id,
                    node_id,
                    input,
                    secrets,
                    timeout_ms,
                    node_kind,
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
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
            _ => 2,
        },
        CliError::Dispatch(_)
        | CliError::Purge(_)
        | CliError::Check(_)
        | CliError::Compensate(_)
        | CliError::Gc(_)
        | CliError::Init(_)
        | CliError::Lock(_)
        | CliError::Doctor(_)
        | CliError::Rebuild(_)
        | CliError::Serve(_)
        | CliError::Status(_)
        | CliError::Workspace(_)
        | CliError::Serve(_)
        | CliError::ExecuteNode(_) => 1,
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
                workflow_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
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
                workflow_id: "instance-123".to_string(),
            }
        );
    }

    #[test]
    fn cli_status_without_instance_returns_error() {
        let args: Vec<OsString> = vec!["vo".into(), "status".into()];
        let result = interpret_cli_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn cli_execute_node_minimal() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "/usr/bin/true".into(),
            "node-a".into(),
            "--instance-id".into(),
            "inst-1".into(),
            "--node-id".into(),
            "exec-1".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        assert_eq!(
            cli.command,
            Command::ExecuteNode {
                binary: PathBuf::from("/usr/bin/true"),
                node_name: "node-a".to_string(),
                instance_id: "inst-1".to_string(),
                node_id: "exec-1".to_string(),
                input: None,
                secrets: vec![],
                timeout_ms: 30000,
                node_kind: "pure".to_string(),
            }
        );
    }

    #[test]
    fn cli_execute_node_with_all_options() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "./target/wf-binary".into(),
            "process-payment".into(),
            "--instance-id".into(),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
            "--node-id".into(),
            "node-42".into(),
            "--input".into(),
            r#"{"amount": 100}"#.into(),
            "--secret".into(),
            "API_KEY=sk_live_abc".into(),
            "--secret".into(),
            "DB_PASS=hunter2".into(),
            "--timeout".into(),
            "5000".into(),
            "--node-kind".into(),
            "managed_effect".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        match cli.command {
            Command::ExecuteNode {
                binary,
                node_name,
                instance_id,
                node_id,
                input,
                secrets,
                timeout_ms,
                node_kind,
            } => {
                assert_eq!(binary, PathBuf::from("./target/wf-binary"));
                assert_eq!(node_name, "process-payment");
                assert_eq!(instance_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
                assert_eq!(node_id, "node-42");
                assert_eq!(input, Some(r#"{"amount": 100}"#.to_string()));
                assert_eq!(secrets, vec!["API_KEY=sk_live_abc", "DB_PASS=hunter2"]);
                assert_eq!(timeout_ms, 5000);
                assert_eq!(node_kind, "managed_effect");
            }
            other => panic!("expected ExecuteNode, got {:?}", other),
        }
    }

    #[test]
    fn cli_execute_node_missing_binary_returns_error() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "--instance-id".into(),
            "inst-1".into(),
            "--node-id".into(),
            "exec-1".into(),
        ];
        let result = interpret_cli_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn cli_execute_node_missing_node_name_returns_error() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "/bin/true".into(),
            "--instance-id".into(),
            "inst-1".into(),
            "--node-id".into(),
            "exec-1".into(),
        ];
        let result = interpret_cli_from(args);
        assert!(result.is_err());
    }

    #[test]
    fn cli_execute_node_unsafe_kind() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "/bin/true".into(),
            "legacy-call".into(),
            "--instance-id".into(),
            "inst-2".into(),
            "--node-id".into(),
            "exec-2".into(),
            "--node-kind".into(),
            "unsafe".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        match cli.command {
            Command::ExecuteNode { node_kind, .. } => {
                assert_eq!(node_kind, "unsafe");
            }
            other => panic!("expected ExecuteNode, got {:?}", other),
        }
    }

    #[test]
    fn cli_execute_node_wait_kind() {
        let args: Vec<OsString> = vec![
            "vo".into(),
            "execute-node".into(),
            "/bin/true".into(),
            "timer-node".into(),
            "--instance-id".into(),
            "inst-3".into(),
            "--node-id".into(),
            "exec-3".into(),
            "--node-kind".into(),
            "wait".into(),
        ];
        let cli = interpret_cli_from(args).unwrap();
        match cli.command {
            Command::ExecuteNode { node_kind, .. } => {
                assert_eq!(node_kind, "wait");
            }
            other => panic!("expected ExecuteNode, got {:?}", other),
        }
    }
}
