use vo_cli::{
    dispatch, interpret_cli_from, map_error_to_exit_code, parse_nats_url, parse_strict_numeric,
    Cli, CliError, NatsUrl,
};

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
fn dispatch_returns_ok_when_cli_contains_valid_command() {
    let cli = Cli {
        command: "start".into(),
    };
    assert!(dispatch(cli).is_ok());
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
fn parse_nats_url_returns_ok_for_localhost_1() {
    let expected = NatsUrl {
        host: "localhost".into(),
        port: Some(1),
    };
    let actual = parse_nats_url("localhost:1").unwrap();
    assert_eq!(actual, expected);
}
