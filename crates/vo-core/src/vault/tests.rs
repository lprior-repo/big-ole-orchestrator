#[cfg(test)]
mod tests {
    
    use crate::vault::{
        CredentialError, CredentialSummary, CredentialVault, Permission, RotationFailureReason,
    };
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
            version_id,
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
            version_id,
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

    #[test]
    fn vault_get_secret_with_revoked_version_returns_master_key_revoked() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        vault
            .revoke_version(&cred_id, &version_id, &principal)
            .unwrap();

        let result = vault.get_secret(&cred_id, &principal);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::MasterKeyRevoked(_)
        ));
    }

    #[test]
    fn vault_get_secret_after_revoke_all_returns_master_key_revoked() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();
        vault.rotate(&cred_id, None).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
        );
        vault.revoke_all(&cred_id, &principal).unwrap();

        let result = vault.get_secret(&cred_id, &principal);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::MasterKeyRevoked(_)
        ));
    }

    #[test]
    fn vault_revoke_version_is_idempotent() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        vault.create_credential(entry).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        let first_result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(first_result.is_ok());

        let second_result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(second_result.is_ok(), "revoke_version should be idempotent");

        let cred = vault.get_credential(&cred_id).unwrap();
        let revoked = cred
            .versions
            .iter()
            .find(|v| v.version_id == version_id)
            .unwrap();
        assert_eq!(revoked.status, CredentialStatus::Revoked);
    }

    #[test]
    fn vault_revoke_version_does_not_affect_other_versions() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let new_version_id = vault.rotate(&cred_id, None).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        vault
            .revoke_version(&cred_id, &new_version_id, &principal)
            .unwrap();

        let cred = vault.get_credential(&cred_id).unwrap();
        let original_version = cred
            .versions
            .iter()
            .find(|v| v.status == CredentialStatus::Superseded)
            .expect("original version should still exist and be Superseded");
        assert_eq!(original_version.status, CredentialStatus::Superseded);

        let revoked_version = cred
            .versions
            .iter()
            .find(|v| v.version_id == new_version_id)
            .unwrap();
        assert_eq!(revoked_version.status, CredentialStatus::Revoked);
    }

    #[test]
    fn vault_revoked_data_remains_stored_for_recovery() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        let original_ciphertext = entry.credential.versions[0].secret_value.ciphertext.clone();
        vault.create_credential(entry).unwrap();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        vault
            .revoke_version(&cred_id, &version_id, &principal)
            .unwrap();

        let cred = vault.get_credential(&cred_id).unwrap();
        let revoked_version = cred
            .versions
            .iter()
            .find(|v| v.version_id == version_id)
            .unwrap();
        assert_eq!(
            revoked_version.secret_value.ciphertext, original_ciphertext,
            "encrypted data should remain stored even after revocation"
        );
    }

    #[test]
    fn vault_recovery_path_exists_for_revoked_key_data() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        let cred_id = entry.credential.id.clone();
        vault.create_credential(entry).unwrap();

        let cred_before_revoke = vault.get_credential(&cred_id).unwrap();
        let stored_ciphertext = cred_before_revoke.versions[0]
            .secret_value
            .ciphertext
            .clone();

        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        vault.revoke_all(&cred_id, &principal).unwrap();

        let cred_after_revoke = vault.get_credential(&cred_id).unwrap();
        assert_eq!(
            cred_after_revoke.versions[0].secret_value.ciphertext, stored_ciphertext,
            "ciphertext remains stored after revocation - data is recoverable at storage level"
        );
    }

    #[test]
    fn vault_revoke_version_on_nonexistent_version_returns_version_not_found() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry();
        vault.create_credential(entry).unwrap();

        let nonexistent_version_id =
            CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFZZ").unwrap();
        let principal = vo_types::credentials::Principal::User(
            InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        );
        let result = vault.revoke_version(
            &CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            &nonexistent_version_id,
            &principal,
        );
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::VersionNotFound { .. }
        ));
    }
}
