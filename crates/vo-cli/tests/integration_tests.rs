use vo_cli::{dispatch, interpret_cli_from, map_error_to_exit_code};

#[test]
fn integration_full_pipeline_success() {
    let args = vec!["vo", "start"];
    let cli = interpret_cli_from(args).expect("Failed to parse valid args");

    let result = dispatch(cli);
    assert!(result.is_ok());

    if let Err(e) = result {
        let code = map_error_to_exit_code(&e);
        assert_eq!(code, 0);
    }
}
