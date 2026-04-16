use std::ffi::OsString;
use std::path::PathBuf;
use vo_cli::cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
use vo_cli::commands::check::CheckError;
use vo_cli::commands::init::InitError;
use vo_cli::commands::rebuild::RebuildError;

#[test]
fn parse_purge_with_instance() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "i-42".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "i-42".to_string()
        }
    );
}

#[test]
fn parse_check_with_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/usr/bin/ls".into()];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Check { workflow: false, path: PathBuf::from("/usr/bin/ls") }
    );
}

#[test]
fn parse_gc_with_engine_url() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://engine:4000".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://engine:4000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn parse_gc_with_dry_run() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc { dry_run, .. } => assert!(dry_run),
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn parse_gc_dry_run_and_engine_url() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--dry-run".into(),
        "--engine-url".into(),
        "http://e:1".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://e:1");
            assert!(dry_run);
        }
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn parse_init_all_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "init".into(),
        "--project-dir".into(),
        "/tmp/proj".into(),
        "--engine-url".into(),
        "http://e:9999".into(),
        "--storage-path".into(),
        "/data/store".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/tmp/proj"));
            assert_eq!(engine_url, "http://e:9999");
            assert_eq!(storage_path, PathBuf::from("/data/store"));
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn parse_init_defaults() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(engine_url, "http://localhost:3000");
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn parse_lock_with_project_dir() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "lock".into(),
        "--project-dir".into(),
        "/my/proj".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Lock {
            project_dir: PathBuf::from("/my/proj")
        }
    );
}

#[test]
fn parse_lock_default_project_dir() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into()];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Lock {
            project_dir: PathBuf::from(".")
        }
    );
}

#[test]
fn parse_doctor_with_project_dir() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "doctor".into(),
        "--project-dir".into(),
        "/diag".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Doctor {
            project_dir: PathBuf::from("/diag")
        }
    );
}

#[test]
fn parse_doctor_default_project_dir() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into()];
    let cli = interpret_cli_from(args).unwrap();
    assert_eq!(
        cli.command,
        Command::Doctor {
            project_dir: PathBuf::from(".")
        }
    );
}

#[test]
fn parse_rebuild_all_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--project-dir".into(),
        "/proj".into(),
        "--projection-id".into(),
        "orders-v2".into(),
        "--list".into(),
        "--force".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/proj"));
            assert_eq!(projection_id, Some("orders-v2".to_string()));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let args: Vec<OsString> = vec!["vo".into(), "rebuild".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("."));
            assert_eq!(projection_id, None);
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn parse_rebuild_with_projection_id_only() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--projection-id".into(),
        "proj-a".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Rebuild {
            projection_id,
            list_projections,
            force,
            ..
        } => {
            assert_eq!(projection_id, Some("proj-a".to_string()));
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn exit_code_clap_error_is_2() {
    let err = CliError::Clap(clap::Error::new(clap::error::ErrorKind::InvalidValue));
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn exit_code_help_is_0() {
    let err = CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayHelp));
    assert_eq!(map_error_to_exit_code(&err), 0);
}

#[test]
fn exit_code_version_is_0() {
    let err = CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayVersion));
    assert_eq!(map_error_to_exit_code(&err), 0);
}

#[test]
fn exit_code_help_on_missing_arg_is_0() {
    let err = CliError::Clap(clap::Error::new(
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
    ));
    assert_eq!(map_error_to_exit_code(&err), 0);
}

#[test]
fn exit_code_dispatch_error_is_1() {
    let err = CliError::Dispatch("fail".to_string());
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_check_error_is_1() {
    let err = CliError::Check(CheckError::FileNotFound {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_init_error_is_1() {
    let err = CliError::Init(InitError::DirNotFound {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_rebuild_error_is_1() {
    let err = CliError::Rebuild(RebuildError::NotInitialized {
        path: PathBuf::from("/x"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn exit_code_invalid_numeric_is_2() {
    let err = CliError::InvalidNumeric("abc".to_string());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn cli_equality_same_command() {
    let a = Cli {
        command: Command::Purge {
            instance: "x".to_string(),
        },
    };
    let b = Cli {
        command: Command::Purge {
            instance: "x".to_string(),
        },
    };
    assert_eq!(a, b);
}

#[test]
fn cli_inequality_different_instance() {
    let a = Cli {
        command: Command::Purge {
            instance: "x".to_string(),
        },
    };
    let b = Cli {
        command: Command::Purge {
            instance: "y".to_string(),
        },
    };
    assert_ne!(a, b);
}

#[test]
fn command_clone_preserves_values() {
    let cmd = Command::Rebuild {
        project_dir: PathBuf::from("/p"),
        projection_id: Some("proj".to_string()),
        list_projections: true,
        force: false,
    };
    let cloned = cmd.clone();
    assert_eq!(cmd, cloned);
}
