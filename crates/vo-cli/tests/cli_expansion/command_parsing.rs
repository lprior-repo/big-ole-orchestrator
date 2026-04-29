use std::path::PathBuf;
use vo_cli::{interpret_cli_from, Command};

#[test]
fn parse_init_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_defaults_engine_url_is_localhost() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { engine_url, .. } => {
            assert_eq!(engine_url, "http://localhost:3000");
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_defaults_storage_path() {
    let cli = interpret_cli_from(vec!["vo", "init"]).expect("parse");
    match cli.command {
        Command::Init { storage_path, .. } => {
            assert_eq!(storage_path, PathBuf::from(".vo/storage"));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_lock_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "lock"]).expect("parse");
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Lock"),
    }
}

#[test]
fn parse_doctor_defaults_project_dir_is_dot() {
    let cli = interpret_cli_from(vec!["vo", "doctor"]).expect("parse");
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from("."));
        }
        _ => panic!("expected Doctor"),
    }
}

#[test]
fn parse_check_with_special_chars_in_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "/tmp/test@#$/bin"]).expect("parse");
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => {
            assert_eq!(path, PathBuf::from("/tmp/test@#$/bin"));
        }
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_check_with_unicode_path() {
    let cli = interpret_cli_from(vec!["vo", "check", "/tmp/日本語/binary"]).expect("parse");
    match cli.command {
        Command::Check {
            workflow: false,
            path,
        } => {
            assert_eq!(path, PathBuf::from("/tmp/日本語/binary"));
        }
        _ => panic!("expected Check"),
    }
}

#[test]
fn parse_purge_with_uuid_instance() {
    let cli = interpret_cli_from(vec![
        "vo",
        "purge",
        "--instance",
        "550e8400-e29b-41d4-a716-446655440000",
    ])
    .expect("parse");
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "550e8400-e29b-41d4-a716-446655440000");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_purge_empty_instance_is_accepted() {
    let cli = interpret_cli_from(vec!["vo", "purge", "--instance", ""]).expect("parse");
    match cli.command {
        Command::Purge { instance } => {
            assert_eq!(instance, "");
        }
        _ => panic!("expected Purge"),
    }
}

#[test]
fn parse_gc_rejects_empty_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--engine-url", ""]).expect("parse");
    match cli.command {
        Command::Gc { engine_url, .. } => {
            assert_eq!(engine_url, "");
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_init_with_empty_project_dir() {
    let cli = interpret_cli_from(vec!["vo", "init", "--project-dir", ""]).expect("parse");
    match cli.command {
        Command::Init { project_dir, .. } => {
            assert_eq!(project_dir, PathBuf::from(""));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_init_with_empty_storage_path() {
    let cli = interpret_cli_from(vec!["vo", "init", "--storage-path", ""]).expect("parse");
    match cli.command {
        Command::Init { storage_path, .. } => {
            assert_eq!(storage_path, PathBuf::from(""));
        }
        _ => panic!("expected Init"),
    }
}

#[test]
fn parse_gc_env_override_for_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse");
    match cli.command {
        Command::Gc { engine_url, .. } => {
            assert_eq!(engine_url, "http://localhost:3000");
        }
        _ => panic!("expected Gc"),
    }
}

#[test]
fn parse_all_subcommands_are_recognized() {
    let subcommands: Vec<&str> = vec!["purge", "check", "gc", "init", "lock", "doctor"];
    for sub in subcommands {
        let args: Vec<&str> = match sub {
            "purge" => vec!["vo", sub, "--instance", "x"],
            "check" => vec!["vo", sub, "/tmp/f"],
            _ => vec!["vo", sub],
        };
        let result = interpret_cli_from(args);
        assert!(result.is_ok(), "subcommand '{}' should parse", sub);
    }
}

#[test]
fn parse_subcommand_without_required_arg_fails() {
    let result = interpret_cli_from(vec!["vo", "purge"]);
    assert!(result.is_err());
}

#[test]
fn parse_double_dash_before_subcommand() {
    let result = interpret_cli_from(vec!["vo", "--", "check", "/tmp/f"]);
    assert!(result.is_err());
}

#[test]
fn parse_version_output_kind() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayVersion
    );
}

#[test]
fn parse_no_args_returns_help_on_missing() {
    let result = interpret_cli_from(vec!["vo"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}
