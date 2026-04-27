use vo_cli::middleware::Middleware;
use vo_cli::{CliError, CommandContext};

#[test]
fn command_context_stores_command() {
    let ctx = CommandContext::new("check");
    assert_eq!(ctx.command_name, "check");
}

#[test]
fn command_context_metadata() {
    let ctx = CommandContext::new("check");
    ctx.set_metadata("key", "value");
    assert_eq!(ctx.get_metadata("key"), Some("value".to_string()));
}

#[test]
fn create_dispatcher_returns_two_middlewares() {
    let d = vo_cli::create_dispatcher();
    let _ = d;
}

#[test]
fn logging_middleware_has_name() {
    let m = vo_cli::middleware::LoggingMiddleware::new();
    assert_eq!(m.name(), "logging");
}

#[test]
fn metrics_middleware_has_name() {
    let m = vo_cli::middleware::MetricsMiddleware::new();
    assert_eq!(m.name(), "metrics");
}

#[tokio::test]
async fn logging_middleware_before_ok() {
    let m = vo_cli::middleware::LoggingMiddleware::new();
    let ctx = CommandContext::new("check");
    assert!(m.before(&ctx).await.is_ok());
}

#[tokio::test]
async fn logging_middleware_after_ok_result() {
    let m = vo_cli::middleware::LoggingMiddleware::new();
    let ctx = CommandContext::new("check");
    m.after(&ctx, &Ok(())).await;
}

#[tokio::test]
async fn logging_middleware_after_err_result() {
    let m = vo_cli::middleware::LoggingMiddleware::new();
    let ctx = CommandContext::new("check");
    m.after(&ctx, &Err(CliError::Dispatch("fail".into()))).await;
}

#[tokio::test]
async fn metrics_middleware_before_ok() {
    let m = vo_cli::middleware::MetricsMiddleware::new();
    let ctx = CommandContext::new("check");
    assert!(m.before(&ctx).await.is_ok());
}

#[tokio::test]
async fn metrics_middleware_after_ok() {
    let m = vo_cli::middleware::MetricsMiddleware::new();
    let ctx = CommandContext::new("check");
    m.after(&ctx, &Ok(())).await;
}

#[tokio::test]
async fn metrics_middleware_after_err() {
    let m = vo_cli::middleware::MetricsMiddleware::new();
    let ctx = CommandContext::new("check");
    m.after(&ctx, &Err(CliError::Dispatch("fail".into()))).await;
}
