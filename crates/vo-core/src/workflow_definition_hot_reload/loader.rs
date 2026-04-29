use std::path::Path;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use vo_types::WorkflowDefinition;

use super::error::Error;
use super::registry::SharedWorkflowRegistry;

const GRAPH_TIMEOUT_SECS: u64 = 10;

pub struct WorkflowDefinitionLoader {
    registry: SharedWorkflowRegistry,
}

impl WorkflowDefinitionLoader {
    pub fn new(registry: SharedWorkflowRegistry) -> Self {
        Self { registry }
    }

    pub async fn load_from_binary<P: AsRef<Path>>(
        &self,
        binary_path: P,
    ) -> Result<WorkflowDefinition, Error> {
        let binary_path = binary_path.as_ref();
        let binary_str = binary_path.to_string_lossy().to_string();

        debug!(binary = %binary_str, "loading workflow definition via --graph");

        let graph_output = self.run_graph_command(&binary_str).await?;

        let definition = Self::parse_workflow_definition(&graph_output)
            .map_err(|e| Error::ValidationFailed {
                workflow: "unknown".to_string(),
                reason: e.to_string(),
            })?;

        let workflow_name = definition.workflow_name.clone();
        let node_count = definition.nodes.len();
        self.registry.register(
            workflow_name.clone(),
            definition.clone(),
            binary_path.to_path_buf(),
        );

        info!(
            workflow_name = %workflow_name,
            binary = %binary_str,
            node_count = node_count,
            "loaded workflow definition"
        );

        Ok(definition)
    }

    pub async fn reload_from_binary<P: AsRef<Path>>(
        &self,
        binary_path: P,
    ) -> Result<Option<WorkflowDefinition>, Error> {
        let binary_path = binary_path.as_ref();
        let binary_str = binary_path.to_string_lossy().to_string();

        debug!(binary = %binary_str, "reloading workflow definition via --graph");

        let graph_output = match self.run_graph_command(&binary_str).await {
            Ok(output) => output,
            Err(e) => {
                error!(
                    binary = %binary_str,
                    error = %e,
                    "failed to reload workflow definition - keeping old definition"
                );
                return Err(e);
            }
        };

        let definition = match Self::parse_workflow_definition(&graph_output) {
            Ok(def) => def,
            Err(e) => {
                error!(
                    binary = %binary_str,
                    error = %e,
                    "failed to parse --graph output - keeping old definition"
                );
                return Err(Error::ValidationFailed {
                    workflow: "unknown".to_string(),
                    reason: e.to_string(),
                });
            }
        };

        let workflow_name = definition.workflow_name.clone();
        let node_count = definition.nodes.len();
        self.registry.register(
            workflow_name.clone(),
            definition.clone(),
            binary_path.to_path_buf(),
        );

        info!(
            workflow_name = %workflow_name,
            binary = %binary_str,
            node_count = node_count,
            "reloaded workflow definition"
        );

        Ok(Some(definition))
    }

    fn parse_workflow_definition(
        json_bytes: &[u8],
    ) -> Result<WorkflowDefinition, vo_types::WorkflowDefinitionError> {
        let mut deserializer = serde_json::Deserializer::from_slice(json_bytes);
        WorkflowDefinition::from_deserializer(&mut deserializer)
    }

    async fn run_graph_command(&self, binary_path: &str) -> Result<Vec<u8>, Error> {
        let mut child = Command::new(binary_path)
            .arg("--graph")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .spawn()
            .map_err(|e| Error::SpawnFailed {
                binary: binary_path.to_string(),
                source: e,
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
            stdout
                .read_to_end(&mut buf)
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok::<Vec<u8>, std::io::Error>(buf)
        });

        let read_stderr = tokio::spawn(async move {
            let mut buf = Vec::new();
            stderr
                .read_to_end(&mut buf)
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok::<Vec<u8>, std::io::Error>(buf)
        });

        let (stdout_result, stderr_result) = tokio::time::timeout(
            Duration::from_secs(GRAPH_TIMEOUT_SECS),
            async { tokio::join!(read_stdout, read_stderr) },
        )
        .await
        .map_err(|_| Error::BinaryTimeout {
            binary: binary_path.to_string(),
            timeout_secs: GRAPH_TIMEOUT_SECS,
        })?;

        let stdout_bytes = match stdout_result {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(e)) => {
                return Err(Error::SpawnFailed {
                    binary: binary_path.to_string(),
                    source: e,
                });
            }
            Err(_) => {
                return Err(Error::BinaryTimeout {
                    binary: binary_path.to_string(),
                    timeout_secs: GRAPH_TIMEOUT_SECS,
                });
            }
        };

        let stderr_bytes = match stderr_result {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(e)) => {
                return Err(Error::SpawnFailed {
                    binary: binary_path.to_string(),
                    source: e,
                });
            }
            Err(_) => {
                return Err(Error::BinaryTimeout {
                    binary: binary_path.to_string(),
                    timeout_secs: GRAPH_TIMEOUT_SECS,
                });
            }
        };

        let exit_code = child
            .wait()
            .await
            .map_err(|e| Error::SpawnFailed {
                binary: binary_path.to_string(),
                source: e,
            })?;

        if let Some(code) = exit_code.code() {
            if code != 0 {
                let stderr_str = String::from_utf8_lossy(&stderr_bytes);
                warn!(
                    binary = %binary_path,
                    exit_code = %code,
                    stderr = %stderr_str,
                    "--graph command failed"
                );
                return Err(Error::BinaryFailed {
                    binary: binary_path.to_string(),
                    code,
                    stderr: stderr_str.to_string(),
                });
            }
        }

        if stdout_bytes.is_empty() {
            return Err(Error::NoGraphOutput {
                binary: binary_path.to_string(),
            });
        }

        Ok(stdout_bytes)
    }
}