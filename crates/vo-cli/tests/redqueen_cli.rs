//! RED-QUEEN coevolutionary adversarial tests for vo-cli parsing.
//! Shell injection, flag abuse, path traversal, unicode, argument smuggling.

use std::path::PathBuf;
use vo_cli::cli::{interpret_cli_from, CliError, Command};

#[test]
fn rq_purge_shell_metachars_pass_through_untampered() {
    let payload = "$(rm -rf /)";
    let cli = interpret_cli_from(["vo", "purge", "--instance", payload]).unwrap();
    assert_eq!(cli.command, Command::Purge { instance: payload.into() });
}

#[test]
fn rq_check_backtick_command_substitution_is_literal() {
    let payload = "`cat /etc/shadow`";
    let cli = interpret_cli_from(["vo", "check", payload]).unwrap();
    assert_eq!(cli.command, Command::Check { workflow: false, path: PathBuf::from(payload) });
}

#[test]
fn rq_check_null_byte_does_not_truncate() {
    let payload = "safe.txt\0;malware";
    let cli = interpret_cli_from(["vo", "check", payload]).unwrap();
    assert_eq!(cli.command, Command::Check { workflow: false, path: PathBuf::from(payload) });
}

#[test]
fn rq_init_absolute_path_traversal_accepted() {
    let cli = interpret_cli_from(["vo", "init", "--project-dir", "/etc/"]).unwrap();
    if let Command::Init { project_dir, .. } = cli.command {
        assert_eq!(project_dir, PathBuf::from("/etc/"));
    }
}

#[test]
fn rq_storage_path_traversal_stored_raw() {
    let cli = interpret_cli_from(["vo", "init", "--storage-path", "/tmp/../../etc/passwd"]).unwrap();
    if let Command::Init { storage_path, .. } = cli.command {
        assert_eq!(storage_path, PathBuf::from("/tmp/../../etc/passwd"));
    }
}

#[test]
fn rq_double_dash_escape_between_flags() {
    let cli = interpret_cli_from(["vo", "check", "--", "--not-a-flag"]).unwrap();
    assert_eq!(cli.command, Command::Check { workflow: false, path: PathBuf::from("--not-a-flag") });
}

#[test]
fn rq_compensate_rejects_empty_workflow_id() {
    assert!(interpret_cli_from(["vo", "compensate", ""]).is_err());
}

#[test]
fn rq_purge_without_instance_flag_fails() {
    assert!(interpret_cli_from(["vo", "purge"]).is_err());
}

#[test]
fn rq_rebuild_force_and_list_together() {
    let cli = interpret_cli_from(["vo", "rebuild", "--force", "--list"]).unwrap();
    if let Command::Rebuild { force, list_projections, .. } = cli.command {
        assert!(force && list_projections);
    }
}

#[test]
fn rq_gc_dry_run_with_custom_engine_url() {
    let cli = interpret_cli_from(["vo", "gc", "--dry-run", "--engine-url", "http://evil:9000"]).unwrap();
    if let Command::Gc { dry_run, engine_url, .. } = cli.command {
        assert!(dry_run);
        assert_eq!(engine_url, "http://evil:9000");
    }
}

#[test]
fn rq_compensate_unicode_spoof_id_stored_raw() {
    let payload = "wf-\u{FF11}\u{FF12}\u{FF13}"; // fullwidth １２３
    let cli = interpret_cli_from(["vo", "compensate", payload]).unwrap();
    if let Command::Compensate { workflow_id, .. } = cli.command {
        assert_eq!(workflow_id, payload);
    }
}

#[test]
fn rq_check_right_to_left_override_in_path() {
    let payload = "normal\u{202E}txt.gpj"; // RLO spoof
    let cli = interpret_cli_from(["vo", "check", payload]).unwrap();
    assert_eq!(cli.command, Command::Check { workflow: false, path: PathBuf::from(payload) });
}

#[test]
fn rq_compensate_file_scheme_url_stored_raw() {
    let cli = interpret_cli_from(["vo", "compensate", "wf-1", "--engine-url", "file:///etc/shadow"]).unwrap();
    if let Command::Compensate { engine_url, .. } = cli.command {
        assert_eq!(engine_url, "file:///etc/shadow");
    }
}

#[test]
fn rq_missing_subcommand_maps_to_exit_2() {
    let err = interpret_cli_from(["vo"]).unwrap_err();
    assert_eq!(vo_cli::cli::map_error_to_exit_code(&CliError::from(err)), 2);
}

#[test]
fn rq_unknown_flag_maps_to_exit_2() {
    let err = interpret_cli_from(["vo", "check", "--bogus"]).unwrap_err();
    assert_eq!(vo_cli::cli::map_error_to_exit_code(&CliError::from(err)), 2);
}

#[test]
fn rq_check_path_4kb_accepted() {
    let payload: String = "a".repeat(4096);
    let cli = interpret_cli_from(["vo", "check", &payload]).unwrap();
    assert_eq!(cli.command, Command::Check { workflow: false, path: PathBuf::from(&payload) });
}

#[test]
fn rq_compensate_instance_id_64kb_accepted() {
    let payload: String = "x".repeat(65536);
    let cli = interpret_cli_from(["vo", "compensate", &payload]).unwrap();
    if let Command::Compensate { workflow_id, .. } = cli.command {
        assert_eq!(workflow_id.len(), 65536);
    }
}
