use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use crate::cli::{Cli, CliError};
use crate::registry::HandlerRegistry;

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub command_name: String,
    pub start_time: Instant,
    pub metadata: Arc<std::sync::Mutex<HashMap<String, String>>>,
}

impl CommandContext {
    pub fn new(command_name: impl Into<String>) -> Self {
        Self {
            command_name: command_name.into(),
            start_time: Instant::now(),
            metadata: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn set_metadata(&self, key: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut map) = self.metadata.lock() {
            map.insert(key.into(), value.into());
        }
    }

    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.lock().ok()?.get(key).cloned()
    }
}

pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;

    fn before(
        &self,
        ctx: &CommandContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>>;

    fn after(
        &self,
        ctx: &CommandContext,
        result: &Result<(), CliError>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub struct LoggingMiddleware;

impl LoggingMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for LoggingMiddleware {
    fn name(&self) -> &str {
        "logging"
    }

    fn before(
        &self,
        ctx: &CommandContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
        let name = self.name().to_string();
        let cmd = ctx.command_name.clone();
        Box::pin(async move {
            eprintln!("[{name}] Executing command: {cmd}");
            Ok(())
        })
    }

    fn after(
        &self,
        ctx: &CommandContext,
        result: &Result<(), CliError>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.start_time.elapsed();
        let msg = match result {
            Ok(()) => format!(
                "[{}] Command completed successfully in {:?}",
                self.name(),
                elapsed
            ),
            Err(e) => format!(
                "[{}] Command failed after {:?}: {}",
                self.name(),
                elapsed,
                e
            ),
        };
        Box::pin(async move {
            eprintln!("{msg}");
        })
    }
}

pub struct MetricsMiddleware;

impl MetricsMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetricsMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for MetricsMiddleware {
    fn name(&self) -> &str {
        "metrics"
    }

    fn before(
        &self,
        _ctx: &CommandContext,
    ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn after(
        &self,
        ctx: &CommandContext,
        result: &Result<(), CliError>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let elapsed = ctx.start_time.elapsed();
        let status = if result.is_ok() { "success" } else { "failure" };
        let name = ctx.command_name.clone();
        Box::pin(async move {
            eprintln!(
                "[metrics] command={} status={} duration_us={}",
                name,
                status,
                elapsed.as_micros()
            );
        })
    }
}

pub struct CommandDispatcher {
    middlewares: Vec<Box<dyn Middleware>>,
    registry: HandlerRegistry,
}

impl CommandDispatcher {
    pub fn new(registry: HandlerRegistry) -> Self {
        Self {
            middlewares: Vec::new(),
            registry,
        }
    }

    pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    pub fn add_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Box::new(middleware));
    }

    pub fn middleware_count(&self) -> usize {
        self.middlewares.len()
    }

    pub async fn dispatch(&self, cli: Cli) -> Result<(), CliError> {
        let handler = self
            .registry
            .get(&cli)
            .ok_or_else(|| CliError::Dispatch("unknown command".to_string()))?;

        let ctx = CommandContext::new(handler.name());

        for middleware in &self.middlewares {
            middleware.before(&ctx).await?;
        }

        let result = handler.execute(&cli).await;

        for middleware in &self.middlewares {
            middleware.after(&ctx, &result).await;
        }

        result
    }
}

pub fn create_dispatcher() -> CommandDispatcher {
    CommandDispatcher::new(HandlerRegistry::default())
        .with_middleware(LoggingMiddleware::new())
        .with_middleware(MetricsMiddleware::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_context_creation() {
        let ctx = CommandContext::new("test-cmd");
        assert_eq!(ctx.command_name, "test-cmd");
    }

    #[test]
    fn test_command_context_metadata() {
        let ctx = CommandContext::new("test-cmd");
        ctx.set_metadata("key", "value");
        assert_eq!(ctx.get_metadata("key"), Some("value".to_string()));
        assert_eq!(ctx.get_metadata("missing"), None);
    }

    #[test]
    fn test_dispatcher_empty_middleware() {
        let dispatcher = CommandDispatcher::new(HandlerRegistry::default());
        assert_eq!(dispatcher.middlewares.len(), 0);
    }

    #[test]
    fn test_dispatcher_with_middleware() {
        let dispatcher =
            CommandDispatcher::new(HandlerRegistry::default()).with_middleware(LoggingMiddleware::new());
        assert_eq!(dispatcher.middlewares.len(), 1);
    }

    #[test]
    fn test_dispatcher_with_multiple_middleware() {
        let dispatcher = CommandDispatcher::new(HandlerRegistry::default())
            .with_middleware(LoggingMiddleware::new())
            .with_middleware(MetricsMiddleware::new());
        assert_eq!(dispatcher.middlewares.len(), 2);
    }
}
