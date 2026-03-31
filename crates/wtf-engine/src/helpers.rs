use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use wtf_types::{WorkflowDefinition, WorkflowDefinitionError, WorkflowName};

use crate::error::BinaryRegistryError;
use crate::types::BinaryPath;

const ETXTBSY_MAX_RETRIES: u32 = 5;
const ETXTBSY_RETRY_DELAY: Duration = Duration::from_millis(10);
const GRAPH_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn verify_source(
    source: &Path,
    source_path: &BinaryPath,
) -> Result<(), BinaryRegistryError> {
    if !source.exists() || !source.is_file() {
        return Err(BinaryRegistryError::BinaryNotFound {
            path: source_path.clone(),
        });
    }
    let metadata = fs::metadata(source).map_err(|e| BinaryRegistryError::HashFailed {
        path: source_path.clone(),
        source: e,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(BinaryRegistryError::NotExecutable {
                path: source_path.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn compute_binary_hash(
    path: &Path,
    source_path: &BinaryPath,
) -> Result<(Vec<u8>, String), BinaryRegistryError> {
    let content = fs::read(path).map_err(|e| BinaryRegistryError::HashFailed {
        path: source_path.clone(),
        source: e,
    })?;
    let hash = Sha256::digest(&content);
    Ok((content, format!("{:x}", hash)))
}

pub(crate) fn copy_to_versions(
    source: &Path,
    versions_dir: &BinaryPath,
    hex_hash: &str,
    source_path: &BinaryPath,
) -> Result<(BinaryPath, bool), BinaryRegistryError> {
    let versioned_file_path = versions_dir.as_path().join(hex_hash);
    let versioned_binary_path =
        BinaryPath::new(versioned_file_path.clone()).expect("join of absolute is absolute");
    let did_copy = !versioned_file_path.exists();
    if did_copy {
        if let Some(parent) = versioned_file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| BinaryRegistryError::CopyFailed {
                src: source_path.clone(),
                dst: versioned_binary_path.clone(),
                source: e,
            })?;
        }
        fs::copy(source, &versioned_file_path).map_err(|e| BinaryRegistryError::CopyFailed {
            src: source_path.clone(),
            dst: versioned_binary_path.clone(),
            source: e,
        })?;
        let f =
            fs::File::open(&versioned_file_path).map_err(|e| BinaryRegistryError::CopyFailed {
                src: source_path.clone(),
                dst: versioned_binary_path.clone(),
                source: e,
            })?;
        f.sync_all().map_err(|e| BinaryRegistryError::CopyFailed {
            src: source_path.clone(),
            dst: versioned_binary_path.clone(),
            source: e,
        })?;
        drop(f);
    }
    Ok((versioned_binary_path, did_copy))
}

fn spawn_graph_command(binary_path: &Path) -> Result<std::process::Child, std::io::Error> {
    let mut last_err = None;
    for _ in 0..=ETXTBSY_MAX_RETRIES {
        match Command::new(binary_path)
            .arg("--graph")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(e) if e.raw_os_error() == Some(26) => {
                last_err = Some(e);
                std::thread::sleep(ETXTBSY_RETRY_DELAY);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap())
}

fn wait_child_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>), String> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout_buf = Vec::new();
                let mut stderr_buf = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut stdout_buf);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut stderr_buf);
                }
                return Ok((status, stdout_buf, stderr_buf));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "--graph subprocess timed out after {}s",
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

pub(crate) fn discover_graph(
    binary_path: &Path,
    workflow_name: &WorkflowName,
) -> Result<WorkflowDefinition, BinaryRegistryError> {
    let mut child = spawn_graph_command(binary_path).map_err(|e| {
        BinaryRegistryError::GraphDiscoveryFailed {
            workflow_name: workflow_name.clone(),
            exit_code: -1,
            stderr: e.to_string(),
        }
    })?;
    let (status, stdout_bytes, stderr_bytes) = wait_child_with_timeout(&mut child, GRAPH_TIMEOUT)
        .map_err(|msg| {
        BinaryRegistryError::GraphDiscoveryFailed {
            workflow_name: workflow_name.clone(),
            exit_code: -1,
            stderr: msg,
        }
    })?;
    if !status.success() {
        return Err(BinaryRegistryError::GraphDiscoveryFailed {
            workflow_name: workflow_name.clone(),
            exit_code: status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
        });
    }
    parse_graph_output(&String::from_utf8_lossy(&stdout_bytes), workflow_name)
}

fn parse_graph_output(
    stdout: &str,
    workflow_name: &WorkflowName,
) -> Result<WorkflowDefinition, BinaryRegistryError> {
    match WorkflowDefinition::parse(stdout.as_bytes()) {
        Ok(def) => Ok(def),
        Err(WorkflowDefinitionError::DeserializationFailed { message }) => {
            if serde_json::from_str::<serde_json::Value>(stdout).is_ok() {
                Err(BinaryRegistryError::WorkflowDefinitionInvalid {
                    workflow_name: workflow_name.clone(),
                    reason: message,
                })
            } else {
                Err(BinaryRegistryError::InvalidGraphOutput {
                    workflow_name: workflow_name.clone(),
                    parse_error: message,
                })
            }
        }
        Err(e) => Err(BinaryRegistryError::WorkflowDefinitionInvalid {
            workflow_name: workflow_name.clone(),
            reason: e.to_string(),
        }),
    }
}
