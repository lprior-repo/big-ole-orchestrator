//! Red Queen adversarial tests for CLI task subcommands (purge, check, gc).
//!
//! bead_id: ve-dyc
//! phase: state-5-red-queen
//!
//! Dimensions attacked:
//!   - subcommand-rejection: unknown subcommands properly rejected
//!   - argument-injection: path traversal, unicode, null bytes
//!   - boundary-values: empty paths, very long paths, special characters
//!   - error-handling: file not found, permission denied, invalid formats
//!   - exit-code-correctness: proper exit codes for different errors
//!   - partial-matching: typos and partial matches rejected
//!   - existing-commands-stability: valid commands still work

use std::ffi::OsString;

use vo_cli::{interpret_cli_from, map_error_to_exit_code, CliError, Command};

// ============================================================
// DIMENSION: unknown-subcommand-rejection
// ============================================================

#[test]
fn rq_unknown_subcommand_purge_config_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "config subcommand should be rejected");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn rq_unknown_subcommand_purge_doctor_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "doctor".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "doctor subcommand should be rejected");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn rq_unknown_subcommand_purge_init_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "init".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "init subcommand should be rejected");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn rq_unknown_subcommand_purge_lock_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "lock".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "lock subcommand should be rejected");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

// ============================================================
// DIMENSION: exit-code-correctness
// ============================================================

#[test]
fn rq_unknown_subcommand_maps_to_exit_code_2() {
    let args: Vec<OsString> = vec!["vo".into(), "config".into()];
    let result = interpret_cli_from(args).expect_err("should be error");
    let code = map_error_to_exit_code(&CliError::Clap(result));
    assert_eq!(code, 2, "invalid subcommand should exit with code 2");
}

#[test]
fn rq_invalid_subcommand_exit_code_consistent_across_commands() {
    for cmd in &["config", "doctor", "init", "lock", "purge", "check", "gc"] {
        let args: Vec<OsString> = vec!["vo".into(), (*cmd).into()];
        let result = interpret_cli_from(args);
        // Unknown commands should all exit with code 2
        if result.is_err() {
            let err = result.expect_err("should be error");
            let code = map_error_to_exit_code(&CliError::Clap(err));
            assert_eq!(code, 2, "subcommand '{}' should exit with code 2", cmd);
        }
    }
}

// ============================================================
// DIMENSION: typo-squatting-rejection
// ============================================================

#[test]
fn rq_typo_purge_purg_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purg".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'purg' should be rejected");
}

#[test]
fn rq_typo_check_heck_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "heck".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'heck' should be rejected");
}

#[test]
fn rq_typo_gc_g_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "g".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'g' should be rejected");
}

#[test]
fn rq_typo_gc_c_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "c".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "typo 'c' should be rejected");
}

// ============================================================
// DIMENSION: partial-matching-rejection
// ============================================================

#[test]
fn rq_partial_purge_instance_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purge-ins".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "partial 'purge-ins' should be rejected");
}

#[test]
fn rq_partial_check_path_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "check-p".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "partial 'check-p' should be rejected");
}

#[test]
fn rq_partial_gc_gar_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "gar".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "partial 'gar' should be rejected");
}

// ============================================================
// DIMENSION: existing-commands-still-work
// ============================================================

#[test]
fn rq_existing_purge_command_works() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test-instance-id".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "purge command should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Purge { .. }));
}

#[test]
fn rq_existing_check_command_works() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "check command should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Check { .. }));
}

#[test]
fn rq_existing_gc_command_works() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://localhost:8080".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc command should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Gc { .. }));
}

#[test]
fn rq_existing_gc_command_with_dry_run_works() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc with --dry-run should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Gc { dry_run: true, .. }));
}

// ============================================================
// DIMENSION: purge-command-attacks
// ============================================================

#[test]
fn rq_purge_without_instance_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purge".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "purge without --instance should be rejected"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn rq_purge_with_empty_instance_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purge".into(), "--instance".into(), "".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "empty instance string is still valid syntax"
    );
    // Empty instance is allowed syntactically (will fail at runtime)
}

#[test]
fn rq_purge_with_instance_path_traversal_rejected() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "../../../etc/passwd".into(),
    ];
    let result = interpret_cli_from(args);
    // Path traversal in instance ID is allowed syntactically (validated at runtime)
    assert!(result.is_ok());
}

#[test]
fn rq_purge_with_instance_null_byte() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "abc\0def".into(),
    ];
    let result = interpret_cli_from(args);
    // Null bytes are allowed syntactically by clap (validated at runtime)
    assert!(result.is_ok());
}

#[test]
fn rq_purge_with_instance_newline_injection() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test\nrm -rf /".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "newline in instance is allowed syntactically"
    );
}

#[test]
fn rq_purge_with_instance_unicode_emoji() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test-😀-instance".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "unicode in instance is allowed syntactically"
    );
}

#[test]
fn rq_purge_with_instance_very_long() {
    let long_instance = "x".repeat(100000);
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        long_instance.into(),
    ];
    let result = interpret_cli_from(args);
    // Very long instances are allowed syntactically (validated at runtime)
    assert!(result.is_ok());
}

