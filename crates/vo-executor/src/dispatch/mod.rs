//! ADR-003: NodeKind-based execution dispatch for raw binary subprocesses.
//!
//! This module routes subprocess execution based on the workflow node's `NodeKind`
//! classification, interpreting the FD4 response envelope according to the step class:
//!
//! - **Pure**: Awaits deterministic output (TaskResult::Success or Failure).
//! - **ManagedEffect**: Awaits typed EffectIntent for engine-side commit.
//! - **Unsafe**: Awaits output but treated as at-least-once only.
//! - **Wait/Signal**: Deferred — no subprocess execution, returns immediately.

use std::collections::BTreeMap;

use crate::errors::ExecuteNodeError;
use crate::types::StepResult;
use vo_ipc::envelope::{Fd3Envelope, Fd4Envelope, TaskResult};
use vo_ipc::{run_subprocess, IpcError, SubprocessConfig as IpcSubprocessConfig};
use vo_types::NodeKind;

/// Result of dispatching a node execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDispatchResult {
    pub step_result: StepResult,
    pub stderr_bytes: Vec<u8>,
    pub stderr_truncated: bool,
}

/// Dispatch a node execution based on its `NodeKind`.
///
/// For Pure, ManagedEffect, and Unsafe nodes, this spawns a subprocess and
/// interprets the FD4 response according to the step class.
/// For Wait, Signal, and Router nodes, returns immediately without spawning.
///
/// # Errors
///
/// Returns `ExecuteNodeError` if:
/// - IPC configuration fails
/// - Subprocess spawn or communication fails
/// - The child returns an unexpected result type for the given NodeKind
pub async fn dispatch_node(
    kind: NodeKind,
    executable_path: &std::path::Path,
    timeout_ms: u64,
    instance_id: &str,
    node_id: &str,
    input: serde_json::Value,
    secrets: BTreeMap<String, String>,
    metadata: BTreeMap<String, String>,
) -> Result<NodeDispatchResult, ExecuteNodeError> {
    match kind {
        NodeKind::Pure | NodeKind::Unsafe | NodeKind::ManagedEffect => {
            dispatch_subprocess(
                kind,
                executable_path,
                timeout_ms,
                instance_id,
                node_id,
                input,
                secrets,
                metadata,
            )
            .await
        }
        NodeKind::Wait => Ok(NodeDispatchResult {
            step_result: StepResult::Success {
                output: "wait_deferred".to_string(),
            },
            stderr_bytes: vec![],
            stderr_truncated: false,
        }),
        NodeKind::Signal => Ok(NodeDispatchResult {
            step_result: StepResult::Success {
                output: "signal_emitted".to_string(),
            },
            stderr_bytes: vec![],
            stderr_truncated: false,
        }),
        NodeKind::Router => Ok(NodeDispatchResult {
            step_result: StepResult::Success {
                output: "router_nop".to_string(),
            },
            stderr_bytes: vec![],
            stderr_truncated: false,
        }),
    }
}

async fn dispatch_subprocess(
    kind: NodeKind,
    executable_path: &std::path::Path,
    timeout_ms: u64,
    instance_id: &str,
    node_id: &str,
    input: serde_json::Value,
    secrets: BTreeMap<String, String>,
    metadata: BTreeMap<String, String>,
) -> Result<NodeDispatchResult, ExecuteNodeError> {
    let fd3 = Fd3Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        input,
        secrets,
        metadata,
    };

    let fd3_payload = serde_json::to_vec(&fd3).map_err(|e| ExecuteNodeError::DispatchIpc {
        detail: format!("fd3 serialize: {e}"),
    })?;

    let ipc_config =
        IpcSubprocessConfig::new(executable_path, timeout_ms, fd3_payload).map_err(|e| {
            ExecuteNodeError::DispatchIpc {
                detail: format!("config: {e}"),
            }
        })?;

    let output = run_subprocess(ipc_config).await.map_err(|e| match e {
        IpcError::Timeout { elapsed_ms, .. } => ExecuteNodeError::TimeoutExceeded {
            elapsed_ms,
            limit_ms: timeout_ms,
        },
        IpcError::ProcessFailed { exit_code, .. } => ExecuteNodeError::TransientError {
            reason: format!("process exited with code {exit_code}"),
            recoverable: false,
        },
        other => ExecuteNodeError::DispatchIpc {
            detail: format!("subprocess: {other}"),
        },
    })?;

    let fd4: Fd4Envelope =
        serde_json::from_slice(&output.fd4_bytes).map_err(|e| ExecuteNodeError::DispatchIpc {
            detail: format!("fd4 parse: {e}"),
        })?;

    let step_result = interpret_result(kind, &fd4)?;

    Ok(NodeDispatchResult {
        step_result,
        stderr_bytes: output.stderr_bytes,
        stderr_truncated: output.stderr_truncated,
    })
}

fn interpret_result(
    kind: NodeKind,
    envelope: &Fd4Envelope,
) -> Result<StepResult, ExecuteNodeError> {
    match (&envelope.result, kind) {
        (TaskResult::Success { output }, NodeKind::Pure | NodeKind::Unsafe) => {
            Ok(StepResult::Success {
                output: output.to_string(),
            })
        }
        (TaskResult::Failure { error }, NodeKind::Pure | NodeKind::Unsafe) => {
            Ok(StepResult::Failure {
                output: error.message.clone(),
            })
        }
        (TaskResult::EffectIntent { intent }, NodeKind::ManagedEffect) => {
            let effect_kind = intent
                .get("effect_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let params = intent
                .get("params")
                .map(|v| v.to_string())
                .unwrap_or_default();
            let connector_id = intent
                .get("connector_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(StepResult::EffectIntent {
                effect_kind,
                params,
                connector_id,
            })
        }
        (TaskResult::Failure { error }, NodeKind::ManagedEffect) => Ok(StepResult::Failure {
            output: error.message.clone(),
        }),
        (TaskResult::EffectIntent { .. }, NodeKind::Pure) => {
            Err(ExecuteNodeError::DispatchMismatch {
                node_kind: "pure".to_string(),
                expected: "success_or_failure".to_string(),
                got: "effect_intent".to_string(),
            })
        }
        (TaskResult::EffectIntent { .. }, NodeKind::Unsafe) => {
            Err(ExecuteNodeError::DispatchMismatch {
                node_kind: "unsafe".to_string(),
                expected: "success_or_failure".to_string(),
                got: "effect_intent".to_string(),
            })
        }
        (TaskResult::Success { .. }, NodeKind::ManagedEffect) => {
            Err(ExecuteNodeError::DispatchMismatch {
                node_kind: "managed_effect".to_string(),
                expected: "effect_intent_or_failure".to_string(),
                got: "success".to_string(),
            })
        }
        (_, NodeKind::Wait | NodeKind::Signal | NodeKind::Router) => Err(ExecuteNodeError::DispatchMismatch {
            node_kind: match kind {
                NodeKind::Wait => "wait".to_string(),
                NodeKind::Signal => "signal".to_string(),
                NodeKind::Router => "router".to_string(),
                _ => unreachable!(),
            },
            expected: "no_subprocess".to_string(),
            got: format!("{:?}", envelope.result).to_lowercase(),
        }),
    }
}

#[cfg(test)]
mod tests;
