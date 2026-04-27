#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{
    interpret_cli_from, parse_strict_numeric, Command,
};

#[test]
fn parse_version_flag_returns_display_version_error() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

#[test]
fn parse_no_args_returns_help_error() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn parse_invalid_subcommand_returns_error() {
    let result = interpret_cli_from(vec!["vo", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn parse_purge_without_instance_returns_error() {
    let result = interpret_cli_from(vec!["vo", "purge"]);
    assert!(result.is_err());
}

#[test]
fn parse_purge_with_empty_instance_succeeds() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).expect("parse");
    match &cli.command {
        Command::Purge { instance } => assert_eq!(instance, ""),
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_purge_with_special_chars_instance() {
    let cli =
        interpret_cli_from(vec!["vo", "purge", "--instance", "inst-123_abc.v2"]).expect("parse");
    match &cli.command {
        Command::Purge { instance } => assert_eq!(instance, "inst-123_abc.v2"),
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_init_with_all_custom_args() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/custom/project",
        "--engine-url",
        "http://engine:4000",
        "--storage-path",
        "/custom/storage",
    ])
    .expect("parse");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, &PathBuf::from("/custom/project"));
            assert_eq!(engine_url, "http://engine:4000");
            assert_eq!(storage_path, &PathBuf::from("/custom/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_with_partial_custom_args() {
    let cli = interpret_cli_from(vec!["vo", "init", "--engine-url", "http://custom:5000"])
        .expect("parse");
    match &cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, &PathBuf::from("."));
            assert_eq!(engine_url, "http://custom:5000");
            assert_eq!(storage_path, &PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_lock_with_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "lock", "--project-dir", "/my/project"]).expect("parse");
    match &cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, &PathBuf::from("/my/project"));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_with_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/diag/path"]).expect("parse");
    match &cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, &PathBuf::from("/diag/path"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse");
    match &cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, &PathBuf::from("."));
            assert!(projection_id.is_none());
            assert!(!list_projections);
            assert!(!force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_id() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "my-proj-42"]).expect("parse");
    match &cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("my-proj-42"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_list_only() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).expect("parse");
    match &cli.command {
        Command::Rebuild {
            list_projections,
            projection_id,
            ..
        } => {
            assert!(*list_projections);
            assert!(projection_id.is_none());
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_force() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--force"]).expect("parse");
    match &cli.command {
        Command::Rebuild { force, .. } => assert!(*force),
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_all_flags_together() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/proj",
        "--projection-id",
        "proj-1",
        "--list",
        "--force",
    ])
    .expect("parse");
    match &cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, &PathBuf::from("/proj"));
            assert_eq!(projection_id.as_deref(), Some("proj-1"));
            assert!(*list_projections);
            assert!(*force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_gc_with_both_flags() {
    let cli = interpret_cli_from(vec![
        "vo",
        "gc",
        "--engine-url",
        "http://custom:9999",
        "--dry-run",
    ])
    .expect("parse");
    match &cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://custom:9999");
            assert!(*dry_run);
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_check_with_relative_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "../bin/workflow"]).expect("parse");
    match &cli.command {
        Command::Check {
            workflow: false,
            path,
        } => {
            assert_eq!(path, &PathBuf::from("../bin/workflow"));
        }
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_strict_negative_rejected() {
    let err = parse_strict_numeric("-5").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("negative"));
}

#[test]
fn parse_strict_empty_rejected() {
    let err = parse_strict_numeric("").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("empty"));
}

#[test]
fn parse_strict_plus_rejected() {
    let err = parse_strict_numeric("+42").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("plus"));
}

#[test]
fn parse_strict_non_digits_rejected() {
    let err = parse_strict_numeric("abc").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("invalid"));
}

#[test]
fn parse_strict_max_u64_accepted() {
    let result = parse_strict_numeric("18446744073709551615");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), u64::MAX);
}

#[test]
fn parse_strict_overflow_rejected() {
    let err = parse_strict_numeric("18446744073709551616").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("overflow"));
}

#[test]
fn help_flag_for_subcommand() {
    let result = interpret_cli_from(vec!["vo", "init", "--help"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn help_flag_for_gc_subcommand() {
    let result = interpret_cli_from(vec!["vo", "gc", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_check_subcommand() {
    let result = interpret_cli_from(vec!["vo", "check", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_lock_subcommand() {
    let result = interpret_cli_from(vec!["vo", "lock", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_doctor_subcommand() {
    let result = interpret_cli_from(vec!["vo", "doctor", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn help_flag_for_rebuild_subcommand() {
    let result = interpret_cli_from(vec!["vo", "rebuild", "--help"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}
