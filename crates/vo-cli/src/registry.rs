use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::{ApiKeySubcommand, Cli, CliError, Command};
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
        registry.register(Box::new(handlers::ServeHandler));
        registry.register(Box::new(handlers::HistoryHandler));
        registry.register(Box::new(handlers::ExecuteNodeHandler));
        registry.register(Box::new(handlers::ApiKeyHandler));
        registry.register(Box::new(handlers::HardlineHandler));
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
        Command::Hardline { .. } => Some("hardline"),
        Command::Serve { .. } => Some("serve"),
        Command::History { .. } => Some("history"),
        Command::ExecuteNode { .. } => Some("execute-node"),
        Command::ApiKey { .. } => Some("apikey"),
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
            let storage_path = PathBuf::from(".vo/storage");
            Box::pin(async move {
                let db = fjall::Database::builder(&storage_path)
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
            let Command::Check { ref path, workflow } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a check command".to_string()))
                });
            };
            let path = path.clone();
            let workflow = workflow;
            Box::pin(async move {
                if workflow {
                    let def = crate::commands::check::validate_workflow_spec(&path)?;
                    println!(
                        "{}: valid workflow spec '{}' ({} nodes)",
                        path.display(),
                        def.workflow_name.as_str(),
                        def.nodes.as_slice().len()
                    );
                } else {
                    crate::commands::check::run_check(&path)?;
                }
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
                if !force {
                    if !crate::commands::compensate::prompt_confirmation(&workflow_id) {
                        return Err(CliError::Compensate(
                            crate::commands::compensate::CompensateError::Aborted,
                        ));
                    }
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

    pub struct ServeHandler;

    impl CommandHandler for ServeHandler {
        fn name(&self) -> &'static str {
            "serve"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Serve {
                ref host,
                ref port,
                ref storage_path,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a serve command".to_string()))
                });
            };
            let host = host.clone();
            let port = *port;
            let storage_path = storage_path.clone();
            Box::pin(async move {
                let config = crate::commands::serve::ServeConfig {
                    host,
                    port,
                    storage_path,
                };
                crate::commands::serve::run_serve(&config).await?;
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
                ref workflow_id,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a status command".to_string()))
                });
            };
            let engine_url = engine_url.clone();
            let workflow_id = workflow_id.clone();
            Box::pin(async move {
                let config = crate::commands::status::StatusConfig {
                    engine_url,
                    instance_id: workflow_id,
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

    pub struct HistoryHandler;

    impl CommandHandler for HistoryHandler {
        fn name(&self) -> &'static str {
            "history"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::History {
                ref instance_id,
                ref engine_url,
                json,
                ..
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a history command".to_string()))
                });
            };
            let instance_id = instance_id.clone();
            let engine_url = engine_url.clone();
            Box::pin(async move {
                let config = crate::commands::workflow_history::WorkflowHistoryConfig {
                    instance_id,
                    engine_url,
                    json,
                };
                let result =
                    crate::commands::workflow_history::run_workflow_history(&config).await?;
                if json {
                    let json_output = serde_json::to_string_pretty(&result).map_err(|e| {
                        CliError::Dispatch(format!("failed to serialize history: {e}"))
                    })?;
                    println!("{json_output}");
                } else {
                    for entry in &result.entries {
                        println!(
                            "[{}] {} step={} type={}",
                            entry.sequence,
                            entry.timestamp_ms,
                            entry.step_id.as_deref().unwrap_or("-"),
                            entry.event_type,
                        );
                        if let Some(ref err) = entry.error {
                            println!("  error: {err}");
                        }
                    }
                }
                Ok(())
            })
        }
    }

    pub struct ExecuteNodeHandler;

    impl CommandHandler for ExecuteNodeHandler {
        fn name(&self) -> &'static str {
            "execute-node"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::ExecuteNode {
                ref binary_path,
                ref node_name,
                ref input,
                timeout,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not an execute-node command".to_string()))
                });
            };
            let binary_path = binary_path.clone();
            let node_name = node_name.clone();
            let input = input.clone();
            let timeout = timeout;
            Box::pin(async move {
                let binary_path_str = binary_path.to_string_lossy().to_string();

                let graph_result = super::execute_with_graph(&binary_path_str).await?;

                let workflow_spec: vo_sdk::WorkflowSpec =
                    serde_json::from_slice(&graph_result).map_err(|e| {
                        CliError::ExecuteNode(format!(
                            "failed to parse workflow spec from --graph output: {e}"
                        ))
                    })?;

                let node = workflow_spec
                    .nodes
                    .iter()
                    .find(|n| n.name.as_str() == node_name)
                    .ok_or_else(|| {
                        CliError::ExecuteNode(format!(
                            "node '{node_name}' not found in workflow '{:?}' (available: {})",
                            workflow_spec.workflow_name,
                            workflow_spec
                                .nodes
                                .iter()
                                .map(|n| n.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    })?;

                println!("Executing node '{}'", node.name);
                println!("  kind: {:?}", node.kind);
                println!("  retry_policy: {:?}", node.retry_policy);

                let fd3_payload = input.unwrap_or_default();
                let output = super::run_node_subprocess(&binary_path_str, &fd3_payload, timeout).await?;

                let output_str = String::from_utf8_lossy(&output.fd4_bytes);
                if !output_str.is_empty() {
                    println!("output: {output_str}");
                }
                println!(
                    "exit_code: {}",
                    output.exit_code.map_or("null".to_string(), |c| c.to_string())
                );
                Ok(())
            })
       }
    }

    pub struct ApiKeyHandler;

    impl CommandHandler for ApiKeyHandler {
        fn name(&self) -> &'static str {
            "apikey"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::ApiKey { ref subcommand } = cli.command else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not an apikey command".to_string()))
                });
            };
            let subcommand = subcommand.clone();
            Box::pin(async move {
                let storage_path = PathBuf::from(".vo/storage");
                let db = fjall::Database::builder(&storage_path)
                    .open()
                    .map_err(|e| CliError::Dispatch(format!("Failed to open database: {e}")))?;
                let api_key_store = vo_storage::api_key_partition::FjallApiKeyStore::open(&db)
                    .map_err(|e| CliError::Dispatch(format!("Failed to open API key store: {e}")))?;
                match subcommand {
                    crate::cli::ApiKeySubcommand::Create { name, expires_in_days } => {
                        let raw_key = super::generate_api_key();
                        let key_id = api_key_store
                            .create_key(&raw_key, &name)
                            .map_err(|e| CliError::Dispatch(format!("Failed to create API key: {e}")))?;
                        println!("Created API key '{name}' with ID: {key_id}");
                        if let Some(days) = expires_in_days {
                            println!("Expires in {days} days");
                        }
                        println!("\nIMPORTANT: Save this API key - it will not be shown again:");
                        println!("{raw_key}");
                        Ok(())
                    }
                    crate::cli::ApiKeySubcommand::List => {
                        let keys = api_key_store
                            .list_keys()
                            .map_err(|e| CliError::Dispatch(format!("Failed to list API keys: {e}")))?;
                        if keys.is_empty() {
                            println!("No API keys found.");
                        } else {
                            println!("API Keys:");
                            println!("{:<36} {:<20} {:<12}", "ID", "Name", "Status");
                            println!("{}", "-".repeat(68));
                            for key in keys {
                                let status = if key.revoked {
                                    "REVOKED".to_string()
                                } else if key.expires_at.is_some() {
                                    "EXPIRED".to_string()
                                } else {
                                    "ACTIVE".to_string()
                                };
                                println!("{:<36} {:<20} {:<12}", key.key_id, key.name, status);
                            }
                        }
                        Ok(())
                    }
                    crate::cli::ApiKeySubcommand::Revoke { key_id } => {
                        api_key_store
                            .revoke_key(&key_id)
                            .map_err(|e| CliError::Dispatch(format!("Failed to revoke API key: {e}")))?;
                        println!("Revoked API key: {key_id}");
                        Ok(())
                    }
                }
            })
        }
    }

    pub struct HardlineHandler;

    impl CommandHandler for HardlineHandler {
        fn name(&self) -> &'static str {
            "hardline"
        }

        fn execute(
            &self,
            cli: &Cli,
        ) -> Pin<Box<dyn Future<Output = Result<(), CliError>> + Send + '_>> {
            let Command::Hardline {
                ref target,
                ref engine_url,
                timeout,
                force,
                dry_run,
            } = cli.command
            else {
                return Box::pin(async {
                    Err(CliError::Dispatch("not a hardline command".to_string()))
                });
            };
            let target = target.clone();
            let engine_url = engine_url.clone();
            Box::pin(async move {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(timeout))
                    .build()
                    .map_err(|e| {
                        CliError::Dispatch(format!("Failed to build HTTP client: {e}"))
                    })?;

                #[derive(serde::Serialize)]
                struct HardlineRequest {
                    target: String,
                    force: bool,
                    dry_run: bool,
                }

                let url = format!("{}/api/v1/hardline", engine_url);
                let response = client
                    .post(&url)
                    .json(&HardlineRequest {
                        target,
                        force,
                        dry_run,
                    })
                    .send()
                    .await
                    .map_err(|e| {
                        CliError::Dispatch(format!("Failed to send hardline request: {e}"))
                    })?;

                let status = response.status();
                if status.is_success() {
                    println!("Hardline command executed successfully.");
                    Ok(())
                } else {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "unknown error".to_string());
                    Err(CliError::Dispatch(format!(
                        "Hardline command failed ({}): {}",
                        status, body
                    )))
                }
            })
        }
    }
}

