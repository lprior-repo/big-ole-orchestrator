#![allow(clippy::redundant_pattern_matching)]
use std::path::{Path, PathBuf};

use vo_cli::{interpret_cli_from, Command};

#[test]
fn parse_init_all_defaults() {
    let cli = interpret_cli_from(vec!["vo", "init"]).unwrap();
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
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_custom_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", "/custom/dir"]).unwrap();
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("/custom/dir"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_all_custom() {
    let cli = interpret_cli_from(vec![
        "vo",
        "init",
        "--project-dir",
        "/my/project",
        "--engine-url",
        "http://engine:8080",
        "--storage-path",
        "/data/vo",
    ])
    .unwrap();
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
            assert_eq!(engine_url, "http://engine:8080");
            assert_eq!(storage_path, PathBuf::from("/data/vo"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).unwrap();
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
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_projection_id() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "my-proj"]).unwrap();
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id, Some("my-proj".to_string()));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_list_only() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).unwrap();
    match cli.command {
        Command::Rebuild {
            list_projections, ..
        } => {
            assert!(list_projections);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_force_with_projection_id() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--projection-id",
        "proj-1",
        "--force",
    ])
    .unwrap();
    match cli.command {
        Command::Rebuild {
            projection_id,
            force,
            ..
        } => {
            assert_eq!(projection_id, Some("proj-1".to_string()));
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_all_flags() {
    let cli = interpret_cli_from(vec![
        "vo",
        "rebuild",
        "--project-dir",
        "/tmp",
        "--projection-id",
        "p1",
        "--list",
        "--force",
    ])
    .unwrap();
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/tmp"));
            assert_eq!(projection_id, Some("p1".to_string()));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_purge_basic() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", "inst-123"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "inst-123".to_string()
        }
    );
}

#[test]
fn parse_purge_empty_instance() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "".to_string()
        }
    );
}

#[test]
fn parse_purge_special_chars_instance() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", "inst-àéïôü-测试"]).unwrap();
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "inst-àéïôü-测试");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_lock_defaults() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_lock_custom_dir() {
    let cli = interpret_cli_from(vec!["vo", "lock", "--project-dir", "/my/project"]).unwrap();
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/my/project"));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_defaults() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_doctor_custom_dir() {
    let cli = interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/health"]).unwrap();
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/health"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_version_flag() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayVersion
    );
}

#[test]
fn parse_no_args_shows_help() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn parse_unknown_subcommand() {
    let result = interpret_cli_from(vec!["vo", "foobar"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn parse_check_path_with_spaces() {
    let cli = interpret_cli_from(vec!["vo", "check", "/path/with spaces/bin"]).unwrap();
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => {
            assert_eq!(path, PathBuf::from("/path/with spaces/bin"));
        }
        _ => panic!("expected Check"),
    }
}
