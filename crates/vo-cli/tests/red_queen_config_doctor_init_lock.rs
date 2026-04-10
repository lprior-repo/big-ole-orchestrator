use std::ffi::OsString;
use vo_cli::{interpret_cli_from, map_error_to_exit_code, CliError, Command};

// ============================================================
// RED QUEEN: Adversarial testing for config/doctor/init/lock
// ============================================================
// These commands do not yet exist. The Red Queen verifies:
// 1. Unknown subcommands are properly rejected (not silently accepted)
// 2. Error types and exit codes are correct for rejected commands
// 3. Partial matches and typos are rejected (not matched to existing commands)
// 4. The CLI contract holds: subcommand_required(true) + arg_required_else_help(true)

// ============================================================
// Dimension: unknown-subcommand-rejection
// ============================================================

#[test]
fn rq_config_subcommand_rejected_as_invalid() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config subcommand should be rejected (does not exist)"
    );
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "config should be InvalidSubcommand, got {:?}",
        err.kind()
    );
}

#[test]
fn rq_doctor_subcommand_rejected_as_invalid() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "doctor subcommand should be rejected (does not exist)"
    );
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "doctor should be InvalidSubcommand, got {:?}",
        err.kind()
    );
}

#[test]
fn rq_init_subcommand_rejected_as_invalid() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "init subcommand should be rejected (does not exist)"
    );
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "init should be InvalidSubcommand, got {:?}",
        err.kind()
    );
}

#[test]
fn rq_lock_subcommand_rejected_as_invalid() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "lock subcommand should be rejected (does not exist)"
    );
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::InvalidSubcommand,
        "lock should be InvalidSubcommand, got {:?}",
        err.kind()
    );
}

// ============================================================
// Dimension: exit-code-correctness
// ============================================================

#[test]
fn rq_config_rejected_maps_to_exit_code_2() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into()];
    let result = interpret_cli_from(args).expect_err("should be error");
    let code = map_error_to_exit_code(&CliError::Clap(result));
    assert_eq!(code, 2, "invalid subcommand should exit with code 2");
}

#[test]
fn rq_doctor_rejected_maps_to_exit_code_2() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into()];
    let result = interpret_cli_from(args).expect_err("should be error");
    let code = map_error_to_exit_code(&CliError::Clap(result));
    assert_eq!(code, 2, "invalid subcommand should exit with code 2");
}

#[test]
fn rq_init_rejected_maps_to_exit_code_2() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into()];
    let result = interpret_cli_from(args).expect_err("should be error");
    let code = map_error_to_exit_code(&CliError::Clap(result));
    assert_eq!(code, 2, "invalid subcommand should exit with code 2");
}

#[test]
fn rq_lock_rejected_maps_to_exit_code_2() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into()];
    let result = interpret_cli_from(args).expect_err("should be error");
    let code = map_error_to_exit_code(&CliError::Clap(result));
    assert_eq!(code, 2, "invalid subcommand should exit with code 2");
}

// ============================================================
// Dimension: typo-squatting-rejection
// ============================================================

#[test]
fn rq_typo_confing_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "confing".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'confing' should be rejected");
}

#[test]
fn rq_typo_docter_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "docter".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'docter' should be rejected");
}

#[test]
fn rq_typo_innit_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "innit".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'innit' should be rejected");
}

#[test]
fn rq_typo_lck_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "lck".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'lck' should be rejected");
}

// ============================================================
// Dimension: argument-injection
// ============================================================

#[test]
fn rq_config_with_path_arg_still_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into(), "/etc/passwd".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config with path arg should still be rejected"
    );
}

#[test]
fn rq_doctor_with_flag_still_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into(), "--force".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "doctor with --force flag should still be rejected"
    );
}

#[test]
fn rq_init_with_name_still_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into(), "my-project".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "init with project name should still be rejected"
    );
}

#[test]
fn rq_lock_with_instance_still_rejected() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "lock".into(),
        "--instance".into(),
        "abc123".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "lock with --instance should still be rejected"
    );
}

// ============================================================
// Dimension: case-sensitivity
// ============================================================

#[test]
fn rq_config_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "CONFIG".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase CONFIG should be rejected");
}

#[test]
fn rq_doctor_mixed_case_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "Doctor".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "mixed-case Doctor should be rejected");
}

#[test]
fn rq_init_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "INIT".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase INIT should be rejected");
}

#[test]
fn rq_lock_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "LOCK".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase LOCK should be rejected");
}

// ============================================================
// Dimension: empty-and-whitespace
// ============================================================

#[test]
fn rq_empty_string_subcommand_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "empty string subcommand should be rejected"
    );
}

#[test]
fn rq_whitespace_subcommand_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), " ".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "whitespace subcommand should be rejected");
}

