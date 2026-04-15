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
        registry.register(Box::new(handlers::CompensateHandler));
        registry.register(Box::new(handlers::GcHandler));
        registry.register(Box::new(handlers::InitHandler));
        registry.register(Box::new(handlers::LockHandler));
        registry.register(Box::new(handlers::DoctorHandler));
        registry.register(Box::new(handlers::RebuildHandler));
        registry.register(Box::new(handlers::StatusHandler));
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
        Command::Compensate { .. } => Some("compensate"),
        Command::Gc { .. } => Some("gc"),
        Command::Init { .. } => Some("init"),
        Command::Lock { .. } => Some("lock"),
        Command::Doctor { .. } => Some("doctor"),
        Command::Rebuild { .. } => Some("rebuild"),
        Command::Status { .. } => Some("status"),
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Purge { ref instance } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a purge command".to_string()))
                });
            };
            let instance = instance.clone();
            Box::pin(async move {
                let fjall_path = std::path::Path::new("/home/lewis/.gemini/tmp/veloxide/fjall");
                let db = fjall::Database::builder(fjall_path)
                    .open()
                    .map_err(|e| CliError::Dispatch(format!("Failed to open database: {e}")))?;

                match vo_storage::purge::purge_instance(&db, &instance) {
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Check { ref path } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a check command".to_string()))
                });
            };
            let path = path.clone();
            Box::pin(async move {
                crate::commands::check::run_check(&path)?;
                Ok(())
            })
        }
    }

    pub struct CompensateHandler;

    impl CommandHandler for CompensateHandler {
        fn name(&self) -> &'static str {
            "compensate"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Compensate {
                ref engine_url,
                ref workflow_id,
                force,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a compensate command".to_string()))
                });
            };
            let engine_url = engine_url.clone();
            let workflow_id = workflow_id.clone();
            Box::pin(async move {
                if !force && !crate::commands::compensate::prompt_confirmation(&workflow_id) {
                    return Err(CliError::Compensate(
                        crate::commands::compensate::CompensateError::Aborted,
                    ));
                }
                let config = crate::commands::compensate::CompensateConfig {
                    engine_url,
                    workflow_id,
                    force,
                };
                crate::commands::compensate::run_compensate(&config).await?;
                Ok(())
            })
        }
    }

    pub struct GcHandler;

    impl CommandHandler for GcHandler {
        fn name(&self) -> &'static str {
            "gc"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Init {
                ref project_dir,
                ref engine_url,
                ref storage_path,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not an init command".to_string()))
                });
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Lock { ref project_dir } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a lock command".to_string()))
                });
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Doctor { ref project_dir } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a doctor command".to_string()))
                });
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

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Rebuild {
                ref project_dir,
                ref projection_id,
                list_projections,
                force,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a rebuild command".to_string()))
                });
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

    pub struct StatusHandler;

    impl CommandHandler for StatusHandler {
        fn name(&self) -> &'static str {
            "status"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Status {
                ref engine_url,
                ref instance,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a status command".to_string()))
                });
            };
            let engine_url = engine_url.clone();
            let instance = instance.clone();
            Box::pin(async move {
                let config = crate::commands::status::StatusConfig {
                    engine_url,
                    instance_id: instance,
                };
                let status = crate::commands::status::run_status(&config).await?;
                println!("+---------------------------+-------------------------------+");
                println!("| Field                     | Value                         |");
                println!("+---------------------------+-------------------------------+");
                println!("| Instance ID               | {} |", status.instance_id);
                println!("| Namespace                 | {} |", status.namespace);
                println!("| Workflow Type             | {} |", status.workflow_type);
                println!("| Paradigm                  | {} |", status.paradigm);
                println!("| Phase                     | {} |", status.phase);
                println!("| Events Applied           | {} |", status.events_applied);
                if let Some(reg_status) = status.registration_status {
                    println!("| Registration              | {} |", reg_status);
                }
                if status.is_quarantined {
                    println!("| Quarantined               | yes                          |");
                }
                println!("+---------------------------+-------------------------------+");
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
        assert!(names.contains(&"compensate"));
        assert!(names.contains(&"gc"));
        assert!(names.contains(&"init"));
        assert!(names.contains(&"lock"));
        assert!(names.contains(&"doctor"));
        assert!(names.contains(&"rebuild"));
        assert!(names.contains(&"status"));
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

    #[test]
    fn registry_lookup_compensate() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Compensate {
                engine_url: "http://localhost:3000".to_string(),
                workflow_id: "wf-test".to_string(),
                force: false,
            },
        };
        let handler = registry.get(&cli).expect("handler found");
        assert_eq!(handler.name(), "compensate");
    }
}
