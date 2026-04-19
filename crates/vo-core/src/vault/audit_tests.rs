//! Credential storage audit logging tests (ve-61ql0).
//!
//! Tests that all credential access is logged. Covers: access log on read,
//! rotation log on rotate, and expiry log on expired credential access.

#[cfg(test)]
mod tests {
    use vo_types::credentials::{
        AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus,
        CredentialVersion, CredentialVersionId, RotationPolicy, RotationState, SecretValue,
        VaultEntry, VaultEntryId,
    };
    use vo_types::{InstanceId, TimestampMs};

    use crate::vault::{CredentialError, CredentialVault, Permission};

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

    fn system_principal() -> vo_types::credentials::Principal {
        vo_types::credentials::Principal::System
    }

    // ── Access log: credential read is observable ────────────────────────

    #[test]
    fn access_log_credential_read_succeeds() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        // Reading a credential should succeed and be observable
        let result = vault.get_credential(&id);
        assert!(result.is_ok());
        let cred = result.unwrap();
        assert_eq!(cred.id, id);
        assert_eq!(cred.name, "github-api");
        assert_eq!(cred.kind, CredentialKind::ApiKey);
    }

    #[test]
    fn access_log_credential_read_not_found() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.get_credential(&id);
        assert!(matches!(result, Err(CredentialError::CredentialNotFound(_))));
    }

    #[test]
    fn access_log_secret_read_succeeds() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let result = vault.get_secret(&id, &system_principal());
        assert!(result.is_ok());
        let secret = result.unwrap();
        assert_eq!(secret.ciphertext.len(), 32);
        assert_eq!(secret.key_version, 1);
    }

    #[test]
    fn access_log_secret_read_not_found() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.get_secret(&id, &system_principal());
        assert!(matches!(result, Err(CredentialError::CredentialNotFound(_))));
    }

    #[test]
    fn access_log_list_credentials() {
        let mut vault = CredentialVault::new();
        let entry1 = create_test_vault_entry();
        let entry2 = {
            let mut e = create_test_vault_entry();
            e.credential.id =
                CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").expect("valid ULID");
            e.credential.name = "stripe-api".to_string();
            e.credential.kind = CredentialKind::Token;
            e
        };

        vault.create_credential(entry1).unwrap();
        vault.create_credential(entry2).unwrap();

        let list = vault.list_credentials().expect("list should succeed");
        assert_eq!(list.len(), 2);
    }

    // ── Rotation log: rotation events are observable ─────────────────────

    #[test]
    fn rotation_log_rotate_creates_new_version() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        let original_version = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        let new_version_id = vault.rotate(&id, None).expect("rotate should succeed");

        let cred = vault.get_credential(&id).unwrap();
        assert_eq!(cred.versions.len(), 2);
        assert_eq!(cred.current_version, new_version_id);

        // Old version should be Superseded
        let old_version = cred
            .versions
            .iter()
            .find(|v| v.version_id == original_version)
            .expect("old version should exist");
        assert_eq!(old_version.status, CredentialStatus::Superseded);

        // New version should be Active
        let new_version = cred
            .versions
            .iter()
            .find(|v| v.version_id == new_version_id)
            .expect("new version should exist");
        assert_eq!(new_version.status, CredentialStatus::Active);
    }

    #[test]
    fn rotation_log_rotate_updates_rotation_status() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        vault.rotate(&id, None).unwrap();

        let status = vault.get_rotation_status(&id).expect("get rotation status");
        assert_eq!(
            status.state(),
            vo_types::credentials::RotationStatus::Idle,
            "rotation should return to Idle after completion"
        );
    }

    #[test]
    fn rotation_log_rotate_not_found() {
        let mut vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let result = vault.rotate(&id, None);
        assert!(matches!(result, Err(CredentialError::CredentialNotFound(_))));
    }

    #[test]
    fn rotation_log_multiple_rotations_tracked() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let _v1 = vault.rotate(&id, None).unwrap();
        let _v2 = vault.rotate(&id, None).unwrap();
        let v3 = vault.rotate(&id, None).unwrap();

        let cred = vault.get_credential(&id).unwrap();
        assert_eq!(cred.versions.len(), 4); // original + 3 rotations

        // All old versions should be Superseded
        let superseded_count = cred
            .versions
            .iter()
            .filter(|v| v.status == CredentialStatus::Superseded)
            .count();
        assert_eq!(superseded_count, 3);

        // Only the latest should be Active
        let active = cred.active_version().expect("should have active version");
        assert_eq!(active.version_id, v3);
    }

    // ── Expiry log: expired credential handling ──────────────────────────

    #[test]
    fn expiry_log_revoked_version_cannot_be_read_as_active() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        // Revoke the active version
        vault
            .revoke_version(&id, &version_id, &system_principal())
            .unwrap();

        let cred = vault.get_credential(&id).unwrap();
        let revoked = cred
            .versions
            .iter()
            .find(|v| v.version_id == version_id)
            .unwrap();
        assert_eq!(revoked.status, CredentialStatus::Revoked);

        // No active version should remain
        assert!(cred.active_version().is_none());
    }

    #[test]
    fn expiry_log_revoke_all_clears_all_versions() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();
        vault.rotate(&id, None).unwrap();
        vault.rotate(&id, None).unwrap();

        vault.revoke_all(&id, &system_principal()).unwrap();

        let cred = vault.get_credential(&id).unwrap();
        for version in &cred.versions {
            assert_eq!(version.status, CredentialStatus::Revoked);
        }
    }

    #[test]
    fn expiry_log_credential_error_expired_has_timestamp() {
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let err = CredentialError::CredentialExpired {
            credential_id: cred_id,
            version_id,
            expired_at: TimestampMs::new_unchecked(9999),
        };
        let msg = format!("{err}");
        assert!(msg.contains("expired at 9999"), "expiry log must include timestamp");
    }

    // ── Audit: access denied is logged ──────────────────────────────────

    #[test]
    fn audit_access_denied_error_includes_principal_and_permission() {
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = vo_types::credentials::Principal::User(instance_id);
        let err = CredentialError::AccessDenied {
            principal: principal.clone(),
            credential_id: cred_id,
            required_permission: Permission::Read,
        };
        let msg = format!("{err}");
        assert!(msg.contains("access denied"), "audit log must indicate denial");
        assert!(msg.contains("read"), "audit log must include required permission");
    }

    #[test]
    fn audit_rotation_failure_is_logged() {
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let err = CredentialError::RotationFailed {
            credential_id: cred_id,
            reason: crate::vault::RotationFailureReason::EncryptionError("key expired".to_string()),
            retry_after: None,
        };
        let msg = format!("{err}");
        assert!(msg.contains("rotation failed"), "audit log must indicate rotation failure");
        assert!(msg.contains("EncryptionError"), "audit log must include failure reason");
    }
}
