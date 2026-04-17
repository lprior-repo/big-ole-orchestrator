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

<<<<<<< HEAD
=======
impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

async fn dispatch_inner(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Purge { instance } => {
            let fjall_path = "/home/lewis/.gemini/tmp/veloxide/fjall";
            let keyspace = fjall::Config::new(fjall_path)
                .open()
                .map_err(|e| CliError::Dispatch(format!("Failed to open keyspace: {e}")))?;

            match vo_storage::purge::purge_instance(&keyspace, &instance) {
                Ok(count) => {
                    println!("Purged {count} events for instance {instance}.");
                    Ok(())
                }
                Err(vo_storage::codec::StorageError::InstanceRunning) => {
                    eprintln!("Cannot purge a running instance.");
                    Err(CliError::Dispatch("Instance is running".to_string()))
                }
                Err(e) => Err(CliError::Dispatch(format!("Purge failed: {e}"))),
            }
        }
        Command::Check { path } => {
            crate::commands::check::run_check(&path)?;
            Ok(())
        }
        Command::Gc {
            engine_url,
            dry_run,
        } => {
            let config = crate::commands::gc::GcConfig {
                engine_url,
                versions_dir: std::path::PathBuf::from("/var/wtf/versions"),
                dry_run,
            };
            crate::commands::gc::run_gc(&config).await?;
            Ok(())
        }
        Command::Init {
            project_dir,
            engine_url,
            storage_path,
        } => {
            let config = crate::commands::init::InitConfig {
                project_dir,
                engine_url,
                storage_path,
            };
            let vo_dir = crate::commands::init::run_init(&config)?;
            println!("Initialized veloxide project at {}", vo_dir.display());
            Ok(())
        }
        Command::Lock { project_dir } => {
            let config = crate::commands::lock::LockConfig { project_dir };
            let lockmap = crate::commands::lock::run_lock(&config)?;
            println!("Locked {} workflow(s):", lockmap.len());
            for (name, hash) in &lockmap {
                println!("  {name} {hash}");
            }
            Ok(())
        }
        Command::Doctor { project_dir } => {
            let config = crate::commands::doctor::DoctorConfig { project_dir };
            let report = crate::commands::doctor::run_doctor(&config)?;
            let (stdout, stderr) = crate::commands::doctor::format_report(&report);
            print!("{stdout}");
            eprint!("{stderr}");
            Ok(())
        }
        Command::Unquarantine {
            workflow_name,
            operator,
            engine_url,
        } => {
            let result = crate::commands::unquarantine::unquarantine_workflow(
                &engine_url,
                &workflow_name,
                &operator,
            )
            .await?;
            crate::commands::unquarantine::display_result(&result);
            Ok(())
        }
        Command::Rebuild {
            projection_id,
            storage_path,
            from_sequence,
            to_sequence,
            cancel_file,
            dry_run,
        } => {
            let config = crate::commands::rebuild::RebuildConfig {
                storage_path,
                projection_id,
                from_sequence,
                to_sequence,
                cancel_file,
                dry_run,
            };
            let progress = crate::commands::rebuild::run_rebuild(&config)?;
            println!(
                "Rebuild complete: {}% ({}/{} events) in {}ms",
                progress.progress_percent, progress.events_processed, progress.events_total, progress.elapsed_ms()
            );
            Ok(())
        }
    }
}

>>>>>>> origin/vo-worker-tests
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
        let dispatcher = CommandDispatcher::new(HandlerRegistry::default())
            .with_middleware(LoggingMiddleware::new());
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
