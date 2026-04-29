use std::path::PathBuf;
use vo_cli::{interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command};

#[test]
fn parse_purge_with_instance_flag() {
    let cli = interpret_cli_from(["vo", "purge", "--instance", "abc-123"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Purge {
            instance: "abc-123".into()
        }
    );
}
#[test]
fn parse_check_with_path() {
    let cli = interpret_cli_from(["vo", "check", "/tmp/binary"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Check {
            workflow: false,
            path: PathBuf::from("/tmp/binary")
        }
    );
}
#[test]
fn parse_gc_defaults() {
    let cli = interpret_cli_from(["vo", "gc"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Gc {
            engine_url: "http://localhost:3000".into(),
            dry_run: false
        }
    );
}
#[test]
fn parse_gc_dry_run() {
    let cli = interpret_cli_from(["vo", "gc", "--dry-run"]).unwrap();
    let Command::Gc { dry_run, .. } = cli.command else {
        panic!("expected Gc")
    };
    assert!(dry_run);
}
#[test]
fn parse_init_with_custom_storage_path() {
    let cli = interpret_cli_from(["vo", "init", "--storage-path", "/data/vo"]).unwrap();
    let Command::Init { storage_path, .. } = &cli.command else {
        panic!("expected Init")
    };
    assert_eq!(storage_path, &PathBuf::from("/data/vo"));
}
#[test]
fn parse_lock_with_project_dir() {
    let cli = interpret_cli_from(["vo", "lock", "--project-dir", "./my-proj"]).unwrap();
    let Command::Lock { project_dir } = &cli.command else {
        panic!("expected Lock")
    };
    assert_eq!(project_dir, &PathBuf::from("./my-proj"));
}
#[test]
fn parse_doctor_defaults() {
    let cli = interpret_cli_from(["vo", "doctor"]).unwrap();
    assert_eq!(
        cli.command,
        Command::Doctor {
            project_dir: PathBuf::from(".")
        }
    );
}
#[test]
fn parse_rebuild_with_list_and_force() {
    let cli = interpret_cli_from(["vo", "rebuild", "--list", "--force"]).unwrap();
    let Command::Rebuild {
        list_projections,
        force,
        ..
    } = &cli.command
    else {
        panic!("expected Rebuild")
    };
    assert!(list_projections);
    assert!(force);
}
#[test]
fn parse_rebuild_with_projection_id() {
    let cli = interpret_cli_from(["vo", "rebuild", "--projection-id", "proj-1"]).unwrap();
    let Command::Rebuild { projection_id, .. } = &cli.command else {
        panic!("expected Rebuild")
    };
    assert_eq!(projection_id.as_deref(), Some("proj-1"));
}
#[test]
fn parse_compensate_all_flags() {
    let cli = interpret_cli_from([
        "vo",
        "compensate",
        "wf-42",
        "--engine-url",
        "http://prod:8080",
        "--force",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        Command::Compensate {
            engine_url: "http://prod:8080".into(),
            workflow_id: "wf-42".into(),
            force: true
        }
    );
}
#[test]
fn parse_status_with_custom_url() {
    let cli = interpret_cli_from([
        "vo",
        "status",
        "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "--engine-url",
        "http://staging:4000",
    ])
    .unwrap();
    let Command::Status {
        engine_url,
        instance,
    } = cli.command
    else {
        panic!("expected Status")
    };
    assert_eq!(engine_url, "http://staging:4000");
    assert_eq!(instance, "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV");
}
#[test]
fn missing_purge_instance_is_error() {
    assert!(interpret_cli_from(["vo", "purge"]).is_err());
}
#[test]
fn missing_check_path_is_error() {
    assert!(interpret_cli_from(["vo", "check"]).is_err());
}
#[test]
fn missing_compensate_workflow_id_is_error() {
    assert!(interpret_cli_from(["vo", "compensate"]).is_err());
}
#[test]
fn missing_status_instance_is_error() {
    assert!(interpret_cli_from(["vo", "status"]).is_err());
}
#[test]
fn unknown_subcommand_is_error() {
    assert!(interpret_cli_from(["vo", "deploy"]).is_err());
}
#[test]
fn no_subcommand_shows_help_error() {
    assert!(interpret_cli_from(["vo"]).is_err());
}
#[test]
fn numeric_valid() {
    assert_eq!(parse_strict_numeric("42").unwrap(), 42);
}
#[test]
fn numeric_rejects_empty() {
    assert!(matches!(
        parse_strict_numeric(""),
        Err(CliError::InvalidNumeric(_))
    ));
}
#[test]
fn numeric_rejects_plus() {
    assert!(matches!(
        parse_strict_numeric("+5"),
        Err(CliError::InvalidNumeric(_))
    ));
}
#[test]
fn numeric_rejects_negative() {
    assert!(matches!(
        parse_strict_numeric("-1"),
        Err(CliError::InvalidNumeric(_))
    ));
}
#[test]
fn numeric_rejects_overflow() {
    assert!(matches!(
        parse_strict_numeric("18446744073709551616"),
        Err(CliError::InvalidNumeric(_))
    ));
}
#[test]
fn numeric_rejects_alpha() {
    assert!(matches!(
        parse_strict_numeric("abc"),
        Err(CliError::InvalidNumeric(_))
    ));
}
#[test]
fn exit_code_invalid_numeric_is_two() {
    assert_eq!(
        map_error_to_exit_code(&CliError::InvalidNumeric("bad".into())),
        2
    );
}
#[test]
fn exit_code_dispatch_error_is_one() {
    assert_eq!(
        map_error_to_exit_code(&CliError::Dispatch("timeout".into())),
        1
    );
}
#[test]
fn error_display_includes_dispatch_message() {
    let msg = format!("{}", CliError::Dispatch("connection refused".into()));
    assert!(msg.contains("connection refused"));
}
#[test]
fn invalid_numeric_display_includes_reason() {
    let msg = format!("{}", CliError::InvalidNumeric("leading plus sign".into()));
    assert!(msg.contains("leading plus sign"));
}
