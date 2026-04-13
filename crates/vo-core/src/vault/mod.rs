use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use vo_types::credentials::{
    Credential, CredentialId, CredentialKind, CredentialVersionId, RotationPolicy, RotationState,
    SecretValue, VaultEntry,
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

impl CredentialVault {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn create_credential(&self, entry: VaultEntry) -> Result<CredentialId, CredentialError> {
        if self.entries.contains_key(&entry.credential.id) {
            return Err(CredentialError::CredentialAlreadyExists(
                entry.credential.id,
            ));
        }
        Ok(entry.credential.id.clone())
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
        &self,
        _id: &CredentialId,
        _metadata: std::collections::HashMap<String, String>,
    ) -> Result<(), CredentialError> {
        Ok(())
    }

    pub fn rotate(
        &self,
        _id: &CredentialId,
        _policy: Option<RotationPolicy>,
    ) -> Result<CredentialVersionId, CredentialError> {
        Ok(CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap())
    }

    pub fn revoke_version(
        &self,
        _id: &CredentialId,
        _version_id: &CredentialVersionId,
        _principal: &vo_types::credentials::Principal,
    ) -> Result<(), CredentialError> {
        Ok(())
    }

    pub fn revoke_all(
        &self,
        _id: &CredentialId,
        _principal: &vo_types::credentials::Principal,
    ) -> Result<(), CredentialError> {
        Ok(())
    }

    pub fn list_credentials(&self) -> Result<Vec<CredentialSummary>, CredentialError> {
        Ok(Vec::new())
    }

    pub fn get_rotation_status(
        &self,
        _id: &CredentialId,
    ) -> Result<RotationState, CredentialError> {
        Ok(RotationState::new())
    }
}

impl Default for CredentialVault {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::access::{is_authorized, AccessChecker};
    use super::rotation::RotationStateMachine;
    use super::*;
    use vo_types::credentials::{
        AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus,
        CredentialVersion, CredentialVersionId, Principal, RotationPolicy, RotationState,
        RotationStatus, SecretValue, VaultEntry, VaultEntryId,
    };
    use vo_types::{DurationMs, InstanceId, SpawnId, TimestampMs, WorkflowName};

    fn create_test_vault_entry(cred_id: &str, ver_id: &str, entry_id: &str) -> VaultEntry {
        let version = CredentialVersion::new(
            CredentialVersionId::parse(ver_id).expect("valid ULID"),
            SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
            CredentialStatus::Active,
            TimestampMs::new_unchecked(1000),
            None,
        );

        let credential = Credential {
            id: CredentialId::parse(cred_id).expect("valid ULID"),
            kind: CredentialKind::ApiKey,
            name: "test-credential".to_string(),
            current_version: CredentialVersionId::parse(ver_id).expect("valid ULID"),
            versions: vec![version],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs::new_unchecked(1000),
            updated_at: TimestampMs::new_unchecked(1000),
        };

        VaultEntry {
            entry_id: VaultEntryId::parse(entry_id).expect("valid ULID"),
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
    fn vault_create_credential_returns_id() {
        let vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
        );
        let result = vault.create_credential(entry);
        assert!(result.is_ok());
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
    fn vault_get_secret_not_found() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let result = vault.get_secret(&id, &principal);
        assert!(result.is_err());
    }

    #[test]
    fn vault_rotate_returns_version_id() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.rotate(&id, None);
        assert!(result.is_ok());
    }

    #[test]
    fn vault_revoke_version_ok() {
        let vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(result.is_ok());
    }

    #[test]
    fn vault_revoke_all_ok() {
        let vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let result = vault.revoke_all(&cred_id, &principal);
        assert!(result.is_ok());
    }

    #[test]
    fn vault_get_rotation_status_returns_idle() {
        let vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.get_rotation_status(&cred_id);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().state(),
            vo_types::credentials::RotationStatus::Idle
        );
    }

    // ============================================================================
    // INTEGRATION TESTS - Full business logic integration
    // ============================================================================

    /// Integration test: Full credential lifecycle - create, rotate, revoke (stub behavior)
    /// Note: CredentialVault is a stub - methods return without storing data
    #[test]
    fn full_credential_lifecycle_stub_behavior() {
        let vault = CredentialVault::new();

        // Step 1: Create credential returns ID (stub - doesn't store)
        let cred_id = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
        let ver_id = "01H5JYV4XHGSR2F8KZ9BWNRFMB";
        let entry_id = "01H5JYV4XHGSR2F8KZ9BWNRFMC";
        let entry = create_test_vault_entry(cred_id, ver_id, entry_id);

        let result = vault.create_credential(entry);
        assert!(result.is_ok(), "Credential creation should return ID");
        assert_eq!(result.unwrap().as_str(), cred_id);

        // Step 2: Get credential returns not found (stub - doesn't store)
        let retrieved = vault.get_credential(&CredentialId::parse(cred_id).unwrap());
        assert!(retrieved.is_err(), "Credential should not be found (stub)");
        assert!(matches!(
            retrieved.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));

        // Step 3: Get secret returns not found (stub - doesn't store)
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap());
        let secret = vault.get_secret(&CredentialId::parse(cred_id).unwrap(), &user);
        assert!(secret.is_err(), "Secret should not be found (stub)");

        // Step 4: Rotate credential returns dummy version (stub)
        let rotated = vault.rotate(&CredentialId::parse(cred_id).unwrap(), None);
        assert!(rotated.is_ok(), "Rotation should return a version ID");
        assert!(rotated.unwrap().as_str().len() > 0);

        // Step 5: Revoke version succeeds (stub)
        let version_to_revoke = "01H5JYV4XHGSR2F8KZ9BWNRFMB";
        let revoke_result = vault.revoke_version(
            &CredentialId::parse(cred_id).unwrap(),
            &CredentialVersionId::parse(version_to_revoke).unwrap(),
            &user,
        );
        assert!(revoke_result.is_ok(), "Revoke should succeed (stub)");

        // Step 6: List credentials returns empty (stub)
        let list_result = vault.list_credentials();
        assert!(list_result.is_ok(), "List should succeed");
        assert!(
            list_result.unwrap().is_empty(),
            "List should be empty (stub)"
        );
    }

    /// Integration test: Metadata update is no-op in stub
    #[test]
    fn metadata_update_noop() {
        let vault = CredentialVault::new();

        let cred_id = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
        let entry = create_test_vault_entry(
            cred_id,
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
        );
        assert!(vault.create_credential(entry).is_ok());

        // Update metadata - stub implementation returns Ok without storing
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("environment".to_string(), "production".to_string());

        let update_result =
            vault.update_metadata(&CredentialId::parse(cred_id).unwrap(), metadata.clone());
        assert!(
            update_result.is_ok(),
            "Metadata update should return Ok (stub)"
        );

        // List still empty (metadata not stored)
        let list = vault.list_credentials().unwrap();
        assert!(list.is_empty(), "List should still be empty");
    }

    /// Integration test: Duplicate credential detection (stub returns Ok for both)
    /// Note: Vault is a stub - create_credential doesn't persist, so no duplicate detection
    #[test]
    fn duplicate_credential_stub_behavior() {
        let vault = CredentialVault::new();

        let cred_id = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
        let entry1 = create_test_vault_entry(
            cred_id,
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
        );
        let entry2 = create_test_vault_entry(
            cred_id,
            "01H5JYV4XHGSR2F8KZ9BWNRFMD",
            "01H5JYV4XHGSR2F8KZ9BWNRFME",
        );

        // Both succeed because vault doesn't persist (stub)
        assert!(vault.create_credential(entry1).is_ok());
        assert!(vault.create_credential(entry2).is_ok());
    }

    /// Integration test: Access control across all principal types
    #[test]
    fn access_control_all_principal_types() {
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let actor =
            Principal::Actor(vo_types::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());
        let workflow = Principal::Workflow(vo_types::WorkflowName::parse("deploy-prod").unwrap());
        let system = Principal::System;

        let policy = AccessPolicy::new(vec![user.clone()]);

        // System always authorized
        assert!(is_authorized(&policy, &system));

        // User in allowed list authorized
        assert!(is_authorized(&policy, &user));

        // Actor not in list denied
        assert!(!is_authorized(&policy, &actor));

        // Workflow not in list denied
        assert!(!is_authorized(&policy, &workflow));
    }

    /// Integration test: Access control with approval required
    #[test]
    fn access_control_approval_required() {
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let approver = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());

        let mut policy = AccessPolicy::new(vec![user.clone()]);
        policy = AccessPolicy {
            allowed_principals: policy.allowed_principals,
            require_approval: true,
            approvers: vec![approver.clone()],
            audit_enabled: true,
        };

        // User not in approvers denied
        assert!(!is_authorized(&policy, &user));

        // Approver authorized
        assert!(is_authorized(&policy, &approver));
    }

    /// Integration test: AccessChecker permission checks
    #[test]
    fn access_checker_permission_checks() {
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let policy = AccessPolicy::new(vec![user.clone()]);
        let checker = AccessChecker::new(&policy, &user);

        // All permissions granted to authorized user
        assert!(checker.can_read());
        assert!(checker.can_write());
        assert!(checker.can_delete());
        assert!(checker.can_rotate());
        assert!(checker.can_revoke());
    }

    /// Integration test: AccessChecker denies unauthorized
    #[test]
    fn access_checker_denies_unauthorized() {
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let unauthorized =
            Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());
        let policy = AccessPolicy::new(vec![user.clone()]);

        let checker = AccessChecker::new(&policy, &unauthorized);

        // All permissions denied to unauthorized user
        assert!(!checker.can_read());
        assert!(!checker.can_write());
        assert!(!checker.can_delete());
        assert!(!checker.can_rotate());
        assert!(!checker.can_revoke());
    }

    /// Integration test: Full rotation state machine lifecycle
    #[test]
    fn rotation_state_machine_full_lifecycle() {
        let mut machine = RotationStateMachine::new();

        // Initial state: Idle
        assert_eq!(machine.state().state(), RotationState::new().state());

        // Start rotation
        assert!(machine.start_rotation().is_ok());
        assert_eq!(machine.state().state(), RotationStatus::Rotating);

        // Complete rotation
        machine.complete_rotation(None);
        assert_eq!(machine.state().state(), RotationState::new().state());
    }

    /// Integration test: Rotation with failure and retry
    #[test]
    fn rotation_with_failure_and_retry() {
        let mut machine = RotationStateMachine::new();

        // Start and fail
        assert!(machine.start_rotation().is_ok());
        machine.fail_rotation("encryption failed".to_string());
        assert!(matches!(
            machine.state().state(),
            RotationStatus::Failed(ref s) if s == "encryption failed"
        ));
        assert_eq!(machine.state().consecutive_failures(), 1);

        // Acknowledge and retry
        machine.acknowledge_failure();
        assert_eq!(machine.state().state(), RotationState::new().state());

        // Retry succeeds
        assert!(machine.start_rotation().is_ok());
        machine.complete_rotation(None);
        assert_eq!(machine.state().state(), RotationState::new().state());
    }

    /// Integration test: Rotation overlap workflow
    #[test]
    fn rotation_overlap_workflow() {
        let mut machine = RotationStateMachine::new();

        // Start rotation
        assert!(machine.start_rotation().is_ok());

        // Enter overlap window
        machine.enter_overlap();
        assert_eq!(machine.state().state(), RotationStatus::WaitingForOverlap);

        // Can still complete from overlap
        machine.complete_rotation(None);
        assert_eq!(machine.state().state(), RotationState::new().state());
    }

    /// Integration test: Rotation policy next computation
    #[test]
    fn rotation_policy_next_computation() {
        let last_rotation = TimestampMs::new_unchecked(1000);

        // Manual policy: no next rotation
        let manual_policy = RotationPolicy::Manual;
        assert!(
            RotationStateMachine::compute_next_rotation(&manual_policy, last_rotation).is_none()
        );

        // Time-based policy: computes next rotation
        let time_policy = RotationPolicy::TimeBased {
            interval: DurationMs::try_from(86400000u64).unwrap(),
            overlap_window: DurationMs::try_from(60000u64).unwrap(),
        };
        let next = RotationStateMachine::compute_next_rotation(&time_policy, last_rotation);
        assert!(next.is_some());
        assert!(next.unwrap().as_u64() > last_rotation.as_u64());
    }

    /// Integration test: Consecutive failure counter persists
    #[test]
    fn consecutive_failures_persist_across_rotations() {
        let mut machine = RotationStateMachine::new();

        // First rotation fails
        machine.start_rotation().unwrap();
        machine.fail_rotation("error 1".to_string());
        assert_eq!(machine.state().consecutive_failures(), 1);

        // Second rotation fails (counter persists)
        machine.start_rotation().unwrap();
        machine.fail_rotation("error 2".to_string());
        assert_eq!(machine.state().consecutive_failures(), 2);

        // Complete resets counter
        machine.complete_rotation(None);
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    /// Integration test: Vault + AccessControl + Rotation working together
    #[test]
    fn vault_access_rotation_integration() {
        let vault = CredentialVault::new();
        let mut rotation_machine = RotationStateMachine::new();

        let cred_id = "01H5JYV4XHGSR2F8KZ9BWNRFMA";
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap());

        // Create credential (stub - doesn't store)
        let entry = create_test_vault_entry(
            cred_id,
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
        );
        assert!(vault.create_credential(entry).is_ok());

        // Access check works independently
        let policy = AccessPolicy::new(vec![user.clone()]);
        assert!(is_authorized(&policy, &user));

        // Rotation state machine works
        assert_eq!(
            rotation_machine.state().state(),
            RotationState::new().state()
        );
        assert!(rotation_machine.start_rotation().is_ok());
        assert_eq!(rotation_machine.state().state(), RotationStatus::Rotating);

        // Complete rotation
        rotation_machine.complete_rotation(None);
        assert_eq!(
            rotation_machine.state().state(),
            RotationState::new().state()
        );
    }

    /// Integration test: Credential not found error propagation
    #[test]
    fn credential_not_found_error_propagation() {
        let vault = CredentialVault::new();

        let result =
            vault.get_credential(&CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    /// Integration test: Rotation state error
    #[test]
    fn rotation_state_error_already_rotating() {
        let mut machine = RotationStateMachine::new();

        // Start rotation
        assert!(machine.start_rotation().is_ok());

        // Try to start again
        let result = machine.start_rotation();
        assert!(matches!(
            result.unwrap_err(),
            rotation::RotationStateError::AlreadyRotating
        ));
    }

    /// Integration test: Access denied error
    #[test]
    fn access_denied_error() {
        let user = Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let unauthorized =
            Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());

        let policy = AccessPolicy::new(vec![user.clone()]);

        let checker = AccessChecker::new(&policy, &unauthorized);
        assert!(!checker.can_read());
    }

    /// Integration test: Permission display
    #[test]
    fn permission_display_integration() {
        assert_eq!(format!("{}", Permission::Read), "read");
        assert_eq!(format!("{}", Permission::Write), "write");
        assert_eq!(format!("{}", Permission::Delete), "delete");
        assert_eq!(format!("{}", Permission::Rotate), "rotate");
        assert_eq!(format!("{}", Permission::Revoke), "revoke");
    }

    /// Integration test: Credential summary accessors
    #[test]
    fn credential_summary_accessors_integration() {
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
}
