#![allow(clippy::redundant_pattern_matching)]
use std::path::PathBuf;
use vo_cli::{interpret_cli_from, Command};

#[test]
fn parse_rebuild_defaults() {
    let cli = interpret_cli_from(vec!["vo", "rebuild"]).expect("parse");
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
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--projection-id", "order-summary"])
        .expect("parse");
    match cli.command {
        Command::Rebuild { projection_id, .. } => {
            assert_eq!(projection_id.as_deref(), Some("order-summary"));
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_with_list_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--list"]).expect("parse");
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
fn parse_rebuild_with_force_flag() {
    let cli = interpret_cli_from(vec!["vo", "rebuild", "--force"]).expect("parse");
    match cli.command {
        Command::Rebuild { force, .. } => {
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
        "/my/proj",
        "--projection-id",
        "p1",
        "--list",
        "--force",
    ])
    .expect("parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from("/my/proj"));
            assert_eq!(projection_id.as_deref(), Some("p1"));
            assert!(list_projections);
            assert!(force);
        }
        _ => panic!("expected Rebuild"),
    }
}

#[test]
fn parse_rebuild_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "rebuild", "--project-dir", "/data/app"]).expect("parse");
    match cli.command {
        Command::Rebuild { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("/data/app"));
        }
        _ => panic!("expected Rebuild"),
    }
}
