//! Integration tests for the vault module.
//!
//! Tests the full credential lifecycle: create, read, update, rotate, revoke, list.
//! These tests exercise the CredentialVault with real data structures.

use vo_core::vault::{CredentialError, CredentialSummary, CredentialVault, Permission};
use vo_types::credentials::{
    AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus, CredentialVersion,
    CredentialVersionId, Principal, RotationPolicy, RotationState, SecretValue, VaultEntry,
    VaultEntryId,
};
use vo_types::{InstanceId, TimestampMs};

fn create_test_vault_entry(
    credential_id: &str,
    version_id: &str,
    entry_id: &str,
    name: &str,
) -> VaultEntry {
    let cred_id = CredentialId::parse(credential_id).expect("valid credential ID");
    let ver_id = CredentialVersionId::parse(version_id).expect("valid version ID");
    let ent_id = VaultEntryId::parse(entry_id).expect("valid entry ID");

    let version = CredentialVersion::new(
        ver_id,
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs::new_unchecked(1000),
        None,
    );

    let credential = Credential {
        id: cred_id,
        kind: CredentialKind::ApiKey,
        name: name.to_string(),
        current_version: ver_id,
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };

    VaultEntry {
        entry_id: ent_id,
        credential,
        access_policy: AccessPolicy::new(vec![]),
        rotation_state: RotationState::new(),
    }
}

fn make_principal() -> Principal {
    Principal::User(InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap())
}

mod lifecycle {
    use super::*;

    #[test]
    fn create_and_get_credential() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let cred = vault.get_credential(&id).expect("get should succeed");
        assert_eq!(cred.name, "github-api");
        assert_eq!(cred.kind, CredentialKind::ApiKey);
    }

    #[test]
    fn create_and_list_single_credential() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let list = vault.list_credentials().expect("list should succeed");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "github-api");
    }

    #[test]
    fn create_and_get_secret() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();
        let principal = make_principal();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let secret = vault
            .get_secret(&id, &principal)
            .expect("get_secret should succeed");
        assert_eq!(secret.ciphertext.len(), 32);
    }

    #[test]
    fn update_metadata() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let mut new_meta = std::collections::HashMap::new();
        new_meta.insert("env".to_string(), "production".to_string());
        new_meta.insert("owner".to_string(), "platform".to_string());

        vault
            .update_metadata(&id, new_meta)
            .expect("update should succeed");

        let cred = vault.get_credential(&id).expect("get should succeed");
        assert_eq!(cred.metadata.get("env").unwrap(), "production");
        assert_eq!(cred.metadata.get("owner").unwrap(), "platform");
    }

    #[test]
    fn rotate_credential() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let new_version = vault.rotate(&id, None).expect("rotate should succeed");

        let cred = vault.get_credential(&id).expect("get should succeed");
        assert_eq!(cred.versions.len(), 2);
        assert_eq!(cred.current_version, new_version);

        let active_count = cred
            .versions
            .iter()
            .filter(|v| v.status == CredentialStatus::Active)
            .count();
        assert_eq!(active_count, 1);

        let superseded_count = cred
            .versions
            .iter()
            .filter(|v| v.status == CredentialStatus::Superseded)
            .count();
        assert_eq!(superseded_count, 1);
    }

    #[test]
    fn multiple_rotations() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let v1 = vault
            .rotate(&id, None)
            .expect("first rotate should succeed");
        let v2 = vault
            .rotate(&id, None)
            .expect("second rotate should succeed");
        let v3 = vault
            .rotate(&id, None)
            .expect("third rotate should succeed");

        assert_ne!(v1, v2);
        assert_ne!(v2, v3);

        let cred = vault.get_credential(&id).expect("get should succeed");
        assert_eq!(cred.versions.len(), 4);
        assert_eq!(cred.current_version, v3);
    }

    #[test]
    fn revoke_version() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let cred_id = entry.credential.id.clone();
        let version_id = entry.credential.current_version.clone();
        let principal = make_principal();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        vault
            .revoke_version(&cred_id, &version_id, &principal)
            .expect("revoke should succeed");

        let cred = vault.get_credential(&cred_id).expect("get should succeed");
        let revoked = cred
            .versions
            .iter()
            .find(|v| v.version_id == version_id)
            .expect("version should exist");
        assert_eq!(revoked.status, CredentialStatus::Revoked);
    }

    #[test]
    fn revoke_all() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let cred_id = entry.credential.id.clone();
        let principal = make_principal();

        vault
            .create_credential(entry)
            .expect("create should succeed");
        vault.rotate(&cred_id, None).expect("rotate should succeed");

        vault
            .revoke_all(&cred_id, &principal)
            .expect("revoke_all should succeed");

        let cred = vault.get_credential(&cred_id).expect("get should succeed");
        for version in &cred.versions {
            assert_eq!(version.status, CredentialStatus::Revoked);
        }
    }

    #[test]
    fn list_multiple_credentials() {
        let mut vault = CredentialVault::new();

        let entry1 = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFC0",
            "github-api",
        );
        let entry2 = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMD",
            "01H5JYV4XHGSR2F8KZ9BWNRFC1",
            "stripe-key",
        );
        let entry3 = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "01H5JYV4XHGSR2F8KZ9BWNRFME",
            "01H5JYV4XHGSR2F8KZ9BWNRFC2",
            "aws-secret",
        );

        vault
            .create_credential(entry1)
            .expect("create should succeed");
        vault
            .create_credential(entry2)
            .expect("create should succeed");
        vault
            .create_credential(entry3)
            .expect("create should succeed");

        let list = vault.list_credentials().expect("list should succeed");
        assert_eq!(list.len(), 3);

        let names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"github-api"));
        assert!(names.contains(&"stripe-key"));
        assert!(names.contains(&"aws-secret"));
    }

    #[test]
    fn get_rotation_status() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let status = vault
            .get_rotation_status(&id)
            .expect("get_rotation_status should succeed");
        assert_eq!(status.state(), vo_types::credentials::RotationStatus::Idle);
    }
}

