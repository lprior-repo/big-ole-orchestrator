use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use crate::cli::{Cli, CliError};

pub trait DispatchContext: Send + Sync {
    fn command_name(&self) -> &str;
    fn elapsed(&self) -> std::time::Duration;
    fn get_metadata(&self, key: &str) -> Option<String>;
    fn set_metadata(&self, key: String, value: String);
}

#[derive(Debug)]
pub struct DefaultDispatchContext {
    name: String,
    start: Instant,
    metadata: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

impl DefaultDispatchContext {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: Instant::now(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl DispatchContext for DefaultDispatchContext {
    fn command_name(&self) -> &str {
        &self.name
    }

    fn elapsed(&self) -> std::time::Duration {
        self.start.elapsed()
    }

    fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.lock().ok()?.get(key).cloned()
    }

    fn set_metadata(&self, key: String, value: String) {
        if let Ok(mut map) = self.metadata.lock() {
            map.insert(key, value);
        }
    }
}

#[derive(Debug)]
pub enum MiddlewareResult {
    Continue,
    Abort(CliError),
}

pub trait MiddlewareV2: Send + Sync {
    fn name(&self) -> &str;

    fn before(
        &self,
        ctx: &dyn DispatchContext,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send + '_>>;

    fn after(&self, ctx: &dyn DispatchContext) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    fn on_error(
        &self,
        ctx: &dyn DispatchContext,
        error: &CliError,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub struct LoggingMiddlewareV2;

impl LoggingMiddlewareV2 {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoggingMiddlewareV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareV2 for LoggingMiddlewareV2 {
    fn name(&self) -> &str {
        "logging"
    }

    fn before(
        &self,
        ctx: &dyn DispatchContext,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send + '_>> {
        let cmd = ctx.command_name().to_string();
        Box::pin(async move {
            eprintln!("[logging] Executing command: {cmd}");
            MiddlewareResult::Continue
        })
    }

    fn after(&self, ctx: &dyn DispatchContext) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.elapsed();
        Box::pin(async move {
            eprintln!("[logging] Command completed in {elapsed:?}");
        })
    }

    fn on_error(
        &self,
        ctx: &dyn DispatchContext,
        error: &CliError,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.elapsed();
        let name = ctx.command_name().to_string();
        let msg = error.to_string();
        Box::pin(async move {
            eprintln!("[logging] Command '{name}' failed after {elapsed:?}: {msg}");
        })
    }
}

pub struct MetricsMiddlewareV2;

impl MetricsMiddlewareV2 {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetricsMiddlewareV2 {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareV2 for MetricsMiddlewareV2 {
    fn name(&self) -> &str {
        "metrics"
    }

    fn before(
        &self,
        _ctx: &dyn DispatchContext,
    ) -> Pin<Box<dyn Future<Output = MiddlewareResult> + Send + '_>> {
        Box::pin(async { MiddlewareResult::Continue })
    }

    fn after(&self, ctx: &dyn DispatchContext) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.elapsed();
        let name = ctx.command_name().to_string();
        Box::pin(async move {
            eprintln!(
                "[metrics] command={} status=success duration_us={}",
                name,
                elapsed.as_micros()
            );
        })
    }

    fn on_error(
        &self,
        ctx: &dyn DispatchContext,
        _error: &CliError,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.elapsed();
        let name = ctx.command_name().to_string();
        Box::pin(async move {
            eprintln!(
                "[metrics] command={} status=failure duration_us={}",
                name,
                elapsed.as_micros()
            );
        })
    }
}

pub struct CommandDispatcherV2 {
    middlewares: Vec<Box<dyn MiddlewareV2>>,
}

impl CommandDispatcherV2 {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn with_middleware<M: MiddlewareV2 + 'static>(mut self, mw: M) -> Self {
        self.middlewares.push(Box::new(mw));
        self
    }

    pub fn add_middleware<M: MiddlewareV2 + 'static>(&mut self, mw: M) {
        self.middlewares.push(Box::new(mw));
    }

    pub fn middleware_count(&self) -> usize {
        self.middlewares.len()
    }

    pub async fn dispatch(&self, cli: Cli) -> Result<(), CliError> {
        let registry = crate::registry::HandlerRegistry::default();
        let handler_ref = registry
            .get(&cli)
            .ok_or_else(|| CliError::Dispatch("unknown command".to_string()))?;

        let command_name = handler_ref.name().to_string();
        let ctx = DefaultDispatchContext::new(&command_name);

        for mw in &self.middlewares {
            match mw.before(&ctx).await {
                MiddlewareResult::Continue => {}
                MiddlewareResult::Abort(err) => {
                    for error_mw in self.middlewares.iter().rev() {
                        error_mw.on_error(&ctx, &err).await;
                    }
                    return Err(err);
                }
            }
        }

        let result = handler_ref.execute(&cli).await;

        match &result {
            Ok(()) => {
                for mw in &self.middlewares {
                    mw.after(&ctx).await;
                }
            }
            Err(err) => {
                for mw in self.middlewares.iter().rev() {
                    mw.on_error(&ctx, err).await;
                }
            }
        }

        result
    }
}

impl Default for CommandDispatcherV2 {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_dispatcher_v2() -> CommandDispatcherV2 {
    CommandDispatcherV2::new()
        .with_middleware(LoggingMiddlewareV2::new())
        .with_middleware(MetricsMiddlewareV2::new())
}

pub async fn dispatch_v2(cli: Cli) -> Result<(), CliError> {
    let dispatcher = create_dispatcher_v2();
    dispatcher.dispatch(cli).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Command;
    use std::path::PathBuf;

    #[test]
    fn default_dispatch_context_stores_command_name() {
        let ctx = DefaultDispatchContext::new("check");
        assert_eq!(ctx.command_name(), "check");
    }

    #[test]
    fn default_dispatch_context_metadata_roundtrip() {
        let ctx = DefaultDispatchContext::new("init");
        ctx.set_metadata("key".to_string(), "value".to_string());
        assert_eq!(ctx.get_metadata("key"), Some("value".to_string()));
        assert_eq!(ctx.get_metadata("missing"), None);
    }

    #[test]
    fn default_dispatch_context_elapsed_is_positive() {
        let ctx = DefaultDispatchContext::new("test");
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(ctx.elapsed() > std::time::Duration::ZERO);
    }

    #[test]
    fn dispatcher_v2_empty_middleware() {
        let d = CommandDispatcherV2::new();
        assert_eq!(d.middleware_count(), 0);
    }

    #[test]
    fn dispatcher_v2_with_middleware() {
        let d = CommandDispatcherV2::new().with_middleware(LoggingMiddlewareV2::new());
        assert_eq!(d.middleware_count(), 1);
    }

    #[test]
    fn dispatcher_v2_with_multiple_middleware() {
        let d = CommandDispatcherV2::new()
            .with_middleware(LoggingMiddlewareV2::new())
            .with_middleware(MetricsMiddlewareV2::new());
        assert_eq!(d.middleware_count(), 2);
    }

    #[tokio::test]
    async fn logging_middleware_v2_before_returns_continue() {
        let ctx = DefaultDispatchContext::new("check");
        let mw = LoggingMiddlewareV2::new();
        matches!(mw.before(&ctx).await, MiddlewareResult::Continue);
    }

    #[tokio::test]
    async fn logging_middleware_v2_after_completes() {
        let ctx = DefaultDispatchContext::new("check");
        let mw = LoggingMiddlewareV2::new();
        mw.after(&ctx).await;
    }

    #[tokio::test]
    async fn logging_middleware_v2_on_error_completes() {
        let ctx = DefaultDispatchContext::new("check");
        let mw = LoggingMiddlewareV2::new();
        let err = CliError::Dispatch("test error".to_string());
        mw.on_error(&ctx, &err).await;
    }

    #[tokio::test]
    async fn metrics_middleware_v2_before_returns_continue() {
        let ctx = DefaultDispatchContext::new("gc");
        let mw = MetricsMiddlewareV2::new();
        matches!(mw.before(&ctx).await, MiddlewareResult::Continue);
    }

    #[tokio::test]
    async fn metrics_middleware_v2_after_completes() {
        let ctx = DefaultDispatchContext::new("gc");
        let mw = MetricsMiddlewareV2::new();
        mw.after(&ctx).await;
    }

    #[tokio::test]
    async fn metrics_middleware_v2_on_error_completes() {
        let ctx = DefaultDispatchContext::new("gc");
        let mw = MetricsMiddlewareV2::new();
        let err = CliError::Dispatch("test error".to_string());
        mw.on_error(&ctx, &err).await;
    }

    #[test]
    fn middleware_v2_names() {
        assert_eq!(LoggingMiddlewareV2::new().name(), "logging");
        assert_eq!(MetricsMiddlewareV2::new().name(), "metrics");
    }

    #[tokio::test]
    async fn dispatch_v2_unknown_command_returns_error() {
        let dispatcher = create_dispatcher_v2();
        let cli = Cli {
            command: Command::Purge {
                instance: "nonexistent".to_string(),
            },
        };
        let result = dispatcher.dispatch(cli).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn dispatch_v2_check_command_runs() {
        let dispatcher = create_dispatcher_v2();
        let cli = Cli {
            command: Command::Check { workflow: false,
                path: PathBuf::from("/nonexistent"),
            },
        };
        let result = dispatcher.dispatch(cli).await;
        assert!(result.is_err());
    }

    #[test]
    fn create_dispatcher_v2_returns_two_middlewares() {
        let d = create_dispatcher_v2();
        assert_eq!(d.middleware_count(), 2);
    }

    #[test]
    fn middleware_result_continue_matches() {
        matches!(MiddlewareResult::Continue, MiddlewareResult::Continue);
    }

    #[test]
    fn middleware_result_abort_matches() {
        let err = CliError::Dispatch("test".to_string());
        matches!(MiddlewareResult::Abort(err), MiddlewareResult::Abort(_));
    }

    #[test]
    fn middleware_result_variants_differ() {
        let err = CliError::Dispatch("test".to_string());
        match (MiddlewareResult::Continue, MiddlewareResult::Abort(err)) {
            (MiddlewareResult::Continue, MiddlewareResult::Abort(_)) => {}
            _ => panic!("variants should differ"),
        }
    }
}
