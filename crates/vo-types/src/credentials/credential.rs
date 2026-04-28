use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::TimestampMs;

use super::ids::{CredentialId, CredentialVersionId};
use super::secret::SecretValue;
use super::types::{CredentialKind, CredentialStatus, RotationPolicy};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialVersion {
    pub version_id: CredentialVersionId,
    pub secret_value: SecretValue,
    pub status: CredentialStatus,
    pub created_at: TimestampMs,
    pub expires_at: Option<TimestampMs>,
    pub rotated_from: Option<CredentialVersionId>,
    pub rotated_to: Option<CredentialVersionId>,
}

impl CredentialVersion {
    #[must_use]
    pub fn new(
        version_id: CredentialVersionId,
        secret_value: SecretValue,
        status: CredentialStatus,
        created_at: TimestampMs,
        expires_at: Option<TimestampMs>,
    ) -> Self {
        Self {
            version_id,
            secret_value,
            status,
            created_at,
            expires_at,
            rotated_from: None,
            rotated_to: None,
        }
    }

    #[must_use]
    pub fn status(&self) -> CredentialStatus {
        self.status.clone()
    }

    #[must_use]
    pub fn created_at(&self) -> TimestampMs {
        self.created_at
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<TimestampMs> {
        self.expires_at
    }

    #[must_use]
    pub fn rotated_from(&self) -> Option<CredentialVersionId> {
        self.rotated_from.clone()
    }

    #[must_use]
    pub fn rotated_to(&self) -> Option<CredentialVersionId> {
        self.rotated_to.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub id: CredentialId,
    pub kind: CredentialKind,
    pub name: String,
    pub current_version: CredentialVersionId,
    pub versions: Vec<CredentialVersion>,
    pub rotation_policy: RotationPolicy,
    pub metadata: HashMap<String, String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl Credential {
    #[must_use]
    pub fn id(&self) -> CredentialId {
        self.id.clone()
    }

    #[must_use]
    pub fn kind(&self) -> CredentialKind {
        self.kind.clone()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn current_version(&self) -> CredentialVersionId {
        self.current_version.clone()
    }

    #[must_use]
    pub fn versions(&self) -> &[CredentialVersion] {
        &self.versions
    }

    #[must_use]
    pub fn rotation_policy(&self) -> RotationPolicy {
        self.rotation_policy.clone()
    }

    #[must_use]
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> TimestampMs {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> TimestampMs {
        self.updated_at
    }

    pub fn active_version(&self) -> Option<&CredentialVersion> {
        self.versions
            .iter()
            .find(|v| v.status == CredentialStatus::Active)
    }
}
