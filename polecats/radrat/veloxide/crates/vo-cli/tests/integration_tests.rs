#![allow(clippy::redundant_pattern_matching)]
use vo_cli::{dispatch, interpret_cli_from, map_error_to_exit_code, Command};

#[tokio::test]
async fn integration_gc_dispatch_succeeds() {
    let args = vec!["vo", "gc", "--dry-run"];
    let cli = interpret_cli_from(args).expect("Failed to parse valid args");

    let result = dispatch(cli).await;
    assert!(matches!(result, Ok(())));

    if let Err(e) = result {
        let code = map_error_to_exit_code(&e);
        assert_eq!(code, 0);
    }
}

#[tokio::test]
async fn integration_check_dispatch_routes_to_check_handler() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.bin");
    std::fs::write(&path, [0x7Fu8, 0x45, 0x4C, 0x46]).expect("write");

    let cli = interpret_cli_from(vec!["vo", "check", path.to_str().expect("utf8")]).expect("parse");
    assert!(matches!(cli.command, Command::Check { .. }));

    let result = dispatch(cli).await;
    assert!(matches!(result, Ok(())));
}

#[tokio::test]
async fn integration_check_nonexistent_file_returns_error() {
    let cli = interpret_cli_from(vec!["vo", "check", "/tmp/nonexistent-vel-co5-test-bin"])
        .expect("parse");

    let result = dispatch(cli).await;
    assert!(matches!(result, Err(_)));
    let code = map_error_to_exit_code(result.as_ref().expect_err("error"));
    assert_eq!(code, 1);
}

#[tokio::test]
async fn integration_gc_dispatch_routes_to_gc_handler() {
    let cli = interpret_cli_from(vec!["vo", "gc", "--dry-run"]).expect("parse");
    assert!(matches!(cli.command, Command::Gc { dry_run: true, .. }));

    let result = dispatch(cli).await;
    let _val = result;
}

#[tokio::test]
async fn integration_gc_with_custom_engine_url() {
    let cli = interpret_cli_from(vec![
        "vo",
        "gc",
        "--engine-url",
        "http://localhost:19999",
        "--dry-run",
    ])
    .expect("parse");

    match &cli.command {
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            assert_eq!(engine_url, "http://localhost:19999");
            assert!(*dry_run);
        }
        _ => panic!("expected Gc command"),
    }

    let result = dispatch(cli).await;
    let _val = result;
}

#[test]
fn integration_check_cli_rejects_missing_path() {
    let result = interpret_cli_from(vec!["vo", "check"]);
    assert!(matches!(result, Err(_)));
    let err = result.expect_err("error");
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}
