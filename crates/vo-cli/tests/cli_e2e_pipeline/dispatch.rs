use std::path::PathBuf;

use vo_cli::{
    CliError, Command, CommandDispatcherV2, DefaultDispatchContext, DispatchContext,
    MiddlewareResult, MiddlewareV2,
};

struct AbortMiddleware;

impl MiddlewareV2 for AbortMiddleware {
    fn name(&self) -> &'static str {
        "abort"
    }

    fn before(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = MiddlewareResult> + Send + '_>> {
        Box::pin(async {
            MiddlewareResult::Abort(CliError::Dispatch("aborted by middleware".into()))
        })
    }

    fn after(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }

    fn on_error(
        &self,
        _ctx: &dyn DispatchContext,
        _error: &CliError,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

#[tokio::test]
async fn dispatch_v2_abort_middleware_returns_error() {
    let dispatcher = CommandDispatcherV2::new().with_middleware(AbortMiddleware);
    let cli = vo_cli::Cli {
        command: Command::Check {
            workflow: false,
            path: PathBuf::from("/tmp"),
        },
    };
    let result = dispatcher.dispatch(cli).await;
    assert!(result.is_err());
    match result {
        Err(CliError::Dispatch(msg)) => assert!(msg.contains("aborted by middleware")),
        _ => panic!("expected Dispatch error"),
    }
}

#[tokio::test]
async fn logging_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = vo_cli::LoggingMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}

#[tokio::test]
async fn metrics_middleware_v2_on_error_captures_context() {
    let ctx = DefaultDispatchContext::new("failing-cmd");
    let mw = vo_cli::MetricsMiddlewareV2::new();
    let err = CliError::Dispatch("test failure".into());
    mw.on_error(&ctx, &err).await;
}