// ============================================================
// Dimension: path-traversal
// ============================================================

#[test]
fn rq_config_with_dotdot_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into(), "../../etc/shadow".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config with path traversal should be rejected"
    );
}

#[test]
fn rq_init_with_dotdot_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into(), "../../../tmp".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "init with path traversal should be rejected"
    );
}

// ============================================================
// Dimension: existing-commands-still-work
// ============================================================

#[test]
fn rq_existing_check_command_unaffected_by_testing() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/usr/bin/ls".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "existing check command must still parse");
    assert!(matches!(result.expect("ok").command, Command::Check { .. }));
}

#[test]
fn rq_existing_gc_command_unaffected_by_testing() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "existing gc command must still parse");
    assert!(matches!(result.expect("ok").command, Command::Gc { .. }));
}

#[test]
fn rq_existing_purge_command_unaffected_by_testing() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test-id".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "existing purge command must still parse");
    assert!(matches!(result.expect("ok").command, Command::Purge { .. }));
}

// ============================================================
// Dimension: no-args-and-help
// ============================================================

#[test]
fn rq_no_subcommand_returns_help_error() {
    let args: Vec<OsString> = vec!["vo".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "no subcommand should fail");
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
    );
}

#[test]
fn rq_help_flag_returns_display_help() {
    let args: Vec<OsString> = vec!["vo".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "--help should produce error (displays help)"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn rq_config_help_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "config --help should be rejected");
}

// ============================================================
// Dimension: concurrent-nonexistent-commands
// ============================================================

#[test]
fn rq_multiple_unknown_subcommands_each_rejected() {
    for cmd in &["config", "doctor", "init", "lock"] {
        let args: Vec<OsString> = vec!["vo".into(), (*cmd).into()];
        let result = interpret_cli_from(args);
        assert!(result.is_err(), "subcommand '{}' should be rejected", cmd);
    }
}

#[test]
fn rq_known_and_unknown_subcommands_dont_interfere() {
    let known_ok: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/true".into()];
    let known_result = interpret_cli_from(known_ok);
    assert!(known_result.is_ok(), "check /bin/true should parse");

    for cmd in &["config", "doctor", "init", "lock"] {
        let args: Vec<OsString> = vec!["vo".into(), (*cmd).into()];
        let result = interpret_cli_from(args);
        assert!(
            result.is_err(),
            "after parsing check, '{}' should still be rejected",
            cmd
        );
    }
}

// ============================================================
// Dimension: unicode-and-special-chars
// ============================================================

#[test]
fn rq_unicode_subcommand_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "cönfig".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "unicode subcommand should be rejected");
}

#[test]
fn rq_null_byte_subcommand_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config\0".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "null-byte subcommand should be rejected");
}

#[test]
fn rq_newline_subcommand_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config\ncheck".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "newline-injection subcommand should be rejected"
    );
}

// ============================================================
// Dimension: very-long-input
// ============================================================

#[test]
fn rq_very_long_subcommand_rejected() {
    let long_cmd = "a".repeat(10000);
    let args: Vec<OsString> = vec!["vo".into(), long_cmd.into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "very long subcommand should be rejected");
}

#[test]
fn rq_config_with_very_long_arg_rejected() {
    let long_arg = "x".repeat(100000);
    let args: Vec<OsString> = vec!["vo".into(), "config".into(), long_arg.into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config with very long arg should be rejected"
    );
}

// ============================================================
// Dimension: hyphenation-confusion
// ============================================================

#[test]
fn rq_dashed_config_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--config".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--config flag should be rejected");
}

#[test]
fn rq_dashed_doctor_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--doctor".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--doctor flag should be rejected");
}

#[test]
fn rq_dashed_init_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--init".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--init flag should be rejected");
}

#[test]
fn rq_dashed_lock_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--lock".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--lock flag should be rejected");
}

// ============================================================
// Dimension: env-var-injection
// ============================================================

#[test]
fn rq_config_with_equals_arg_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into(), "key=value".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config with key=value arg should be rejected"
    );
}

// ============================================================
// Dimension: flag-combination-injection
// ============================================================

#[test]
fn rq_config_with_all_flags_rejected() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "config".into(),
        "--help".into(),
        "--version".into(),
        "-v".into(),
        "-h".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "config with multiple flags should be rejected"
    );
}

#[test]
fn rq_init_with_double_dash_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into(), "--".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "init with -- separator should be rejected");
}

// ============================================================
// Dimension: subcommand-prefix-confusion
// ============================================================

#[test]
fn rq_check_config_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "check-config".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "check-config should be rejected (not a valid subcommand)"
    );
}

#[test]
fn rq_gc_lock_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "gc-lock".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "gc-lock should be rejected (not a valid subcommand)"
    );
}
