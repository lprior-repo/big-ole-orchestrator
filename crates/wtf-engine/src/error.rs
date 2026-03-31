use crate::types::BinaryPath;
use thiserror::Error;
use wtf_types::WorkflowName;

#[derive(Debug, Error)]
pub enum BinaryRegistryError {
    #[error("binary not found at path: {path}")]
    BinaryNotFound { path: BinaryPath },

    #[error("binary is not executable: {path}")]
    NotExecutable { path: BinaryPath },

    #[error("failed to hash binary at {path}: {source}")]
    HashFailed {
        path: BinaryPath,
        source: std::io::Error,
    },

    #[error("failed to copy binary from {src} to {dst}: {source}")]
    CopyFailed {
        src: BinaryPath,
        dst: BinaryPath,
        source: std::io::Error,
    },

    #[error(
        "--graph failed for workflow '{workflow_name}': exit code {exit_code}, stderr: {stderr}"
    )]
    GraphDiscoveryFailed {
        workflow_name: WorkflowName,
        exit_code: i32,
        stderr: String,
    },

    #[error("--graph output for workflow '{workflow_name}' is not valid JSON: {parse_error}")]
    InvalidGraphOutput {
        workflow_name: WorkflowName,
        parse_error: String,
    },

    #[error("workflow '{workflow_name}' is deactivated")]
    WorkflowDeactivated { workflow_name: WorkflowName },

    #[error("workflow '{workflow_name}' not found in registry")]
    NotFound { workflow_name: WorkflowName },

    #[error("failed to delete versioned binary at {path}: {source}")]
    ReaperDeleteFailed {
        path: BinaryPath,
        source: std::io::Error,
    },

    #[error("BinaryPath must be absolute, got: {path}")]
    NonAbsolutePath { path: String },

    #[error("workflow definition validation failed for '{workflow_name}': {reason}")]
    WorkflowDefinitionInvalid {
        workflow_name: WorkflowName,
        reason: String,
    },
}

impl PartialEq for BinaryRegistryError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BinaryNotFound { path: a }, Self::BinaryNotFound { path: b }) => a == b,
            (Self::NotExecutable { path: a }, Self::NotExecutable { path: b }) => a == b,
            (
                Self::HashFailed {
                    path: a,
                    source: sa,
                },
                Self::HashFailed {
                    path: b,
                    source: sb,
                },
            ) => a == b && sa.kind() == sb.kind(),
            (
                Self::CopyFailed {
                    src: a,
                    dst: da,
                    source: sa,
                },
                Self::CopyFailed {
                    src: b,
                    dst: db,
                    source: sb,
                },
            ) => a == b && da == db && sa.kind() == sb.kind(),
            (
                Self::GraphDiscoveryFailed {
                    workflow_name: a,
                    exit_code: ea,
                    stderr: sa,
                },
                Self::GraphDiscoveryFailed {
                    workflow_name: b,
                    exit_code: eb,
                    stderr: sb,
                },
            ) => a == b && ea == eb && sa == sb,
            (
                Self::InvalidGraphOutput {
                    workflow_name: a,
                    parse_error: sa,
                },
                Self::InvalidGraphOutput {
                    workflow_name: b,
                    parse_error: sb,
                },
            ) => a == b && sa == sb,
            (
                Self::WorkflowDeactivated { workflow_name: a },
                Self::WorkflowDeactivated { workflow_name: b },
            ) => a == b,
            (Self::NotFound { workflow_name: a }, Self::NotFound { workflow_name: b }) => a == b,
            (
                Self::ReaperDeleteFailed {
                    path: a,
                    source: sa,
                },
                Self::ReaperDeleteFailed {
                    path: b,
                    source: sb,
                },
            ) => a == b && sa.kind() == sb.kind(),
            (Self::NonAbsolutePath { path: a }, Self::NonAbsolutePath { path: b }) => a == b,
            (
                Self::WorkflowDefinitionInvalid {
                    workflow_name: a,
                    reason: ra,
                },
                Self::WorkflowDefinitionInvalid {
                    workflow_name: b,
                    reason: rb,
                },
            ) => a == b && ra == rb,
            _ => false,
        }
    }
}
