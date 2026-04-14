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
    Workspace(#[from] crate::commands::workspace::WorkspaceError),
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
    Rebuild {
        project_dir: PathBuf,
        projection_id: Option<String>,
        list_projections: bool,
        force: bool,
    },
    Workspace {
        project_dir: PathBuf,
        subcommand: WorkspaceSubcommand,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum WorkspaceSubcommand {
    Create {
        name: WorkspaceName,
        parent_id: Option<WorkspaceId>,
        metadata: BTreeMap<String, String>,
    },
    List {
        workspace_id: Option<WorkspaceId>,
    },
    Delete {
        id: WorkspaceId,
        force: bool,
    },
    Move {
        id: WorkspaceId,
        new_parent_id: Option<WorkspaceId>,
    },
    Show {
        id: WorkspaceId,
    },
    Find {
        path: WorkspacePath,
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
            clap::Command::new("workspace")
                .about("Manage workspaces")
                .arg(
                    clap::Arg::new("project-dir")
                        .long("project-dir")
                        .default_value(".")
                        .help("Project directory"),
                )
                .subcommand(
                    clap::Command::new("create")
                        .about("Create a workspace")
                        .arg(clap::Arg::new("name").required(true).help("Workspace name"))
                        .arg(
                            clap::Arg::new("parent")
                                .long("parent")
                                .help("Parent workspace ID"),
                        )
                        .arg(
                            clap::Arg::new("metadata")
                                .long("metadata")
                                .value_name("key=value")
                                .num_args(0..)
                                .help("Metadata key-value pairs"),
                        ),
                )
                .subcommand(
                    clap::Command::new("list").about("List workspaces").arg(
                        clap::Arg::new("workspace")
                            .long("workspace")
                            .help("List children of this workspace"),
                    ),
                )
                .subcommand(
                    clap::Command::new("delete")
                        .about("Delete a workspace")
                        .arg(
                            clap::Arg::new("id")
                                .required(true)
                                .help("Workspace ID to delete"),
                        )
                        .arg(
                            clap::Arg::new("force")
                                .long("force")
                                .action(clap::ArgAction::SetTrue)
                                .help("Force delete even with children"),
                        ),
                )
                .subcommand(
                    clap::Command::new("move")
                        .about("Move a workspace")
                        .arg(
                            clap::Arg::new("id")
                                .required(true)
                                .help("Workspace ID to move"),
                        )
                        .arg(
                            clap::Arg::new("parent")
                                .long("parent")
                                .required(true)
                                .help("New parent workspace ID (or empty for root)"),
                        ),
                )
                .subcommand(
                    clap::Command::new("show")
                        .about("Show workspace details")
                        .arg(clap::Arg::new("id").required(true).help("Workspace ID")),
                )
                .subcommand(
                    clap::Command::new("find")
                        .about("Find workspace by path")
                        .arg(
                            clap::Arg::new("path")
                                .required(true)
                                .help("Workspace path (e.g., parent/child)"),
                        ),
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
        Some(("workspace", ws_matches)) => {
            let project_dir = ws_matches
                .get_one::<String>("project-dir")
                .map(PathBuf::from)
                .unwrap_or_default();

            let (subcommand_name, sub_matches) = ws_matches
                .subcommand()
                .ok_or_else(|| clap::Error::new(clap::error::ErrorKind::InvalidSubcommand))?;

            match subcommand_name {
                "create" => {
                    let name_str = sub_matches.get_one::<String>("name").ok_or_else(|| {
                        clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)
                    })?;
                    let name = WorkspaceName::parse(name_str)
                        .map_err(|e| clap::Error::new(clap::error::ErrorKind::InvalidValue))?;
                    let parent_id = sub_matches.get_one::<String>("parent").map(|s| {
                        WorkspaceId::from_ulid(s.parse().unwrap_or_else(|_| {
                            ulid::Ulid::from_string(s).unwrap_or_else(|_| ulid::Ulid::new())
                        }))
                    });
                    let metadata_pairs: Vec<&String> = sub_matches
                        .get_many::<String>("metadata")
                        .map(|v| v.collect())
                        .unwrap_or_default();
                    let mut metadata = BTreeMap::new();
                    for pair in metadata_pairs {
                        if let Some((k, v)) = pair.split_once('=') {
                            metadata.insert(k.to_string(), v.to_string());
                        }
                    }
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::Create {
                                name,
                                parent_id,
                                metadata,
                            },
                        },
                    })
                }
                "list" => {
                    let workspace_id = sub_matches.get_one::<String>("workspace").map(|s| {
                        WorkspaceId::from_ulid(s.parse().unwrap_or_else(|_| {
                            ulid::Ulid::from_string(s).unwrap_or_else(|_| ulid::Ulid::new())
                        }))
                    });
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::List { workspace_id },
                        },
                    })
                }
                "delete" => {
                    let id_str = sub_matches.get_one::<String>("id").ok_or_else(|| {
                        clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)
                    })?;
                    let id = WorkspaceId::from_ulid(id_str.parse().unwrap_or_else(|_| {
                        ulid::Ulid::from_string(id_str).unwrap_or_else(|_| ulid::Ulid::new())
                    }));
                    let force = sub_matches.get_flag("force");
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::Delete { id, force },
                        },
                    })
                }
                "move" => {
                    let id_str = sub_matches.get_one::<String>("id").ok_or_else(|| {
                        clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)
                    })?;
                    let id = WorkspaceId::from_ulid(id_str.parse().unwrap_or_else(|_| {
                        ulid::Ulid::from_string(id_str).unwrap_or_else(|_| ulid::Ulid::new())
                    }));
                    let new_parent_id = sub_matches.get_one::<String>("parent").map(|s| {
                        WorkspaceId::from_ulid(s.parse().unwrap_or_else(|_| {
                            ulid::Ulid::from_string(s).unwrap_or_else(|_| ulid::Ulid::new())
                        }))
                    });
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::Move { id, new_parent_id },
                        },
                    })
                }
                "show" => {
                    let id_str = sub_matches.get_one::<String>("id").ok_or_else(|| {
                        clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)
                    })?;
                    let id = WorkspaceId::from_ulid(id_str.parse().unwrap_or_else(|_| {
                        ulid::Ulid::from_string(id_str).unwrap_or_else(|_| ulid::Ulid::new())
                    }));
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::Show { id },
                        },
                    })
                }
                "find" => {
                    let path_str = sub_matches.get_one::<String>("path").ok_or_else(|| {
                        clap::Error::new(clap::error::ErrorKind::MissingRequiredArgument)
                    })?;
                    let segments: Vec<WorkspaceName> = path_str
                        .split('/')
                        .map(WorkspaceName::parse)
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| clap::Error::new(clap::error::ErrorKind::InvalidValue))?;
                    let segments = vo_types::NonEmptyVec::new(segments)
                        .map_err(|_| clap::Error::new(clap::error::ErrorKind::InvalidValue))?;
                    let path = WorkspacePath::new(segments)
                        .map_err(|_| clap::Error::new(clap::error::ErrorKind::InvalidValue))?;
                    Ok(Cli {
                        command: Command::Workspace {
                            project_dir,
                            subcommand: WorkspaceSubcommand::Find { path },
                        },
                    })
                }
                _ => Err(clap::Error::new(clap::error::ErrorKind::InvalidSubcommand)),
            }
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
        | CliError::Workspace(_) => 1,
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
