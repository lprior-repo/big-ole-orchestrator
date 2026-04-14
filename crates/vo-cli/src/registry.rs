use std::collections::HashMap;

use crate::cli::{Cli, Command};
use crate::handler::CommandHandler;

pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register(Box::new(handlers::PurgeHandler));
        registry.register(Box::new(handlers::CheckHandler));
        registry.register(Box::new(handlers::GcHandler));
        registry.register(Box::new(handlers::InitHandler));
        registry.register(Box::new(handlers::LockHandler));
        registry.register(Box::new(handlers::DoctorHandler));
        registry.register(Box::new(handlers::RebuildHandler));
        registry.register(Box::new(handlers::WorkspaceHandler));
        registry
    }
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, handler: Box<dyn CommandHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub fn get(&self, cli: &Cli) -> Option<&dyn CommandHandler> {
        let key = command_key(&cli.command)?;
        self.handlers.get(key).map(|h| h.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }
}

fn command_key(command: &Command) -> Option<&'static str> {
    match command {
        Command::Purge { .. } => Some("purge"),
        Command::Check { .. } => Some("check"),
        Command::Gc { .. } => Some("gc"),
        Command::Init { .. } => Some("init"),
        Command::Lock { .. } => Some("lock"),
        Command::Doctor { .. } => Some("doctor"),
        Command::Rebuild { .. } => Some("rebuild"),
        Command::Workspace { .. } => Some("workspace"),
    }
}

mod handlers {
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;

    use crate::cli::{Cli, CliError, Command};
    use crate::handler::CommandHandler;

    pub struct PurgeHandler;

