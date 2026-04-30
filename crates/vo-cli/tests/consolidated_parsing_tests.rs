//! Consolidated CLI argument parsing tests.
//!
//! Replaces duplicated parsing tests from 7+ test files using rstest
//! parameterized test cases.

use std::path::PathBuf;

use vo_cli::{interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command};

// ---------------------------------------------------------------------------
// Init subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::defaults(vec!["vo", "init"], ".", "http://localhost:3000", ".vo/storage")]
#[case::custom_project_dir(
    vec!["vo", "init", "--project-dir", "/custom/dir"],
    "/custom/dir", "http://localhost:3000", ".vo/storage"
)]
#[case::all_custom(
    vec!["vo", "init", "--project-dir", "/my/project", "--engine-url", "http://engine:8080", "--storage-path", "/data/vo"],
    "/my/project", "http://engine:8080", "/data/vo"
)]
#[case::engine_url_only(
    vec!["vo", "init", "--engine-url", "http://engine:8080"],
    ".", "http://engine:8080", ".vo/storage"
)]
#[case::project_dir_only(
    vec!["vo", "init", "--project-dir", "/my/project"],
    "/my/project", "http://localhost:3000", ".vo/storage"
)]
#[case::storage_path_only(
    vec!["vo", "init", "--storage-path", "/data/vo"],
    ".", "http://localhost:3000", "/data/vo"
)]
fn parse_init_command(
    #[case] args: Vec<&str>,
    #[case] expected_project_dir: &str,
    #[case] expected_engine_url: &str,
    #[case] expected_storage_path: &str,
) {
    let cli = interpret_cli_from(args).expect("init should parse");
    match cli.command {
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            assert_eq!(project_dir, PathBuf::from(expected_project_dir));
            assert_eq!(engine_url, expected_engine_url);
            assert_eq!(storage_path, PathBuf::from(expected_storage_path));
        }
        _ => panic!("expected Init command"),
    }
}

// ---------------------------------------------------------------------------
// GC subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::defaults(vec!["vo", "gc"], "http://localhost:3000", false)]
#[case::custom_engine_url(vec!["vo", "gc", "--engine-url", "http://engine:4000"], "http://engine:4000", false)]
#[case::dry_run(vec!["vo", "gc", "--dry-run"], "http://localhost:3000", true)]
#[case::dry_run_and_engine_url(vec!["vo", "gc", "--dry-run", "--engine-url", "http://e:1"], "http://e:1", true)]
fn parse_gc_command(
    #[case] args: Vec<&str>,
    #[case] expected_engine_url: &str,
    #[case] expected_dry_run: bool,
) {
    let cli = interpret_cli_from(args).expect("gc should parse");
    match cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, expected_engine_url);
            assert_eq!(dry_run, expected_dry_run);
        }
        _ => panic!("expected Gc command"),
    }
}

// ---------------------------------------------------------------------------
// Rebuild subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::defaults(vec!["vo", "rebuild"], ".", None::<&str>, false, false)]
#[case::projection_id(vec!["vo", "rebuild", "--projection-id", "my-proj"], ".", Some("my-proj"), false, false)]
#[case::list_only(vec!["vo", "rebuild", "--list"], ".", None::<&str>, true, false)]
#[case::force_only(vec!["vo", "rebuild", "--force"], ".", None::<&str>, false, true)]
#[case::force_with_projection(
    vec!["vo", "rebuild", "--projection-id", "proj-1", "--force"],
    ".", Some("proj-1"), false, true
)]
#[case::list_and_force(vec!["vo", "rebuild", "--list", "--force"], ".", None::<&str>, true, true)]
#[case::all_flags(
    vec!["vo", "rebuild", "--project-dir", "/tmp", "--projection-id", "p1", "--list", "--force"],
    "/tmp", Some("p1"), true, true
)]
#[case::custom_project_dir(vec!["vo", "rebuild", "--project-dir", "/data/app"], "/data/app", None::<&str>, false, false)]
fn parse_rebuild_command(
    #[case] args: Vec<&str>,
    #[case] expected_project_dir: &str,
    #[case] expected_projection_id: Option<&str>,
    #[case] expected_list: bool,
    #[case] expected_force: bool,
) {
    let cli = interpret_cli_from(args).expect("rebuild should parse");
    match cli.command {
        Command::Rebuild {
            project_dir,
            projection_id,
            list_projections,
            force,
        } => {
            assert_eq!(project_dir, PathBuf::from(expected_project_dir));
            assert_eq!(projection_id.as_deref(), expected_projection_id);
            assert_eq!(list_projections, expected_list);
            assert_eq!(force, expected_force);
        }
        _ => panic!("expected Rebuild command"),
    }
}

