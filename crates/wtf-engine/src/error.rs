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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn error_variants_are_correct() {
        let err1 = BinaryRegistryError::BinaryNotFound {
            path: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap(),
        };
        assert_eq!(
            err1,
            BinaryRegistryError::BinaryNotFound {
                path: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap()
            }
        );

        let err2 = BinaryRegistryError::NotExecutable {
            path: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap(),
        };
        assert_ne!(err1, err2);
        assert_eq!(err2.to_string(), "binary is not executable: /a");

        let err3 = BinaryRegistryError::HashFailed {
            path: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap(),
            source: io::Error::new(io::ErrorKind::NotFound, "err"),
        };
        assert_ne!(err2, err3);

        let err4 = BinaryRegistryError::CopyFailed {
            src: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap(),
            dst: BinaryPath::new(std::path::PathBuf::from("/b")).unwrap(),
            source: io::Error::new(io::ErrorKind::NotFound, "err"),
        };
        assert_ne!(err3, err4);

        let err5 = BinaryRegistryError::GraphDiscoveryFailed {
            workflow_name: WorkflowName("w".to_string()),
            exit_code: 1,
            stderr: "err".to_string(),
        };
        assert_ne!(err4, err5);

        let err6 = BinaryRegistryError::InvalidGraphOutput {
            workflow_name: WorkflowName("w".to_string()),
            parse_error: "err".to_string(),
        };
        assert_ne!(err5, err6);

        let err7 = BinaryRegistryError::WorkflowDeactivated {
            workflow_name: WorkflowName("w".to_string()),
        };
        assert_ne!(err6, err7);

        let err8 = BinaryRegistryError::NotFound {
            workflow_name: WorkflowName("w".to_string()),
        };
        assert_ne!(err7, err8);

        let err9 = BinaryRegistryError::ReaperDeleteFailed {
            path: BinaryPath::new(std::path::PathBuf::from("/a")).unwrap(),
            source: io::Error::new(io::ErrorKind::NotFound, "err"),
        };
        assert_ne!(err8, err9);

        let err10 = BinaryRegistryError::NonAbsolutePath {
            path: "a".to_string(),
        };
        assert_ne!(err9, err10);

        let err11 = BinaryRegistryError::WorkflowDefinitionInvalid {
            workflow_name: WorkflowName("w".to_string()),
            reason: "err".to_string(),
        };
        assert_ne!(err10, err11);
    }
}