    impl CommandHandler for PurgeHandler {
        fn name(&self) -> &'static str {
            "purge"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Purge { ref instance } = cli.command else {
                return Box::pin(async { Err(CliError::Dispatch("not a purge command".to_string())) });
            };
            let instance = instance.clone();
            Box::pin(async move {
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
            })
        }
    }

    pub struct CheckHandler;

    impl CommandHandler for CheckHandler {
        fn name(&self) -> &'static str {
            "check"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Check { ref path } = cli.command else {
                return Box::pin(async { Err(CliError::Dispatch("not a check command".to_string())) });
            };
            let path = path.clone();
            Box::pin(async move {
                crate::commands::check::run_check(&path)?;
                Ok(())
            })
        }
    }

    pub struct GcHandler;

    impl CommandHandler for GcHandler {
        fn name(&self) -> &'static str {
            "gc"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Gc {
                ref engine_url,
                dry_run,
            } = cli.command
            else {
                return Box::pin(async { Err(CliError::Dispatch("not a gc command".to_string())) });
            };
            let engine_url = engine_url.clone();
            Box::pin(async move {
                let config = crate::commands::gc::GcConfig {
                    engine_url,
                    versions_dir: PathBuf::from("/var/wtf/versions"),
                    dry_run,
                };
                crate::commands::gc::run_gc(&config).await?;
                Ok(())
            })
        }
    }

    pub struct InitHandler;

    impl CommandHandler for InitHandler {
        fn name(&self) -> &'static str {
            "init"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Init {
                ref project_dir,
                ref engine_url,
                ref storage_path,
            } = cli.command
            else {
                return Box::pin(async { Err(CliError::Dispatch("not an init command".to_string())) });
            };
            let project_dir = project_dir.clone();
            let engine_url = engine_url.clone();
            let storage_path = storage_path.clone();
            Box::pin(async move {
                let config = crate::commands::init::InitConfig {
                    project_dir,
                    engine_url,
                    storage_path,
                };
                let vo_dir = crate::commands::init::run_init(&config)?;
                println!("Initialized veloxide project at {}", vo_dir.display());
                Ok(())
            })
        }
    }

    pub struct LockHandler;

    impl CommandHandler for LockHandler {
        fn name(&self) -> &'static str {
            "lock"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Lock { ref project_dir } = cli.command else {
                return Box::pin(async { Err(CliError::Dispatch("not a lock command".to_string())) });
            };
            let project_dir = project_dir.clone();
            Box::pin(async move {
                let config = crate::commands::lock::LockConfig { project_dir };
                let lockmap = crate::commands::lock::run_lock(&config)?;
                println!("Locked {} workflow(s):", lockmap.len());
                for (name, hash) in &lockmap {
                    println!("  {name} {hash}");
                }
                Ok(())
            })
        }
    }

    pub struct DoctorHandler;

    impl CommandHandler for DoctorHandler {
        fn name(&self) -> &'static str {
            "doctor"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Doctor { ref project_dir } = cli.command else {
                return Box::pin(async { Err(CliError::Dispatch("not a doctor command".to_string())) });
            };
            let project_dir = project_dir.clone();
            Box::pin(async move {
                let config = crate::commands::doctor::DoctorConfig { project_dir };
                let report = crate::commands::doctor::run_doctor(&config)?;
                let (stdout, stderr) = crate::commands::doctor::format_report(&report);
                print!("{stdout}");
                eprint!("{stderr}");
                Ok(())
            })
        }
    }

    pub struct RebuildHandler;

    impl CommandHandler for RebuildHandler {
        fn name(&self) -> &'static str {
            "rebuild"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Rebuild {
                ref project_dir,
                ref projection_id,
                list_projections,
                force,
            } = cli.command
            else {
                return Box::pin(async { Err(CliError::Dispatch("not a rebuild command".to_string())) });
            };
            let project_dir = project_dir.clone();
            let projection_id = projection_id.clone();
            Box::pin(async move {
                let config = crate::commands::rebuild::RebuildConfig {
                    project_dir,
                    projection_id,
                    list_projections,
                    force,
                    schema_version: None,
                };
                let report = crate::commands::rebuild::run_rebuild(&config)?;
                println!("{}", report.format_progress());
                Ok(())
            })
        }
    }

    pub struct WorkspaceHandler;

    impl CommandHandler for WorkspaceHandler {
        fn name(&self) -> &'static str {
            "workspace"
        }

        fn execute(&self, cli: &Cli) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Workspace {
                ref project_dir,
                ref subcommand,
            } = cli.command
            else {
                return Box::pin(async { Err(CliError::Dispatch("not a workspace command".to_string())) });
            };
            let project_dir = project_dir.clone();
            let subcommand = subcommand.clone();
            Box::pin(async move {
                let config = crate::commands::workspace::WorkspaceConfig {
                    project_dir,
                };
                let output = crate::commands::workspace::run_workspace(&config, subcommand)?;
                println!("{}", output);
                Ok(())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn registry_contains_all_commands() {
        let registry = HandlerRegistry::default();
        let names = registry.names();
        assert!(names.contains(&"purge"));
        assert!(names.contains(&"check"));
        assert!(names.contains(&"gc"));
        assert!(names.contains(&"init"));
        assert!(names.contains(&"lock"));
        assert!(names.contains(&"doctor"));
        assert!(names.contains(&"rebuild"));
        assert!(names.contains(&"workspace"));
    }

    #[test]
    fn registry_lookup_returns_handler() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Check {
                path: PathBuf::from("/tmp"),
            },
        };
        let handler = registry.get(&cli).expect("handler found");
        assert_eq!(handler.name(), "check");
    }

    #[test]
    fn registry_lookup_purge() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Purge {
                instance: "test".to_string(),
            },
        };
        let handler = registry.get(&cli).expect("handler found");
        assert_eq!(handler.name(), "purge");
    }

    #[test]
    fn registry_lookup_rebuild() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Rebuild {
                project_dir: PathBuf::from("/tmp"),
                projection_id: None,
                list_projections: false,
                force: false,
            },
        };
        let handler = registry.get(&cli).expect("handler found");
        assert_eq!(handler.name(), "rebuild");
    }
}
