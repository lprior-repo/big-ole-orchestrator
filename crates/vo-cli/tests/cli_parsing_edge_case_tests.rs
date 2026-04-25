use std::path::PathBuf;
use vo_cli::{interpret_cli_from, Command};

#[test]
fn parse_unknown_subcommand_fails() {
    let result = interpret_cli_from(vec!["vo", "foobar"]);
    assert!(result.is_err());
}

#[test]
fn parse_gc_with_unknown_flag_fails() {
    let result = interpret_cli_from(vec!["vo", "gc", "--unknown-flag"]);
    assert!(result.is_err());
}

#[test]
fn parse_check_with_extra_positional_rejected() {
    let result = interpret_cli_from(vec!["vo", "check", "/tmp/a", "/tmp/b"]);
    assert!(result.is_err());
}

#[test]
fn parse_doctor_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "doctor", "--project-dir", "/custom/proj"]).expect("parse");
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/custom/proj"));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_lock_custom_project_dir() {
    let cli =
        interpret_cli_from(vec!["vo", "lock", "--project-dir", "/my/workspace"]).expect("parse");
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("/my/workspace"));
        }
        _ => panic!("expected Lock"),
    }
}