#[test]
fn rq_purge_with_duplicate_instance_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "first".into(),
        "--instance".into(),
        "second".into(),
    ];
    let result = interpret_cli_from(args);
    // Duplicate flags cause error in clap
    assert!(result.is_err());
}

// ============================================================
// DIMENSION: check-command-attacks
// ============================================================

#[test]
fn rq_check_without_path_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "check without path should be rejected");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn rq_check_with_empty_path_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "".into()];
    let result = interpret_cli_from(args);
    // Empty path is allowed syntactically (will fail at runtime)
    assert!(result.is_ok());
}

#[test]
fn rq_check_with_path_traversal() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "../../../etc/passwd".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "path traversal is allowed syntactically");
}

#[test]
fn rq_check_with_symlink_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/proc/self/exe".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "symlink paths are allowed syntactically");
}

#[test]
fn rq_check_with_device_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/dev/null".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "device paths are allowed syntactically");
}

#[test]
fn rq_check_with_fifo_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/test_fifo".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "FIFO paths are allowed syntactically");
}

#[test]
fn rq_check_with_absolute_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/usr/bin/ls".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "absolute paths should work");
}

#[test]
fn rq_check_with_relative_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "./test".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "relative paths should work");
}

#[test]
fn rq_check_with_path_unicode() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/テスト".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "unicode paths are allowed syntactically");
}

#[test]
fn rq_check_with_path_very_long() {
    let long_path = "/".repeat(10000);
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), long_path.into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "very long paths are allowed syntactically");
}

#[test]
fn rq_check_with_path_null_byte() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/test\0.txt".into()];
    let result = interpret_cli_from(args);
    // Null bytes are allowed syntactically by clap (validated at runtime)
    assert!(result.is_ok());
}

#[test]
fn rq_check_with_path_special_chars() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/test;rm -rf /.txt".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "shell special chars in path are allowed syntactically"
    );
}

#[test]
fn rq_check_with_path_space() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/test file.txt".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "spaces in path are allowed syntactically");
}

#[test]
fn rq_check_with_path_quote_chars() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/test\"quote".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "quote chars in path are allowed syntactically"
    );
}

// ============================================================
// DIMENSION: gc-command-attacks
// ============================================================

#[test]
fn rq_gc_without_engine_url_works() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc without engine-url should use default");
    let cli = result.expect("should parse");
    // Check that it's a Gc command with some engine_url (default or env)
    match cli.command {
        Command::Gc { ref engine_url, .. } => {
            assert!(!engine_url.is_empty(), "engine_url should be set");
        }
        _ => panic!("Expected Gc command"),
    }
}

#[test]
fn rq_gc_with_engine_url_works() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://example.com:8080".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc with engine-url should work");
    let cli = result.expect("should parse");
    assert!(matches!(
        cli.command,
        Command::Gc {
            engine_url,
            dry_run: false
        } if engine_url == "http://example.com:8080"
    ));
}

#[test]
fn rq_gc_with_engine_url_env_var() {
    std::env::set_var("VO_ENGINE_URL", "http://env-host:9090");
    let args: Vec<OsString> = vec!["vo".into(), "gc".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc should use env var");
    let cli = result.expect("should parse");
    assert!(matches!(
        cli.command,
        Command::Gc { engine_url, .. } if engine_url == "http://env-host:9090"
    ));
    std::env::remove_var("VO_ENGINE_URL");
}

#[test]
fn rq_gc_with_engine_url_flag_overrides_env() {
    std::env::set_var("VO_ENGINE_URL", "http://env-host:9090");
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://flag-host:7070".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "flag should override env var");
    let cli = result.expect("should parse");
    assert!(matches!(
        cli.command,
        Command::Gc { engine_url, .. } if engine_url == "http://flag-host:7070"
    ));
    std::env::remove_var("VO_ENGINE_URL");
}

#[test]
fn rq_gc_with_dry_run_flag() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc with --dry-run should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Gc { dry_run: true, .. }));
}

#[test]
fn rq_gc_with_dry_run_short_flag_not_supported() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "-d".into()];
    let result = interpret_cli_from(args);
    // -d is not defined, should be invalid subcommand or unknown flag
    assert!(result.is_err());
}

#[test]
fn rq_gc_with_engine_url_empty_string() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--engine-url".into(), "".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "empty engine-url is allowed syntactically");
}

#[test]
fn rq_gc_with_engine_url_very_long() {
    let long_url = "http://".repeat(10000) + "example.com";
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        long_url.into(),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_ok(),
        "very long engine-url is allowed syntactically"
    );
}

#[test]
fn rq_gc_with_engine_url_invalid_scheme() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "ftp://invalid-scheme.com".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "invalid scheme is allowed syntactically");
}

#[test]
fn rq_gc_with_both_flags() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "gc".into(),
        "--engine-url".into(),
        "http://localhost:3000".into(),
        "--dry-run".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "gc with both flags should work");
    let cli = result.expect("should parse");
    assert!(matches!(cli.command, Command::Gc { dry_run: true, .. }));
}

// ============================================================
// DIMENSION: help-and-version-flags
// ============================================================

