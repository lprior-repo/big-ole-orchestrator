use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use vo_cli::{interpret_cli_from, map_error_to_exit_code, CliError};

fn interpret_done(args: Vec<&str>) -> Result<(), CliError> {
    let args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
    let cli = interpret_cli_from(args).map_err(CliError::Clap)?;
    match cli.command {
        vo_cli::Command::Purge { .. } => Ok(()),
        _ => Ok(()),
    }
}

fn run_done(args: Vec<&str>) -> i32 {
    map_error_to_exit_code(&interpret_done(args).unwrap_err())
}

// ============================================================
// RQ-CLI-V7-001: Unicode subcommand injection
// Attack: Send "done" with zero-width unicode characters
// ============================================================

#[test]
fn rq_done_rejects_zero_width_unicode_injection() {
    let result = interpret_cli_from(vec!["vo", "\u{200B}done"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn rq_done_rejects_mixed_unicode_zalgo() {
    let result = interpret_cli_from(vec!["vo", "d\u{0301}\u{0332}o\u{0333}n\u{0334}e"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn rq_done_rejects_bidi_override_injection() {
    let result = interpret_cli_from(vec!["vo", "\u{202E}done"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

// ============================================================
// RQ-CLI-V7-002: Flag injection attacks
// Attack: done as a flag, not a subcommand
// ============================================================

#[test]
fn rq_done_rejects_done_as_long_flag() {
    let result = interpret_cli_from(vec!["vo", "--done"]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn rq_done_rejects_done_with_dash_prefix() {
    let result = interpret_cli_from(vec!["vo", "-done"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_double_dash_done() {
    let result = interpret_cli_from(vec!["vo", "--", "done"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-003: Shell metacharacter injection
// Attack: done with shell special characters
// ============================================================

#[test]
fn rq_done_rejects_done_with_pipe() {
    let result = interpret_cli_from(vec!["vo", "done;ls"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_backtick() {
    let result = interpret_cli_from(vec!["vo", "done`ls`"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_and() {
    let result = interpret_cli_from(vec!["vo", "done&&ls"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_or() {
    let result = interpret_cli_from(vec!["vo", "done||ls"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_newline() {
    let result = interpret_cli_from(vec!["vo", "done\n"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_null_byte() {
    let args: Vec<OsString> = vec!["vo".into(), OsString::from("done\0"), "ls".into()];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-004: Case sensitivity attacks
// Attack: variations in case
// ============================================================

#[test]
fn rq_done_rejects_DONE_uppercase() {
    let result = interpret_cli_from(vec!["vo", "DONE"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn rq_done_rejects_Done_mixed_case() {
    let result = interpret_cli_from(vec!["vo", "Done"]);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::InvalidSubcommand
    );
}

#[test]
fn rq_done_rejects_don3_with_numbers() {
    let result = interpret_cli_from(vec!["vo", "don3"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_don_with_special_chars() {
    let result = interpret_cli_from(vec!["vo", "don\x1b"]); // escape
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-005: Length attacks
// Attack: extremely long "done" variants
// ============================================================

#[test]
fn rq_done_rejects_1mb_done() {
    let long_done = format!("done{}", "x".repeat(1024 * 1024));
    let result = interpret_cli_from(vec!["vo", &long_done]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_10kb_done() {
    let long_done = format!("done{}", "x".repeat(10 * 1024));
    let result = interpret_cli_from(vec!["vo", &long_done]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_empty_string_subcommand() {
    let result = interpret_cli_from(vec!["vo", ""]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-006: Homograph attacks
// Attack: visually similar characters
// ============================================================

#[test]
fn rq_done_rejects_cyrillic_done() {
    let result = interpret_cli_from(vec!["vo", "dоne"]); // Cyrillic 'о' (U+043E)
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_greek_done() {
    let result = interpret_cli_from(vec!["vo", "δone"]); // Greek delta
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_fullwidth_done() {
    let result = interpret_cli_from(vec!["vo", "ｄｏｎｅ"]); // Fullwidth
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-007: Subcommand chaining
// Attack: multiple subcommands
// ============================================================

#[test]
fn rq_done_rejects_done_check() {
    let result = interpret_cli_from(vec!["vo", "done", "check"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_gc() {
    let result = interpret_cli_from(vec!["vo", "done", "gc"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_multiple_subcommands() {
    let result = interpret_cli_from(vec!["vo", "done", "check", "gc"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-008: Context confusion attacks
// Attack: done looks like an argument
// ============================================================

#[test]
fn rq_done_accepts_done_as_path_argument_to_check() {
    let result = interpret_cli_from(vec!["vo", "check", "done"]);
    assert!(result.is_ok());
}

#[test]
fn rq_done_rejects_done_as_argument_to_gc() {
    let result = interpret_cli_from(vec!["vo", "gc", "done"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-009: Exit code verification
// Attack: verify error handling returns correct exit codes
// ============================================================

#[test]
fn rq_done_exit_code_is_2_for_unknown_subcommand() {
    let exit_code = run_done(vec!["vo", "done"]);
    assert_eq!(exit_code, 2);
}

#[test]
fn rq_done_exit_code_is_2_for_done_flag() {
    let exit_code = run_done(vec!["vo", "--done"]);
    assert_eq!(exit_code, 2);
}

// ============================================================
// RQ-CLI-V7-010: UTF-8 mutation attacks
// Attack: malformed UTF-8
// ============================================================

#[test]
fn rq_done_rejects_truncated_utf8() {
    let args: Vec<OsString> = vec![
        "vo".into(),
        OsString::from_vec(vec![0x64, 0x6F, 0x6E, 0x65, 0xCC]),
    ];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_overlong_utf8_encoding() {
    let overlong = vec![0x64, 0x6F, 0x6E, 0x65, 0xF0, 0x80, 0x80, 0x80];
    let args: Vec<OsString> = vec!["vo".into(), OsString::from_vec(overlong)];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_invalid_utf8_sequences() {
    let invalid = vec![0x64, 0x6F, 0x6E, 0x65, 0x80, 0x80];
    let args: Vec<OsString> = vec!["vo".into(), OsString::from_vec(invalid)];
    let result = interpret_cli_from(args);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-011: Numeric escape sequences
// Attack: \0, \x00 in subcommand name
// ============================================================

#[test]
fn rq_done_rejects_done_with_null_escape() {
    let result = interpret_cli_from(vec!["vo", "done\\0"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_hex_escape() {
    let result = interpret_cli_from(vec!["vo", "done\\x00"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-012: Confusion with built-in commands
// Attack: done is not a built-in even though --help is
// ============================================================

#[test]
fn rq_done_rejects_help_done() {
    let result = interpret_cli_from(vec!["vo", "help", "done"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_version_done() {
    let result = interpret_cli_from(vec!["vo", "version", "done"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-013: Argument order attacks
// Attack: done appears in different positions
// ============================================================

#[test]
fn rq_done_rejects_done_after_subcommand() {
    let result = interpret_cli_from(vec!["vo", "check", "/tmp", "done"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_before_subcommand() {
    let result = interpret_cli_from(vec!["vo", "done", "check", "/tmp"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_between_flags() {
    let result = interpret_cli_from(vec!["vo", "--dry-run", "done", "--engine-url"]);
    assert!(result.is_err());
}

// ============================================================
// RQ-CLI-V7-014: Path traversal in done
// Attack: done/../../../etc/passwd style attacks
// ============================================================

#[test]
fn rq_done_rejects_done_with_path_traversal() {
    let result = interpret_cli_from(vec!["vo", "done/../check"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_absolute_path() {
    let result = interpret_cli_from(vec!["vo", "/done"]);
    assert!(result.is_err());
}

#[test]
fn rq_done_rejects_done_with_relative_path() {
    let result = interpret_cli_from(vec!["vo", "./done"]);
    assert!(result.is_err());
}