fn generate_api_key() -> String {
    let ulid = ulid::Ulid::new();
    format!("vo_sk_{}", ulid.to_string())
}

  async fn execute_with_graph(binary_path: &str) -> Result<Vec<u8>, CliError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    let mut child = Command::new(binary_path)
        .arg("--graph")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            CliError::ExecuteNode(format!("failed to spawn binary '{binary_path}': {e}"))
        })?;

    let mut stdout = child
        .stdout
        .take()
        .expect("child stdout should be piped");
    let mut stderr = child
        .stderr
        .take()
        .expect("child stderr should be piped");

    let read_stdout = tokio::spawn(async move {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).await.map_err(|e| {
            CliError::ExecuteNode(format!("failed to read stdout: {e}"))
        })?;
        Ok::<Vec<u8>, CliError>(buf)
    });

    let read_stderr = tokio::spawn(async move {
        let mut buf = Vec::new();
        stderr.read_to_end(&mut buf).await.map_err(|e| {
            CliError::ExecuteNode(format!("failed to read stderr: {e}"))
        })?;
        Ok::<Vec<u8>, CliError>(buf)
    });

    let result = timeout(
        Duration::from_secs(10),
        async {
            tokio::try_join!(read_stdout, read_stderr)
        },
    )
    .await
    .map_err(|_| CliError::ExecuteNode(format!("timeout reading binary output")))?;

    let (stdout_result, stderr_result) = result.map_err(|e| {
        CliError::ExecuteNode(format!("task join error: {e}"))
    })?;
    let stdout_bytes = stdout_result?;
    let stderr_bytes = stderr_result?;

    let exit_code = child
        .wait()
        .await
        .map_err(|e| CliError::ExecuteNode(format!("failed to wait for child: {e}")))?;

    if let Some(code) = exit_code.code() {
        if code != 0 {
            let stderr_str = String::from_utf8_lossy(&stderr_bytes);
            return Err(CliError::ExecuteNode(format!(
                "binary exited with code {code}: {stderr_str}"
            )));
        }
    }

    if stdout_bytes.is_empty() {
        return Err(CliError::ExecuteNode(
            "--graph produced no output".to_string(),
        ));
    }

    Ok(stdout_bytes)
}

async fn run_node_subprocess(
    binary_path: &str,
    fd3_payload: &[u8],
    timeout_secs: u64,
) -> Result<vo_executor::SubprocessOutput, CliError> {
    let config = vo_executor::SubprocessConfig::new(
        binary_path.to_string(),
        vec![],
        timeout_secs * 1000,
        fd3_payload.to_vec(),
    ).unwrap();
    vo_executor::run_subprocess(config).await.map_err(|e| {
        CliError::ExecuteNode(format!("subprocess execution failed: {e}"))
    })
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
        assert!(names.contains(&"execute-node"));
        assert!(names.contains(&"hardline"));
    }

    #[test]
    fn registry_lookup_returns_handler() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Check {
                workflow: false,
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
                storage_path: PathBuf::from(".vo/storage"),
                dry_run: false,
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

    #[test]
    fn registry_lookup_hardline() {
        let registry = HandlerRegistry::default();
        let cli = Cli {
            command: Command::Hardline {
                target: "test-target".to_string(),
                engine_url: "http://localhost:3000".to_string(),
                timeout: 60,
                force: false,
                dry_run: false,
            },
        };
        let handler = registry.get(&cli).expect("handler found");
        assert_eq!(handler.name(), "hardline");
    }
}
