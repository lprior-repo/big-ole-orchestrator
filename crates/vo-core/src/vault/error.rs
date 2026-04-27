use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use vo_types::credentials::{CredentialId, CredentialVersionId, Permission, Principal};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CredentialError {
    #[error("credential not found: {0}")]
    CredentialNotFound(CredentialId),

    #[error("credential already exists: {0}")]
    CredentialAlreadyExists(CredentialId),

    #[error("version {version_id} not found for credential {credential_id}")]
    VersionNotFound {
        credential_id: CredentialId,
        version_id: CredentialVersionId,
    },

    #[error("credential {credential_id} is {current_status}, required {required_status:?} for {operation}")]
    InvalidCredentialState {
        credential_id: CredentialId,
        current_status: vo_types::credentials::CredentialStatus,
        required_status: Vec<vo_types::credentials::CredentialStatus>,
        operation: String,
    },

    #[error("rotation failed for {credential_id}: {reason}")]
    RotationFailed {
        credential_id: CredentialId,
        reason: RotationFailureReason,
        retry_after: Option<vo_types::DurationMs>,
    },

    #[error("access denied for {principal} on {credential_id}: requires {required_permission}")]
    AccessDenied {
        principal: Principal,
        credential_id: CredentialId,
        required_permission: Permission,
    },

    #[error("credential {credential_id} version {version_id} expired at {expired_at}")]
    CredentialExpired {
        credential_id: CredentialId,
        version_id: CredentialVersionId,
        expired_at: vo_types::TimestampMs,
    },

    #[error("invalid {kind} format: {detail}")]
    InvalidCredentialFormat {
        kind: vo_types::credentials::CredentialKind,
        detail: String,
    },

    #[error("master key not found: {0}")]
    MasterKeyNotFound(u32),

    #[error("master key revoked: {0}")]
    MasterKeyRevoked(u32),

    #[error("vault storage error: {0}")]
    VaultStorageError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationFailureReason {
    GenerationError(String),
    StorageError(String),
    EncryptionError(String),
    DecryptionError(String),
    OverlapViolation,
    PolicyViolation,
}

impl fmt::Display for RotationFailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationError(reason) => write!(f, "GenerationError({reason})"),
            Self::StorageError(reason) => write!(f, "StorageError({reason})"),
            Self::EncryptionError(reason) => write!(f, "EncryptionError({reason})"),
            Self::DecryptionError(reason) => write!(f, "DecryptionError({reason})"),
            Self::OverlapViolation => write!(f, "OverlapViolation"),
            Self::PolicyViolation => write!(f, "PolicyViolation"),
        }
    }
}
