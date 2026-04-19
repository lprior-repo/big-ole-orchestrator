use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use vo_types::credentials::{
    Credential, CredentialId, CredentialKind, CredentialVersion, CredentialVersionId,
    RotationPolicy, RotationState, SecretValue, VaultEntry,
};

pub mod access;
pub mod rotation;

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
        principal: vo_types::credentials::Principal,
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
        kind: CredentialKind,
        detail: String,
    },

    #[error("master key not found: {0}")]
    MasterKeyNotFound(u32),

    #[error("master key revoked: {0}")]
    MasterKeyRevoked(u32),

    #[error("vault storage error: {0}")]
    VaultStorageError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    Read,
    Write,
    Delete,
    Rotate,
    Revoke,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Delete => write!(f, "delete"),
            Self::Rotate => write!(f, "rotate"),
            Self::Revoke => write!(f, "revoke"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSummary {
    pub id: CredentialId,
    pub name: String,
    pub kind: CredentialKind,
    pub version_count: usize,
    pub rotation_status: vo_types::credentials::RotationStatus,
}

impl CredentialSummary {
    #[must_use]
    pub fn id(&self) -> CredentialId {
        self.id.clone()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn kind(&self) -> CredentialKind {
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
                let rotation_status = match entry.rotation_state.state() {
                    vo_types::credentials::RotationStatus::Idle => {
                        vo_types::credentials::RotationStatus::Idle
                    }
                    other => other,
                };
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

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::credentials::{
        AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus,
        CredentialVersion, CredentialVersionId, RotationPolicy, RotationState, SecretValue,
        VaultEntry, VaultEntryId,
    };
    use vo_types::{InstanceId, TimestampMs};

    fn create_test_vault_entry() -> VaultEntry {
        let credential_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
        let version_id =
            CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").expect("valid ULID");
        let entry_id = VaultEntryId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").expect("valid ULID");

        let version = CredentialVersion::new(
            version_id.clone(),
            SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
            CredentialStatus::Active,
            TimestampMs::new_unchecked(1000),
            None,
        );

        let credential = Credential {
            id: credential_id.clone(),
            kind: CredentialKind::ApiKey,
            name: "github-api".to_string(),
            current_version: version_id.clone(),
            versions: vec![version],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs::new_unchecked(1000),
            updated_at: TimestampMs::new_unchecked(1000),
        };

        VaultEntry {
            entry_id,
            credential,
            access_policy: AccessPolicy::new(vec![]),
            rotation_state: RotationState::new(),
        }
    }

    #[test]
    fn credential_error_display_credential_not_found() {
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let err = CredentialError::CredentialNotFound(id.clone());
        assert_eq!(
            format!("{}", err),
            "credential not found: 01H5JYV4XHGSR2F8KZ9BWNRFMA"
        );
    }

    #[test]
    fn credential_error_display_version_not_found() {
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let err = CredentialError::VersionNotFound {
            credential_id: cred_id,
            version_id: version_id,
        };
        let msg = format!("{}", err);
        assert!(
            msg.contains("not found"),
            "error message should contain 'not found': {}",
            msg
        );
    }

    #[test]
    fn credential_error_display_rotation_failed() {
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let err = CredentialError::RotationFailed {
            credential_id: id,
            reason: RotationFailureReason::EncryptionError("key not found".to_string()),
            retry_after: None,
        };
        assert!(format!("{}", err).contains("rotation failed"));
        assert!(format!("{}", err).contains("EncryptionError"));
    }

    #[test]
    fn credential_error_display_access_denied() {
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let err = CredentialError::AccessDenied {
            principal,
            credential_id: id,
            required_permission: Permission::Read,
        };
        assert!(format!("{}", err).contains("access denied"));
    }

    #[test]
    fn credential_error_display_credential_expired() {
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let err = CredentialError::CredentialExpired {
            credential_id: cred_id,
            version_id: version_id,
            expired_at: TimestampMs::new_unchecked(1000),
        };
        assert!(format!("{}", err).contains("expired at"));
    }

    #[test]
    fn rotation_failure_reason_display_generation_error() {
        let reason = RotationFailureReason::GenerationError("RNG failed".to_string());
        assert_eq!(format!("{}", reason), "GenerationError(RNG failed)");
    }

    #[test]
    fn rotation_failure_reason_display_storage_error() {
        let reason = RotationFailureReason::StorageError("disk full".to_string());
        assert_eq!(format!("{}", reason), "StorageError(disk full)");
    }

    #[test]
    fn rotation_failure_reason_display_encryption_error() {
        let reason = RotationFailureReason::EncryptionError("key invalid".to_string());
        assert_eq!(format!("{}", reason), "EncryptionError(key invalid)");
    }

    #[test]
    fn rotation_failure_reason_display_overlap_violation() {
        let reason = RotationFailureReason::OverlapViolation;
        assert_eq!(format!("{}", reason), "OverlapViolation");
    }

    #[test]
    fn rotation_failure_reason_display_policy_violation() {
        let reason = RotationFailureReason::PolicyViolation;
        assert_eq!(format!("{}", reason), "PolicyViolation");
    }

    #[test]
    fn rotation_failure_reason_display_decryption_error() {
        let reason = RotationFailureReason::DecryptionError("key corrupted".to_string());
        assert_eq!(format!("{}", reason), "DecryptionError(key corrupted)");
    }

    #[test]
    fn permission_display_read() {
        assert_eq!(format!("{}", Permission::Read), "read");
    }

    #[test]
    fn permission_display_write() {
        assert_eq!(format!("{}", Permission::Write), "write");
    }

    #[test]
    fn permission_display_delete() {
        assert_eq!(format!("{}", Permission::Delete), "delete");
    }

    #[test]
    fn permission_display_rotate() {
        assert_eq!(format!("{}", Permission::Rotate), "rotate");
    }

    #[test]
    fn permission_display_revoke() {
        assert_eq!(format!("{}", Permission::Revoke), "revoke");
    }

    #[test]
    fn credential_summary_accessors() {
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let summary = CredentialSummary {
            id: id.clone(),
            name: "test".to_string(),
            kind: CredentialKind::ApiKey,
            version_count: 3,
            rotation_status: vo_types::credentials::RotationStatus::Idle,
        };
        assert_eq!(summary.id(), id);
        assert_eq!(summary.name(), "test");
        assert_eq!(summary.kind(), CredentialKind::ApiKey);
        assert_eq!(summary.version_count(), 3);
    }

    #[test]
    fn vault_new_is_empty() {
        let vault = CredentialVault::new();
        let result = vault.list_credentials();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn vault_create_credential_stores_and_lists() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        let result = vault.create_credential(entry);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);

        let list = vault.list_credentials().expect("list should succeed");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[test]
    fn vault_create_credential_duplicate_fails() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        vault.create_credential(entry.clone()).unwrap();
        let result = vault.create_credential(entry);
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialAlreadyExists(_)
        ));
    }

    #[test]
    fn vault_get_credential_not_found() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.get_credential(&id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn vault_get_credential_after_create() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();
        let cred = vault.get_credential(&id).expect("should find credential");
        assert_eq!(cred.id, id);
        assert_eq!(cred.name, "github-api");
    }

    #[test]
    fn vault_get_secret_not_found() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let result = vault.get_secret(&id, &principal);
        assert!(result.is_err());
    }

    #[test]
    fn vault_rotate_returns_unique_version_id() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let result = vault.rotate(&id, None);
        assert!(result.is_ok());

        let cred = vault.get_credential(&id).unwrap();
        assert_eq!(cred.versions.len(), 2);
        assert_eq!(cred.current_version, result.unwrap());

        let old_version = cred
            .versions
            .iter()
            .find(|v| v.status == CredentialStatus::Superseded)
            .expect("old version should be superseded");
        assert_eq!(old_version.status, CredentialStatus::Superseded);
    }

    #[test]
    fn vault_rotate_not_found() {
        let mut vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.rotate(&id, None);
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn vault_revoke_version_marks_revoked() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        let result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(result.is_ok());

        let cred = vault.get_credential(&cred_id).unwrap();
        let revoked = cred
            .versions
            .iter()
            .find(|v| v.version_id == version_id)
            .unwrap();
        assert_eq!(revoked.status, CredentialStatus::Revoked);
    }

    #[test]
    fn vault_revoke_version_not_found() {
        let mut vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        let result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn vault_revoke_all_marks_all_revoked() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();
        vault.rotate(&cred_id, None).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
        );
        let result = vault.revoke_all(&cred_id, &principal);
        assert!(result.is_ok());

        let cred = vault.get_credential(&cred_id).unwrap();
        for version in &cred.versions {
            assert_eq!(version.status, CredentialStatus::Revoked);
        }
    }

    #[test]
    fn vault_revoke_all_not_found() {
        let mut vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
        );
        let result = vault.revoke_all(&cred_id, &principal);
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn vault_get_rotation_status_returns_idle() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let result = vault.get_rotation_status(&cred_id);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().state(),
            vo_types::credentials::RotationStatus::Idle
        );
    }

    #[test]
    fn vault_update_metadata_updates_stored_entry() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let mut new_meta = std::collections::HashMap::new();
        new_meta.insert("env".to_string(), "production".to_string());
        vault.update_metadata(&id, new_meta).unwrap();

        let cred = vault.get_credential(&id).unwrap();
        assert_eq!(cred.metadata.get("env").unwrap(), "production");
    }

    #[test]
    fn vault_update_metadata_not_found() {
        let mut vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.update_metadata(&id, std::collections::HashMap::new());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn vault_rotate_generates_unique_ids() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let v1 = vault.rotate(&id, None).unwrap();
        let v2 = vault.rotate(&id, None).unwrap();
        assert_ne!(v1, v2, "each rotation should produce a unique version ID");
    }

    #[test]
    fn vault_rotate_tracks_rotated_from() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        let original_version = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        let new_version = vault.rotate(&id, None).unwrap();

        let cred = vault.get_credential(&id).unwrap();
        let new_entry = cred
            .versions
            .iter()
            .find(|v| v.version_id == new_version)
            .unwrap();
        assert_eq!(new_entry.rotated_from, Some(original_version));
    }
}
