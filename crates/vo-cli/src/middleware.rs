use std::time::Instant;

use crate::cli::{Cli, CliError, Command};

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub command: Command,
    pub start_time: Instant,
}

impl CommandContext {
    pub fn new(command: Command) -> Self {
        Self {
            command,
            start_time: Instant::now(),
        }
    }
}

pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;
    fn before(&self, ctx: &CommandContext) -> Result<(), CliError>;
    fn after(&self, ctx: &CommandContext, result: &Result<(), CliError>);
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

    fn before(&self, ctx: &CommandContext) -> Result<(), CliError> {
        eprintln!("[{}] Executing command: {:?}", self.name(), ctx.command);
        Ok(())
    }

    fn after(&self, ctx: &CommandContext, result: &Result<(), CliError>) {
        let elapsed = ctx.start_time.elapsed();
        match result {
            Ok(()) => eprintln!(
                "[{}] Command completed successfully in {:?}",
                self.name(),
                elapsed
            ),
            Err(e) => eprintln!(
                "[{}] Command failed after {:?}: {}",
                self.name(),
                elapsed,
                e
            ),
        }
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

    fn before(&self, _ctx: &CommandContext) -> Result<(), CliError> {
        Ok(())
    }

    fn after(&self, ctx: &CommandContext, result: &Result<(), CliError>) {
        let elapsed = ctx.start_time.elapsed();
        let status = if result.is_ok() { "success" } else { "failure" };
        eprintln!(
            "[metrics] command={:?} status={} duration_us={}",
            ctx.command,
            status,
            elapsed.as_micros()
        );
    }
}

pub struct CommandDispatcher {
    middlewares: Vec<Box<dyn Middleware>>,
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
        self.middlewares.push(Box::new(middleware));
        self
    }

    pub fn add_middleware<M: Middleware + 'static>(&mut self, middleware: M) {
        self.middlewares.push(Box::new(middleware));
    }

    pub async fn dispatch(&self, cli: Cli) -> Result<(), CliError> {
        let ctx = CommandContext::new(cli.command.clone());

        for middleware in &self.middlewares {
            middleware.before(&ctx)?;
        }

        let result = dispatch_inner(cli).await;

        for middleware in &self.middlewares {
            middleware.after(&ctx, &result);
        }

        result
    }
}

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
    }
}

pub fn create_dispatcher() -> CommandDispatcher {
    CommandDispatcher::new()
        .with_middleware(LoggingMiddleware::new())
        .with_middleware(MetricsMiddleware::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_context_creation() {
        let cmd = Command::Check {
            path: std::path::PathBuf::from("/tmp"),
        };
        let ctx = CommandContext::new(cmd.clone());
        assert_eq!(ctx.command, cmd);
    }

    #[test]
    fn test_dispatcher_empty() {
        let dispatcher = CommandDispatcher::new();
        assert_eq!(dispatcher.middlewares.len(), 0);
    }

    #[test]
    fn test_dispatcher_with_middleware() {
        let dispatcher = CommandDispatcher::new().with_middleware(LoggingMiddleware::new());
        assert_eq!(dispatcher.middlewares.len(), 1);
    }

    #[test]
    fn test_dispatcher_with_multiple_middleware() {
        let dispatcher = CommandDispatcher::new()
            .with_middleware(LoggingMiddleware::new())
            .with_middleware(MetricsMiddleware::new());
        assert_eq!(dispatcher.middlewares.len(), 2);
    }
}
