use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: vo_types::credentials::CredentialId,
    pub name: String,
    pub kind: vo_types::credentials::CredentialKind,
    pub version_count: usize,
    pub rotation_status: vo_types::credentials::RotationStatus,
}

impl CredentialSummary {
    #[must_use]
    pub fn id(&self) -> vo_types::credentials::CredentialId {
        self.id.clone()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn kind(&self) -> vo_types::credentials::CredentialKind {
        self.kind.clone()
    }

    #[must_use]
    pub fn version_count(&self) -> usize {
        self.version_count
    }

    #[must_use]
    pub fn rotation_status(&self) -> vo_types::credentials::RotationStatus {
        self.rotation_status.clone()
    }
}