// ---------------------------------------------------------------------------
// Lock subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::defaults(vec!["vo", "lock"], ".")]
#[case::custom_dir(vec!["vo", "lock", "--project-dir", "/my/project"], "/my/project")]
fn parse_lock_command(#[case] args: Vec<&str>, #[case] expected_project_dir: &str) {
    let cli = interpret_cli_from(args).expect("lock should parse");
    match cli.command {
        Command::Lock { project_dir } => {
            assert_eq!(project_dir, PathBuf::from(expected_project_dir));
        }
        _ => panic!("expected Lock command"),
    }
}

// ---------------------------------------------------------------------------
// Doctor subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::defaults(vec!["vo", "doctor"], ".")]
#[case::custom_dir(vec!["vo", "doctor", "--project-dir", "/health"], "/health")]
fn parse_doctor_command(#[case] args: Vec<&str>, #[case] expected_project_dir: &str) {
    let cli = interpret_cli_from(args).expect("doctor should parse");
    match cli.command {
        Command::Doctor { project_dir } => {
            assert_eq!(project_dir, PathBuf::from(expected_project_dir));
        }
        _ => panic!("expected Doctor command"),
    }
}

// ---------------------------------------------------------------------------
// Purge subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::basic(vec!["vo", "purge", "--instance", "inst-123"], "inst-123", false)]
#[case::empty_instance(vec!["vo", "purge", "--instance", ""], "", false)]
#[case::uuid_format(
    vec!["vo", "purge", "--instance", "550e8400-e29b-41d4-a716-446655440000"],
    "550e8400-e29b-41d4-a716-446655440000", false
)]
#[case::numeric_string(vec!["vo", "purge", "--instance", "12345"], "12345", false)]
#[case::with_dry_run(vec!["vo", "purge", "--instance", "inst-1", "--dry-run"], "inst-1", true)]
fn parse_purge_command(
    #[case] args: Vec<&str>,
    #[case] expected_instance: &str,
    #[case] expected_dry_run: bool,
) {
    let cli = interpret_cli_from(args).expect("purge should parse");
    match cli.command {
        Command::Purge {
            instance,
            storage_path: _,
            dry_run,
        } => {
            assert_eq!(instance, expected_instance);
            assert_eq!(dry_run, expected_dry_run);
        }
        _ => panic!("expected Purge command"),
    }
}

// ---------------------------------------------------------------------------
// Check subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::absolute_path(vec!["vo", "check", "/usr/bin/ls"], false, "/usr/bin/ls")]
#[case::relative_path(vec!["vo", "check", "../bin/app"], false, "../bin/app")]
#[case::dot(vec!["vo", "check", "."], false, ".")]
fn parse_check_command(
    #[case] args: Vec<&str>,
    #[case] expected_workflow: bool,
    #[case] expected_path: &str,
) {
    let cli = interpret_cli_from(args).expect("check should parse");
    match cli.command {
        Command::Check { workflow, path } => {
            assert_eq!(workflow, expected_workflow);
            assert_eq!(path, PathBuf::from(expected_path));
        }
        _ => panic!("expected Check command"),
    }
}

// ---------------------------------------------------------------------------
// Compensate subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::all_flags(
    vec!["vo", "compensate", "wf-42", "--engine-url", "http://prod:8080", "--force"],
    "wf-42", "http://prod:8080", true
)]
#[case::defaults(vec!["vo", "compensate", "wf-abc123"], "wf-abc123", "http://localhost:3000", false)]
fn parse_compensate_command(
    #[case] args: Vec<&str>,
    #[case] expected_workflow_id: &str,
    #[case] expected_engine_url: &str,
    #[case] expected_force: bool,
) {
    let cli = interpret_cli_from(args).expect("compensate should parse");
    match cli.command {
        Command::Compensate {
            engine_url,
            workflow_id,
            force,
        } => {
            assert_eq!(workflow_id, expected_workflow_id);
            assert_eq!(engine_url, expected_engine_url);
            assert_eq!(force, expected_force);
        }
        _ => panic!("expected Compensate command"),
    }
}

