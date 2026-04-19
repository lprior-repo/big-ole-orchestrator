use std::ffi::OsString;
use std::path::PathBuf;
use vo_cli::cli::{interpret_cli_from, map_error_to_exit_code, Cli, CliError, Command};
use vo_cli::parse::parse_strict_numeric;

#[test]
fn no_args_returns_error() {
    let result = interpret_cli_from::<_, OsString>([]);
    assert!(result.is_err());
}

#[test]
fn unknown_subcommand_returns_error() {
    let args: Vec<OsString> = vec!["vo".into(), "foobar".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn empty_string_subcommand_returns_error() {
    let args: Vec<OsString> = vec!["vo".into(), "".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn purge_without_instance_returns_error() {
    let args: Vec<OsString> = vec!["vo".into(), "purge".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn check_without_path_returns_error() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn gc_parses_defaults() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:3000");
            assert!(!dry_run);
        }
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn gc_parses_custom_engine_url() {
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
fn gc_parses_dry_run_flag() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc { dry_run, .. } => assert!(dry_run),
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn gc_parses_engine_url_and_dry_run_combined() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://custom:9999".into(),
        "--dry-run".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://custom:9999");
            assert!(dry_run);
        }
        _ => panic!("expected Gc command"),
    }
}

#[test]
fn init_parses_defaults() {
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
fn init_parses_all_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "init".into(),
        "--project-dir".into(),
        "/my/project".into(),
        "--engine-url".into(),
        "http://custom:5000".into(),
        "--storage-path".into(),
        "/data/storage".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
            assert_eq!(engine_url, "http://custom:5000");
            assert_eq!(storage_path, PathBuf::from("/data/storage"));
        }
        _ => panic!("expected Init command"),
    }
}

#[test]
fn lock_parses_default_project_dir() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock command"),
    }
}

#[test]
fn lock_parses_custom_project_dir() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "lock".into(),
        "--project-dir".into(),
        "/custom/dir".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/custom/dir"));
        }
        _ => panic!("expected Lock command"),
    }
}

#[test]
fn doctor_parses_default_project_dir() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor command"),
    }
}

#[test]
fn doctor_parses_custom_project_dir() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "doctor".into(),
        "--project-dir".into(),
        "/health/check".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/health/check"));
        }
        _ => panic!("expected Doctor command"),
    }
}

#[test]
fn rebuild_parses_defaults() {
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
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn rebuild_parses_all_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--project-dir".into(),
        "/proj".into(),
        "--projection-id".into(),
        "proj-42".into(),
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
            assert_eq!(projection_id.as_deref(), Some("proj-42"));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn rebuild_parses_projection_id_without_list() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "rebuild".into(),
        "--projection-id".into(),
        "my-projection".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Rebuild {
            projection_id,
            list_projections,
            force,
            ..
        } => {
            assert_eq!(projection_id.as_deref(), Some("my-projection"));
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn rebuild_parses_force_without_projection_id() {
    let args: Vec<OsString> = vec!["vo".into(), "rebuild".into(), "--force".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Rebuild {
            projection_id,
            force,
            ..
        } => {
            assert!(projection_id.is_none());
            assert!(force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

#[test]
fn check_parses_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => {
            assert_eq!(path, PathBuf::from("/bin/ls"));
        }
        _ => panic!("expected Check command"),
    }
}

#[test]
fn purge_parses_instance() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "inst-abc-123".into(),
    ];
    let cli = interpret_cli_from(args).unwrap();
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "inst-abc-123");
        }
        _ => panic!("expected Purge command"),
    }
}

#[test]
fn cli_command_equality() {
    let c1 = Command::Check {
        workflow: false,
        path: PathBuf::from("/a"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: PathBuf::from("/a"),
    };
    assert_eq!(c1, c2);
}

#[test]
fn cli_command_inequality() {
    let c1 = Command::Check {
        workflow: false,
        path: PathBuf::from("/a"),
    };
    let c2 = Command::Check {
        workflow: false,
        path: PathBuf::from("/b"),
    };
    assert_ne!(c1, c2);
}

#[test]
fn cli_struct_equality() {
    let cli1 = Cli {
        command: Command::Gc {
            engine_url: "http://a".into(),
            dry_run: false,
        },
    };
    let cli2 = Cli {
        command: Command::Gc {
            engine_url: "http://a".into(),
            dry_run: false,
        },
    };
    assert_eq!(cli1, cli2);
}

#[test]
fn cli_struct_clone() {
    let cli = Cli {
        command: Command::Lock {
            project_dir: PathBuf::from("/tmp"),
        },
    };
    let cloned = cli.clone();
    assert_eq!(cli, cloned);
}

#[test]
fn cli_error_display_invalid_numeric() {
    let err = CliError::InvalidNumeric("bad input".into());
    let msg = err.to_string();
    assert!(msg.contains("invalid numeric"));
    assert!(msg.contains("bad input"));
}

#[test]
fn cli_error_display_dispatch() {
    let err = CliError::Dispatch("something went wrong".into());
    let msg = err.to_string();
    assert!(msg.contains("dispatch error"));
    assert!(msg.contains("something went wrong"));
}

#[test]
fn map_error_to_exit_code_numeric_returns_2() {
    let err = CliError::InvalidNumeric("x".into());
    assert_eq!(map_error_to_exit_code(&err), 2);
}

#[test]
fn map_error_to_exit_code_dispatch_returns_1() {
    let err = CliError::Dispatch("x".into());
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn parse_strict_numeric_valid_values() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
    assert_eq!(parse_strict_numeric("1").unwrap(), 1);
    assert_eq!(parse_strict_numeric("42").unwrap(), 42);
    assert_eq!(parse_strict_numeric("999999").unwrap(), 999999);
}

#[test]
fn parse_strict_numeric_rejects_empty() {
    assert!(parse_strict_numeric("").is_err());
}

#[test]
fn parse_strict_numeric_rejects_plus() {
    assert!(parse_strict_numeric("+1").is_err());
}

#[test]
fn parse_strict_numeric_rejects_minus() {
    assert!(parse_strict_numeric("-1").is_err());
}

#[test]
fn parse_strict_numeric_rejects_hex_prefix() {
    assert!(parse_strict_numeric("0x10").is_err());
}

#[test]
fn parse_strict_numeric_rejects_alpha() {
    assert!(parse_strict_numeric("abc").is_err());
}

#[test]
fn parse_strict_numeric_rejects_float() {
    assert!(parse_strict_numeric("1.5").is_err());
}

#[test]
fn parse_strict_numeric_rejects_whitespace() {
    assert!(parse_strict_numeric(" 42").is_err());
    assert!(parse_strict_numeric("42 ").is_err());
    assert!(parse_strict_numeric("4 2").is_err());
}

#[test]
fn parse_strict_numeric_max_u64() {
    assert_eq!(
        parse_strict_numeric("18446744073709551615").unwrap(),
        u64::MAX
    );
}

#[test]
fn parse_strict_numeric_overflow() {
    assert!(parse_strict_numeric("18446744073709551616").is_err());
}

#[test]
fn parse_strict_numeric_mixed_alnum() {
    assert!(parse_strict_numeric("12ab34").is_err());
}

#[test]
fn parse_strict_numeric_leading_zeros_ok() {
    assert_eq!(parse_strict_numeric("007").unwrap(), 7);
}
