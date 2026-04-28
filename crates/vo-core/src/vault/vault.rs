use vo_types::credentials::{
    Credential, CredentialId, CredentialVersion, CredentialVersionId, RotationPolicy,
    RotationState, SecretValue, VaultEntry,
};

use crate::vault::error::CredentialError;
use crate::vault::types::CredentialSummary;

pub struct CredentialVault {
    entries: std::collections::HashMap<CredentialId, VaultEntry>,
}

fn generate_version_id() -> CredentialVersionId {
    let ulid = ulid::Ulid::new();
    CredentialVersionId::parse(&ulid.to_string())
        .expect("ULID generation always produces valid 26-char strings")
}

impl CredentialVault {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn create_credential(
        &mut self,
        entry: VaultEntry,
    ) -> Result<CredentialId, CredentialError> {
        if self.entries.contains_key(&entry.credential.id) {
            return Err(CredentialError::CredentialAlreadyExists(
                entry.credential.id,
            ));
        }
        let id = entry.credential.id.clone();
        self.entries.insert(id.clone(), entry);
        Ok(id)
    }

    pub fn get_credential(&self, id: &CredentialId) -> Result<Credential, CredentialError> {
        self.entries
            .get(id)
            .map(|e| e.credential.clone())
            .ok_or(CredentialError::CredentialNotFound(id.clone()))
    }

    pub fn get_secret(
        &self,
        id: &CredentialId,
        _principal: &vo_types::credentials::Principal,
    ) -> Result<SecretValue, CredentialError> {
        let entry = self
            .entries
            .get(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;

        let current = entry
            .credential
            .versions
            .iter()
            .find(|v| v.version_id == entry.credential.current_version)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;

        if current.status == vo_types::credentials::CredentialStatus::Revoked {
            return Err(CredentialError::MasterKeyRevoked(
                current.secret_value.key_version,
            ));
        }

        if let Some(expires_at) = current.expires_at {
            if expires_at <= vo_types::TimestampMs::now() {
                return Err(CredentialError::CredentialExpired {
                    credential_id: id.clone(),
                    version_id: current.version_id.clone(),
                    expired_at: expires_at,
                });
            }
        }

        let active = entry
            .credential
            .active_version()
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;
        Ok(active.secret_value.clone())
    }

    pub fn update_metadata(
        &mut self,
        id: &CredentialId,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<(), CredentialError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;
        entry.credential.metadata = metadata;
        entry.credential.updated_at = vo_types::TimestampMs::now();
        Ok(())
    }

    pub fn rotate(
        &mut self,
        id: &CredentialId,
        _policy: Option<RotationPolicy>,
    ) -> Result<CredentialVersionId, CredentialError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;

        let old_active = entry
            .credential
            .active_version()
            .map(|v| v.version_id.clone());

        let new_version_id = generate_version_id();

        for version in &mut entry.credential.versions {
            if version.status == vo_types::credentials::CredentialStatus::Active {
                version.status = vo_types::credentials::CredentialStatus::Superseded;
                version.rotated_to = Some(new_version_id.clone());
            }
        }

        let new_version = CredentialVersion::new(
            new_version_id.clone(),
            SecretValue::new(
                vec![0u8; 32],
                [0u8; 12],
                entry
                    .credential
                    .versions
                    .last()
                    .map(|v| v.secret_value.key_version + 1)
                    .unwrap_or(1),
            )
            .map_err(|e| CredentialError::VaultStorageError(e.to_string()))?,
            vo_types::credentials::CredentialStatus::Active,
            vo_types::TimestampMs::now(),
            None,
        );

        entry.credential.versions.push(CredentialVersion {
            rotated_from: old_active,
            ..new_version
        });
        entry.credential.current_version = new_version_id.clone();
        entry.credential.updated_at = vo_types::TimestampMs::now();

        Ok(new_version_id)
    }

    pub fn revoke_version(
        &mut self,
        id: &CredentialId,
        version_id: &CredentialVersionId,
        _principal: &vo_types::credentials::Principal,
    ) -> Result<(), CredentialError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;

        let version = entry
            .credential
            .versions
            .iter_mut()
            .find(|v| v.version_id == *version_id)
            .ok_or(CredentialError::VersionNotFound {
                credential_id: id.clone(),
                version_id: version_id.clone(),
            })?;

        version.status = vo_types::credentials::CredentialStatus::Revoked;
        entry.credential.updated_at = vo_types::TimestampMs::now();
        Ok(())
    }

    pub fn revoke_all(
        &mut self,
        id: &CredentialId,
        _principal: &vo_types::credentials::Principal,
    ) -> Result<(), CredentialError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;

        for version in &mut entry.credential.versions {
            version.status = vo_types::credentials::CredentialStatus::Revoked;
        }
        entry.credential.updated_at = vo_types::TimestampMs::now();
        Ok(())
    }

    pub fn list_credentials(&self) -> Result<Vec<CredentialSummary>, CredentialError> {
        let summaries = self
            .entries
            .values()
            .map(|entry| {
                let rotation_status = entry.rotation_state.state();
                CredentialSummary {
                    id: entry.credential.id.clone(),
                    name: entry.credential.name.clone(),
                    kind: entry.credential.kind.clone(),
                    version_count: entry.credential.versions.len(),
                    rotation_status,
                }
            })
            .collect();
        Ok(summaries)
    }

    pub fn get_rotation_status(&self, id: &CredentialId) -> Result<RotationState, CredentialError> {
        let entry = self
            .entries
            .get(id)
            .ok_or(CredentialError::CredentialNotFound(id.clone()))?;
        Ok(entry.rotation_state.clone())
    }
}

impl Default for CredentialVault {
    fn default() -> Self {
        Self::new()
    }
}