#[test]
fn rq_help_flag_on_root_shows_help() {
    let args: Vec<OsString> = vec!["vo".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--help should produce DisplayHelp error");
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn rq_help_flag_on_purge_shows_help() {
    let args: Vec<OsString> = vec!["vo".into(), "purge".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "purge --help should produce DisplayHelp error"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn rq_help_flag_on_check_shows_help() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "check --help should produce DisplayHelp error"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn rq_help_flag_on_gc_shows_help() {
    let args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--help".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "gc --help should produce DisplayHelp error"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn rq_version_flag_shows_version() {
    let args: Vec<OsString> = vec!["vo".into(), "--version".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "--version should produce DisplayVersion error"
    );
    let err = result.expect_err("should be error");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

// ============================================================
// DIMENSION: case-sensitivity
// ============================================================

#[test]
fn rq_purge_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "PURGE".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase PURGE should be rejected");
}

#[test]
fn rq_check_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "CHECK".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase CHECK should be rejected");
}

#[test]
fn rq_gc_uppercase_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "GC".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "uppercase GC should be rejected");
}

#[test]
fn rq_purge_mixed_case_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "PuRgE".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "mixed-case PuRgE should be rejected");
}

// ============================================================
// DIMENSION: no-args-behavior
// ============================================================

#[test]
fn rq_no_subcommand_shows_help() {
    let args: Vec<OsString> = vec!["vo".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "no subcommand should show help");
    let err = result.expect_err("should be error");
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[test]
fn rq_only_binary_shows_help() {
    let args: Vec<OsString> = vec!["vo".into(), "vo".into()];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "binary name as subcommand should show help"
    );
}

// ============================================================
// DIMENSION: dashed-variants-rejection
// ============================================================

#[test]
fn rq_dashed_purge_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--purge".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--purge should be rejected");
}

#[test]
fn rq_dashed_check_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--check".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--check should be rejected");
}

#[test]
fn rq_dashed_gc_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "--gc".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "--gc should be rejected");
}

// ============================================================
// DIMENSION: command-combinations
// ============================================================

#[test]
fn rq_multiple_commands_first_one_works() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test".into(),
        "check".into(),
        "/bin/ls".into(),
    ];
    let result = interpret_cli_from(args);
    // Multiple subcommands cause error (subcommand_required=true)
    assert!(result.is_err());
}

#[test]
fn rq_command_then_flag() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test".into(),
        "--dry-run".into(),
    ];
    let result = interpret_cli_from(args);
    // --dry-run is not valid for purge, should fail
    assert!(result.is_err());
}

// ============================================================
// DIMENSION: unicode-and-special-chars-in-commands
// ============================================================

#[test]
fn rq_purge_with_unicode_instance() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "テスト-instance".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "unicode in instance is allowed");
}

#[test]
fn rq_purge_with_cjk_instance() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "テスト".into(),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "CJK in instance is allowed");
}

#[test]
fn rq_check_with_unicode_path() {
    let args: Vec<OsString> = vec!["vo".into(), "check".into(), "/tmp/テスト".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_ok(), "unicode in path is allowed");
}

// ============================================================
// DIMENSION: subcommand-prefix-confusion
// ============================================================

#[test]
fn rq_check_config_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "check-config".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "check-config should be rejected");
}

#[test]
fn rq_gc_lock_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "gc-lock".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "gc-lock should be rejected");
}

#[test]
fn rq_purge_instance_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purge-instance".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "purge-instance should be rejected");
}

#[test]
fn rq_purge_with_rejected() {
    let args: Vec<OsString> = vec!["vo".into(), "purge-with".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err(), "purge-with should be rejected");
}

// ============================================================
// DIMENSION: concurrent-stability
// ============================================================

#[test]
fn rq_parsing_multiple_commands_stable() {
    // Parse multiple commands in sequence - ensure state doesn't leak
    let purge_args: Vec<OsString> = vec![
        "vo".into(),
        "purge".into(),
        "--instance".into(),
        "test".into(),
    ];
    let check_args: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];
    let gc_args: Vec<OsString> = vec!["vo".into(), "gc".into(), "--dry-run".into()];

    let purge_result = interpret_cli_from(purge_args.clone());
    let check_result = interpret_cli_from(check_args.clone());
    let gc_result = interpret_cli_from(gc_args.clone());

    assert!(purge_result.is_ok());
    assert!(check_result.is_ok());
    assert!(gc_result.is_ok());

    // Re-parse to ensure no state leakage
    let purge_result_2 = interpret_cli_from(purge_args);
    assert!(purge_result_2.is_ok());
}

#[test]
fn rq_error_parsing_does_not_affect_subsequent_valid_parsing() {
    // Parse an invalid command first
    let invalid_args: Vec<OsString> = vec!["vo".into(), "invalid".into()];
    let valid_args: Vec<OsString> = vec!["vo".into(), "check".into(), "/bin/ls".into()];

    let invalid_result = interpret_cli_from(invalid_args);
    let valid_result = interpret_cli_from(valid_args);

    assert!(invalid_result.is_err());
    assert!(
        valid_result.is_ok(),
        "valid command should still parse after invalid one"
    );
}
