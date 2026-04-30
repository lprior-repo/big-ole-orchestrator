//! ADR-003: Execute a single workflow node via raw OS subprocess.
//!
//! This command invokes `vo_executor::dispatch_node` to spawn a workflow binary
//! as a child process, sending input over fd3 and reading the result over fd4.
//! The node kind determines how the result is interpreted:
//!
//! - **pure**: Expects `TaskResult::Success` or `Failure`.
//! - **managed_effect**: Expects `TaskResult::EffectIntent` for engine-side commit.
//! - **unsafe**: Expects `Success` or `Failure` (at-least-once semantics).
//! - **wait / signal**: No subprocess spawned, returns immediately.

use std::collections::BTreeMap;
use std::path::PathBuf;

use thiserror::Error;

/// Errors from the `execute-node` command.
#[derive(Debug, Error)]
pub enum ExecuteNodeError {
    /// The binary path does not exist or is not executable.
    #[error("invalid binary: {path}: {reason}")]
    InvalidBinary { path: String, reason: String },

    /// Failed to parse the input JSON payload.
    #[error("invalid input JSON: {0}")]
    InvalidInput(#[from] serde_json::Error),

    /// Invalid node kind string.
    #[error("invalid node kind: {0} (expected: pure, managed_effect, unsafe, wait, signal)")]
    InvalidNodeKind(String),

    /// Invalid secret format (expected KEY=VALUE).
    #[error("invalid secret format: {0} (expected KEY=VALUE)")]
    InvalidSecret(String),

    /// The dispatch execution failed.
    #[error("dispatch failed: {0}")]
    DispatchFailed(String),
}

/// Configuration for the `execute-node` command.
pub struct ExecuteNodeConfig {
    /// Path to the workflow binary.
    pub binary: PathBuf,
    /// Name of the node to execute.
    pub node_name: String,
    /// Workflow instance ID.
    pub instance_id: String,
    /// Node execution ID.
    pub node_id: String,
    /// JSON input payload (optional, defaults to null).
    pub input: serde_json::Value,
    /// Secrets to inject via fd3 IPC.
    pub secrets: BTreeMap<String, String>,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Node kind classification.
    pub node_kind: vo_types::NodeKind,
}

/// Execute a single workflow node via subprocess dispatch.
///
/// This is the CLI entry point for ADR-003 raw binary execution.
/// It validates inputs, resolves the binary path, and delegates to
/// `vo_executor::dispatch_node`.
///
/// # Errors
///
/// Returns `ExecuteNodeError` if the binary is invalid, inputs are malformed,
/// or the subprocess dispatch fails.
pub async fn run_execute_node(config: &ExecuteNodeConfig) -> Result<(), ExecuteNodeError> {
    // Validate binary exists and is executable
    let binary_path = config.binary.as_path();
    if !binary_path.exists() {
        return Err(ExecuteNodeError::InvalidBinary {
            path: config.binary.display().to_string(),
            reason: "file does not exist".to_string(),
        });
    }
    if !binary_path.is_file() {
        return Err(ExecuteNodeError::InvalidBinary {
            path: config.binary.display().to_string(),
            reason: "not a regular file".to_string(),
        });
    }

    // Resolve version-pinned binary path
    let pinned = vo_executor::resolve_binary_path(config.binary.to_str().ok_or_else(|| {
        ExecuteNodeError::InvalidBinary {
            path: config.binary.display().to_string(),
            reason: "path is not valid UTF-8".to_string(),
        }
    })?)
    .map_err(|e| ExecuteNodeError::DispatchFailed(format!("version pinning: {e}")))?;

    // Dispatch the node execution
    let result = vo_executor::dispatch_node(
        config.node_kind.clone(),
        std::path::Path::new(&pinned.versioned_path),
        config.timeout_ms,
        &config.instance_id,
        &config.node_id,
        config.input.clone(),
        config.secrets.clone(),
        BTreeMap::new(),
    )
    .await
    .map_err(|e| ExecuteNodeError::DispatchFailed(e.to_string()))?;

    // Print stderr if captured
    if !result.stderr_bytes.is_empty() {
        let stderr_str = String::from_utf8_lossy(&result.stderr_bytes);
        if result.stderr_truncated {
            eprintln!("[stderr] {stderr_str}... (truncated)");
        } else {
            eprintln!("[stderr] {stderr_str}");
        }
    }

    // Print the result
    match result.step_result {
        vo_executor::StepResult::Success { output } => {
            println!("success: {output}");
        }
        vo_executor::StepResult::Failure { output } => {
            println!("failure: {output}");
        }
        vo_executor::StepResult::EffectIntent {
            ref effect_kind,
            ref connector_id,
            ..
        } => {
            println!("effect_intent: kind={effect_kind}, connector={connector_id}");
        }
    }

    Ok(())
}

/// Parse a node kind string into a `NodeKind`.
///
/// # Errors
///
/// Returns `ExecuteNodeError::InvalidNodeKind` if the string is not a valid kind.
pub fn parse_node_kind(s: &str) -> Result<vo_types::NodeKind, ExecuteNodeError> {
    match s {
        "pure" => Ok(vo_types::NodeKind::Pure),
        "managed_effect" => Ok(vo_types::NodeKind::ManagedEffect),
        "unsafe" => Ok(vo_types::NodeKind::Unsafe),
        "wait" => Ok(vo_types::NodeKind::Wait),
        "signal" => Ok(vo_types::NodeKind::Signal),
        other => Err(ExecuteNodeError::InvalidNodeKind(other.to_string())),
    }
}

/// Parse secret key=value pairs from CLI arguments.
///
/// # Errors
///
/// Returns `ExecuteNodeError::InvalidSecret` if any entry lacks an `=` separator.
pub fn parse_secrets(secrets: &[String]) -> Result<BTreeMap<String, String>, ExecuteNodeError> {
    let mut map = BTreeMap::new();
    for s in secrets {
        let (key, value) = s
            .split_once('=')
            .ok_or_else(|| ExecuteNodeError::InvalidSecret(s.clone()))?;
        if key.is_empty() {
            return Err(ExecuteNodeError::InvalidSecret(s.clone()));
        }
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_node_kind_all_variants() {
        assert!(matches!(parse_node_kind("pure"), Ok(NodeKind::Pure)));
        assert!(matches!(
            parse_node_kind("managed_effect"),
            Ok(NodeKind::ManagedEffect)
        ));
        assert!(matches!(parse_node_kind("unsafe"), Ok(NodeKind::Unsafe)));
        assert!(matches!(parse_node_kind("wait"), Ok(NodeKind::Wait)));
        assert!(matches!(parse_node_kind("signal"), Ok(NodeKind::Signal)));
    }

    #[test]
    fn parse_node_kind_invalid() {
        assert!(parse_node_kind("bogus").is_err());
        assert!(parse_node_kind("").is_err());
        assert!(parse_node_kind("PURE").is_err());
        assert!(parse_node_kind("ManagedEffect").is_err());
    }

    #[test]
    fn parse_secrets_valid() {
        let secrets = vec![
            "API_KEY=secret123".to_string(),
            "DB_PASS=pass".to_string(),
            "EMPTY_VAL=".to_string(),
        ];
        let map = parse_secrets(&secrets).unwrap();
        assert_eq!(map.get("API_KEY").unwrap(), "secret123");
        assert_eq!(map.get("DB_PASS").unwrap(), "pass");
        assert_eq!(map.get("EMPTY_VAL").unwrap(), "");
    }

    #[test]
    fn parse_secrets_empty() {
        let map = parse_secrets(&[]).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn parse_secrets_rejects_missing_equals() {
        let secrets = vec!["NO_EQUALS".to_string()];
        assert!(parse_secrets(&secrets).is_err());
    }

    #[test]
    fn parse_secrets_rejects_empty_key() {
        let secrets = vec!["=value".to_string()];
        assert!(parse_secrets(&secrets).is_err());
    }

    #[test]
    fn execute_node_error_display() {
        let err = ExecuteNodeError::InvalidBinary {
            path: "/tmp/binary".to_string(),
            reason: "not found".to_string(),
        };
        assert!(err.to_string().contains("/tmp/binary"));

        let err = ExecuteNodeError::InvalidNodeKind("bogus".to_string());
        assert!(err.to_string().contains("bogus"));

        let err = ExecuteNodeError::InvalidSecret("bad".to_string());
        assert!(err.to_string().contains("bad"));
    }

    use vo_types::NodeKind;
}
