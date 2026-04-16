use vo_cli::{interpret_cli_from, map_error_to_exit_code, parse_strict_numeric, CliError, Command};

#[test]
fn interpret_cli_from_returns_display_help_error_when_help_flag_provided() {
    let result = interpret_cli_from(vec!["vo", "--help"]);
    assert_eq!(
        result.unwrap_err().kind(),
        clap::error::ErrorKind::DisplayHelp
    );
}

#[test]
fn map_error_to_exit_code_returns_0_for_clap_displayhelp() {
    let mut cmd = clap::Command::new("vo");
    let err = cmd.error(clap::error::ErrorKind::DisplayHelp, "help");
    assert_eq!(map_error_to_exit_code(&CliError::Clap(err)), 0);
}

#[test]
fn parse_strict_numeric_returns_ok_for_0() {
    assert_eq!(parse_strict_numeric("0").unwrap(), 0);
}

#[test]
fn parse_strict_numeric_returns_err_for_plus1() {
    assert!(matches!(
        parse_strict_numeric("+1"),
        Err(CliError::InvalidNumeric(_))
    ));
}

#[test]
fn map_error_to_exit_code_returns_1_for_check_error() {
    let err = CliError::Check(vo_cli::CheckError::FileNotFound {
        path: std::path::PathBuf::from("/tmp/test"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn map_error_to_exit_code_returns_1_for_gc_error() {
    let err = CliError::Gc(vo_cli::GcError::VersionsDirNotFound {
        path: std::path::PathBuf::from("/var/wtf/versions"),
    });
    assert_eq!(map_error_to_exit_code(&err), 1);
}

#[test]
fn interpret_cli_from_parses_check_subcommand() {
    let cli = interpret_cli_from(vec!["vo", "check", "/usr/bin/ls"]).expect("parse");
    assert_eq!(
        cli.command,
        Command::Check { workflow: false, path: std::path::PathBuf::from("/usr/bin/ls") }
    );
}

#[test]
fn interpret_cli_from_parses_gc_subcommand_defaults() {
    let cli = interpret_cli_from(vec!["vo", "gc"]).expect("parse");
    assert_eq!(
        cli.command,
        Command::Gc {
            engine_url: "http://localhost:3000".to_string(),
            dry_run: false,
        }
    );
}

#[test]
fn interpret_cli_from_parses_gc_dry_run() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    assert_eq!(
        cli.command,
        Command::Gc {
            engine_url: "http://localhost:3000".to_string(),
            dry_run: true,
        }
    );
}

#[test]
fn interpret_cli_from_parses_gc_engine_url() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--engine-url", "http://example.com:9999"])
        .expect("parse");
    assert_eq!(
        cli.command,
        Command::Gc {
            engine_url: "http://example.com:9999".to_string(),
            dry_run: false,
        }
    );
}