mod error_handling {
    use super::*;

    #[test]
    fn create_duplicate_fails() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );

        vault
            .create_credential(entry.clone())
            .expect("first create should succeed");
        let result = vault.create_credential(entry);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialAlreadyExists(_)
        ));
    }

    #[test]
    fn get_nonexistent_fails() {
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
    fn get_secret_nonexistent_fails() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let principal = make_principal();

        let result = vault.get_secret(&id, &principal);
        assert!(result.is_err());
    }

    #[test]
    fn rotate_nonexistent_fails() {
        let mut vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let result = vault.rotate(&id, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn revoke_nonexistent_fails() {
        let mut vault = CredentialVault::new();
        let cred_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let principal = make_principal();

        let result = vault.revoke_version(&cred_id, &version_id, &principal);
        assert!(result.is_err());
    }

    #[test]
    fn update_metadata_nonexistent_fails() {
        let mut vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let result = vault.update_metadata(&id, std::collections::HashMap::new());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            CredentialError::CredentialNotFound(_)
        ));
    }

    #[test]
    fn get_rotation_status_nonexistent_fails() {
        let vault = CredentialVault::new();
        let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

        let result = vault.get_rotation_status(&id);
        assert!(result.is_err());
    }
}

mod credential_summary {
    use super::*;

    #[test]
    fn summary_contains_correct_info() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );

        vault
            .create_credential(entry)
            .expect("create should succeed");

        let list = vault.list_credentials().expect("list should succeed");
        assert_eq!(list.len(), 1);

        let summary = &list[0];
        assert_eq!(summary.name, "github-api");
        assert_eq!(summary.kind, CredentialKind::ApiKey);
        assert_eq!(summary.version_count, 1);
    }

    #[test]
    fn summary_updates_after_rotation() {
        let mut vault = CredentialVault::new();
        let entry = create_test_vault_entry(
            "01H5JYV4XHGSR2F8KZ9BWNRFMA",
            "01H5JYV4XHGSR2F8KZ9BWNRFMB",
            "01H5JYV4XHGSR2F8KZ9BWNRFMC",
            "github-api",
        );
        let id = entry.credential.id.clone();

        vault
            .create_credential(entry)
            .expect("create should succeed");
        vault.rotate(&id, None).expect("rotate should succeed");

        let list = vault.list_credentials().expect("list should succeed");
        let summary = list.iter().find(|s| s.id == id).unwrap();
        assert_eq!(summary.version_count, 2);
    }
}