// ---------------------------------------------------------------------------
// Status subcommand parsing
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::custom_url(
    vec!["vo", "status", "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV", "--engine-url", "http://staging:4000"],
    "ns/01ARZ3NDEKTSV4RRFFQ69G5FAV", "http://staging:4000"
)]
#[case::defaults(vec!["vo", "status", "01ARZ3NDEKTSV4RRFFQ69G5FAV"], "01ARZ3NDEKTSV4RRFFQ69G5FAV", "http://localhost:3000")]
fn parse_status_command(
    #[case] args: Vec<&str>,
    #[case] expected_workflow_id: &str,
    #[case] expected_engine_url: &str,
) {
    let cli = interpret_cli_from(args).expect("status should parse");
    match cli.command {
        Command::Status {
            engine_url,
            workflow_id,
        } => {
            assert_eq!(workflow_id, expected_workflow_id);
            assert_eq!(engine_url, expected_engine_url);
        }
        _ => panic!("expected Status command"),
    }
}

// ---------------------------------------------------------------------------
// Missing required arguments
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::no_args(vec!["vo"])]
#[case::unknown_subcommand(vec!["vo", "foobar"])]
fn parse_invalid_commands_fails(#[case] args: Vec<&str>) {
    assert!(interpret_cli_from(args).is_err());
}

#[rstest::rstest]
#[case::purge_no_instance(vec!["vo", "purge"])]
#[case::check_no_path(vec!["vo", "check"])]
#[case::compensate_no_workflow_id(vec!["vo", "compensate"])]
#[case::status_no_instance(vec!["vo", "status"])]
#[case::hardline_no_target(vec!["vo", "hardline"])]
#[case::history_no_instance(vec!["vo", "history"])]
fn parse_missing_required_field_fails(#[case] args: Vec<&str>) {
    assert!(interpret_cli_from(args).is_err());
}

// ---------------------------------------------------------------------------
// Version, help, and extra-argument rejection
// ---------------------------------------------------------------------------

#[test]
fn parse_version_flag() {
    let result = interpret_cli_from(vec!["vo", "--version"]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayVersion
    ));
}

#[test]
fn parse_no_args_shows_help() {
    assert!(interpret_cli_from(vec!["vo"]).is_err());
}

#[test]
fn parse_check_with_extra_positional_rejected() {
    assert!(interpret_cli_from(vec!["vo", "check", "/tmp/a", "/tmp/b"]).is_err());
}

#[test]
fn parse_gc_with_unknown_flag_fails() {
    assert!(interpret_cli_from(vec!["vo", "gc", "--unknown-flag"]).is_err());
}

// ---------------------------------------------------------------------------
// Exit code mapping
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::clap_help(
    CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayHelp)),
    0
)]
#[case::clap_version(
    CliError::Clap(clap::Error::new(clap::error::ErrorKind::DisplayVersion)),
    0
)]
#[case::invalid_numeric(CliError::InvalidNumeric("bad".into()), 2)]
#[case::dispatch(CliError::Dispatch("fail".into()), 1)]
fn exit_code_mapping(#[case] err: CliError, #[case] expected: i32) {
    assert_eq!(map_error_to_exit_code(&err), expected);
}

// ---------------------------------------------------------------------------
// parse_strict_numeric
// ---------------------------------------------------------------------------

#[rstest::rstest]
#[case::zero("0", Some(0u64))]
#[case::one("1", Some(1))]
#[case::large("999999", Some(999999))]
#[case::leading_zeros("007", Some(7))]
#[case::max_u64("18446744073709551615", Some(u64::MAX))]
#[case::trillion("1000000000000", Some(1000000000000))]
#[case::negative("-1", None)]
#[case::minus_zero("-0", None)]
#[case::leading_plus("+42", None)]
#[case::empty("", None)]
#[case::letters("abc", None)]
#[case::overflow("18446744073709551616", None)]
#[case::float("3.14", None)]
#[case::binary("0b1010", None)]
#[case::hex("0x10", None)]
#[case::alphanumeric("12abc34", None)]
#[case::space_prefix(" 42", None)]
#[case::space_suffix("42 ", None)]
#[case::tab_prefix("\t42", None)]
#[case::newline_suffix("42\n", None)]
fn parse_strict_numeric_cases(#[case] input: &str, #[case] expected: Option<u64>) {
    let result = parse_strict_numeric(input);
    match expected {
        Some(val) => assert_eq!(result.unwrap(), val, "input: {input:?}"),
        None => assert!(result.is_err(), "input: {input:?} should be rejected"),
    }
}
