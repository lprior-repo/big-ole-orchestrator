#[cfg(test)]
mod security_validation_tests {
    use super::*;
    use crate::{
        credentials::{
            AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus,
            CredentialVersion, CredentialVersionId, Principal, RotationPolicy, RotationState,
            RotationStatus, SecretValue, VaultEntry, VaultEntryId,
        },
        encryption::{CryptoAlgorithm, DekId, EncryptedBlob, KeyMetadata, WrappedDek},
        DurationMs, ParseError, TimestampMs,
    };

    #[test]
    fn test_credential_lifecycle_authentication() {
        let credential_id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let secret = SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid secret");

        let version = CredentialVersion::new(
            version_id.clone(),
            secret.clone(),
            CredentialStatus::Active,
            TimestampMs(1000),
            Some(TimestampMs(100000)),
        );

        let credential = Credential {
            id: credential_id.clone(),
            kind: CredentialKind::ApiKey,
            name: "test-api-key".to_string(),
            current_version: version_id.clone(),
            versions: vec![version.clone()],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        assert_eq!(credential.id(), credential_id);
        assert_eq!(credential.kind(), CredentialKind::ApiKey);
        assert_eq!(credential.name(), "test-api-key");
        assert_eq!(credential.current_version(), version_id);
        assert!(credential.active_version().is_some());
        assert_eq!(
            credential.active_version().unwrap().status(),
            CredentialStatus::Active
        );
    }

    #[test]
    fn test_credential_rotation_lifecycle() {
        let old_version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let new_version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let old_secret = SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid secret");
        let new_secret = SecretValue::new(vec![1u8; 32], [1u8; 12], 2).expect("valid secret");

        let old_version = CredentialVersion::new(
            old_version_id.clone(),
            old_secret,
            CredentialStatus::Superseded,
            TimestampMs(1000),
            Some(TimestampMs(50000)),
        );

        let new_version = CredentialVersion::new(
            new_version_id.clone(),
            new_secret,
            CredentialStatus::Active,
            TimestampMs(5000),
            Some(TimestampMs(100000)),
        );

        let credential = Credential {
            id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
            kind: CredentialKind::ApiKey,
            name: "rotating-key".to_string(),
            current_version: new_version_id.clone(),
            versions: vec![old_version, new_version],
            rotation_policy: RotationPolicy::TimeBased {
                interval: DurationMs(86400000),
                overlap_window: DurationMs(60000),
            },
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(5000),
        };

        let active = credential.active_version().unwrap();
        assert_eq!(active.status(), CredentialStatus::Active);
        assert_eq!(active.version_id, new_version_id);

        let superseded = credential
            .versions
            .iter()
            .find(|v| v.status == CredentialStatus::Superseded);
        assert!(superseded.is_some());
        assert_eq!(superseded.unwrap().version_id, old_version_id);
    }

    #[test]
    fn test_credential_rotation_policy_validation() {
        let time_based = RotationPolicy::TimeBased {
            interval: DurationMs(86400000),
            overlap_window: DurationMs(60000),
        };
        assert!(time_based.validate().is_ok());

        let usage_based = RotationPolicy::UsageBased {
            max_uses: 1000,
            overlap_window: DurationMs(60000),
        };
        assert!(usage_based.validate().is_ok());

        let event_based = RotationPolicy::EventBased {
            trigger_events: vec!["security.breach".to_string(), "key.expired".to_string()],
            overlap_window: DurationMs(60000),
        };
        assert!(event_based.validate().is_ok());

        let manual = RotationPolicy::Manual;
        assert!(manual.validate().is_ok());

        let invalid_time = RotationPolicy::TimeBased {
            interval: DurationMs(0),
            overlap_window: DurationMs(60000),
        };
        assert!(invalid_time.validate().is_err());

        let invalid_overlap = RotationPolicy::TimeBased {
            interval: DurationMs(86400000),
            overlap_window: DurationMs(59999),
        };
        assert!(invalid_overlap.validate().is_err());

        let invalid_usage = RotationPolicy::UsageBased {
            max_uses: 0,
            overlap_window: DurationMs(60000),
        };
        assert!(invalid_usage.validate().is_err());

        let invalid_event = RotationPolicy::EventBased {
            trigger_events: vec![],
            overlap_window: DurationMs(60000),
        };
        assert!(invalid_event.validate().is_err());
    }

    #[test]
    fn test_access_policy_enforcement() {
        let user_id = crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let actor_id = crate::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let workflow_name = crate::WorkflowName::parse("data_processor").unwrap();

        let policy = AccessPolicy {
            allowed_principals: vec![
                Principal::User(user_id.clone()),
                Principal::Actor(actor_id.clone()),
                Principal::Workflow(workflow_name.clone()),
            ],
            require_approval: true,
            approvers: vec![Principal::System],
            audit_enabled: true,
        };

        assert_eq!(policy.allowed_principals().len(), 3);
        assert!(policy.require_approval());
        assert!(policy.audit_enabled());

        let user_principal = &policy.allowed_principals()[0];
        assert!(matches!(user_principal, Principal::User(_)));

        let actor_principal = &policy.allowed_principals()[1];
        assert!(matches!(actor_principal, Principal::Actor(_)));

        let workflow_principal = &policy.allowed_principals()[2];
        assert!(matches!(workflow_principal, Principal::Workflow(_)));
    }

    #[test]
    fn test_access_policy_principal_matching() {
        let user_id = crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let policy = AccessPolicy::new(vec![Principal::User(user_id.clone())]);

        let matching_user = Principal::User(user_id.clone());
        let different_user =
            Principal::User(crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());
        let actor = Principal::Actor(crate::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap());
        let system = Principal::System;

        assert!(policy.allowed_principals().contains(&matching_user));
        assert!(!policy.allowed_principals().contains(&different_user));
        assert!(!policy.allowed_principals().contains(&actor));
        assert!(!policy.allowed_principals().contains(&system));
    }

    #[test]
    fn test_vault_entry_structure() {
        let credential = Credential {
            id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            kind: CredentialKind::Password,
            name: "db-password".to_string(),
            current_version: CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
            versions: vec![CredentialVersion::new(
                CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
                SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
                CredentialStatus::Active,
                TimestampMs(1000),
                None,
            )],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        let user_id = crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();
        let access_policy = AccessPolicy::new(vec![Principal::User(user_id)]);

        let vault_entry = VaultEntry {
            entry_id: VaultEntryId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMD").unwrap(),
            credential,
            access_policy,
            rotation_state: RotationState::new(),
        };

        assert_eq!(
            vault_entry.entry_id().as_str(),
            "01H5JYV4XHGSR2F8KZ9BWNRFMD"
        );
        assert_eq!(vault_entry.credential().name(), "db-password");
        assert!(vault_entry.access_policy().audit_enabled());
        assert_eq!(vault_entry.rotation_state().state(), RotationStatus::Idle);
    }

    #[test]
    fn test_rotation_state_transitions() {
        let mut state = RotationState::new();
        assert_eq!(state.state(), RotationStatus::Idle);
        assert_eq!(state.consecutive_failures(), 0);

        state = RotationState {
            state: RotationStatus::Rotating,
            last_rotation: Some(TimestampMs(1000)),
            next_scheduled_rotation: Some(TimestampMs(100000)),
            consecutive_failures: 0,
            last_failure_reason: None,
        };

        assert_eq!(state.state(), RotationStatus::Rotating);
        assert!(state.last_rotation().is_some());
        assert!(state.next_scheduled_rotation().is_some());
    }

    #[test]
    fn test_encrypted_blob_structure() {
        let iv = vec![0u8; 12];
        let ciphertext = vec![1u8; 64];
        let tag = vec![2u8; 16];

        let blob = EncryptedBlob::new(iv.clone(), ciphertext.clone(), tag.clone()).unwrap();

        assert_eq!(blob.iv, iv);
        assert_eq!(blob.ciphertext, ciphertext);
        assert_eq!(blob.tag, tag);
        assert_eq!(blob.total_size(), 92);

        let display = format!("{}", blob);
        assert!(display.contains("iv=12"));
        assert!(display.contains("ciphertext=64"));
        assert!(display.contains("tag=16"));
    }

    #[test]
    fn test_encryption_algorithm_constants() {
        assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
        assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
        assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);

        let algorithm = CryptoAlgorithm::Aes256Gcm;
        let display = format!("{}", algorithm);
        assert_eq!(display, "AES-256-GCM");
    }

    #[test]
    fn test_wrapped_dek_storage() {
        let wrapped_bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33];
        let wrapped_dek = WrappedDek::new(wrapped_bytes.clone());

        assert_eq!(wrapped_dek.as_bytes(), &wrapped_bytes);
        assert_eq!(wrapped_dek.as_bytes().len(), 8);

        let display = format!("{}", wrapped_dek);
        assert!(display.contains("WrappedDek(8 bytes)"));
    }

    #[test]
    fn test_dek_id_validation() {
        let valid_ulid = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        assert_eq!(valid_ulid.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");

        let nil_ulid = DekId::parse("00000000000000000000000000");
        assert!(nil_ulid.is_err());

        let invalid_ulid = DekId::parse("not-a-valid-ulid");
        assert!(invalid_ulid.is_err());

        let too_short = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRF");
        assert!(too_short.is_err());

        let too_long = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMAAA");
        assert!(too_long.is_err());
    }

    #[test]
    fn test_dek_id_roundtrip() {
        let original = DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let bytes = original.to_bytes().expect("valid bytes");
        let reconstructed = DekId::from_bytes(bytes);

        assert_eq!(original.as_str(), reconstructed.as_str());
    }

    #[test]
    fn test_secret_value_security_invariants() {
        let valid_secret = SecretValue::new(vec![0u8; 32], [0u8; 12], 1);
        assert!(valid_secret.is_ok());

        let empty_secret = SecretValue::new(vec![], [0u8; 12], 1);
        assert!(empty_secret.is_err());

        if let Err(ParseError::Empty { type_name }) = empty_secret {
            assert_eq!(type_name, "SecretValue");
        } else {
            panic!("Expected ParseError::Empty");
        }

        let secret = valid_secret.unwrap();
        assert!(!secret.ciphertext().is_empty());
        assert_eq!(secret.nonce(), [0u8; 12]);
        assert_eq!(secret.key_version(), 1);
    }

    #[test]
    fn test_principal_display_formats() {
        let instance_id = crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let spawn_id = crate::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let workflow_name = crate::WorkflowName::parse("data_processor").unwrap();

        let user = Principal::User(instance_id.clone());
        let actor = Principal::Actor(spawn_id.clone());
        let workflow = Principal::Workflow(workflow_name.clone());
        let system = Principal::System;

        assert_eq!(format!("{}", user), format!("User({})", instance_id));
        assert_eq!(format!("{}", actor), format!("Actor({})", spawn_id));
        assert_eq!(
            format!("{}", workflow),
            format!("Workflow({})", workflow_name)
        );
        assert_eq!(format!("{}", system), "System");
    }

    #[test]
    fn test_credential_kind_variants() {
        let variants = CredentialKind::all_variants();
        assert_eq!(variants.len(), 6);

        assert_eq!(format!("{}", CredentialKind::ApiKey), "ApiKey");
        assert_eq!(format!("{}", CredentialKind::Password), "Password");
        assert_eq!(format!("{}", CredentialKind::Token), "Token");
        assert_eq!(format!("{}", CredentialKind::Certificate), "Certificate");
        assert_eq!(format!("{}", CredentialKind::SigningKey), "SigningKey");
        assert_eq!(
            format!("{}", CredentialKind::EncryptionKey),
            "EncryptionKey"
        );

        let custom = CredentialKind::Custom("my-custom-type".to_string());
        assert_eq!(format!("{}", custom), "Custom(my-custom-type)");
    }

    #[test]
    fn test_credential_status_lifecycle() {
        assert!(!CredentialStatus::Active.is_terminal());
        assert!(!CredentialStatus::Rotating.is_terminal());
        assert!(CredentialStatus::Expired.is_terminal());
        assert!(CredentialStatus::Revoked.is_terminal());
        assert!(CredentialStatus::Superseded.is_terminal());

        assert_eq!(format!("{}", CredentialStatus::Active), "Active");
        assert_eq!(format!("{}", CredentialStatus::Rotating), "Rotating");
        assert_eq!(format!("{}", CredentialStatus::Expired), "Expired");
        assert_eq!(format!("{}", CredentialStatus::Revoked), "Revoked");
        assert_eq!(format!("{}", CredentialStatus::Superseded), "Superseded");
    }

    #[test]
    fn test_key_metadata_creation() {
        let instance_id = crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let metadata = KeyMetadata::new(instance_id, CryptoAlgorithm::Aes256Gcm);

        assert_eq!(metadata.algorithm, CryptoAlgorithm::Aes256Gcm);
        assert_eq!(metadata.instance_id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
        assert!(metadata.created_at_ms > 0);
    }

    #[test]
    fn test_credential_serialization() {
        let credential = Credential {
            id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            kind: CredentialKind::ApiKey,
            name: "test-key".to_string(),
            current_version: CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
            versions: vec![CredentialVersion::new(
                CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
                SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
                CredentialStatus::Active,
                TimestampMs(1000),
                None,
            )],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        let serialized = serde_json::to_string(&credential).expect("serializable");
        let deserialized: Credential = serde_json::from_str(&serialized).expect("deserializable");

        assert_eq!(credential.id(), deserialized.id());
        assert_eq!(credential.kind(), deserialized.kind());
        assert_eq!(credential.name(), deserialized.name());
        assert_eq!(credential.current_version(), deserialized.current_version());
    }

    #[test]
    fn test_vault_entry_serialization() {
        let vault_entry = VaultEntry {
            entry_id: VaultEntryId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            credential: Credential {
                id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
                kind: CredentialKind::Password,
                name: "test".to_string(),
                current_version: CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
                versions: vec![CredentialVersion::new(
                    CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
                    SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
                    CredentialStatus::Active,
                    TimestampMs(1000),
                    None,
                )],
                rotation_policy: RotationPolicy::Manual,
                metadata: std::collections::HashMap::new(),
                created_at: TimestampMs(1000),
                updated_at: TimestampMs(1000),
            },
            access_policy: AccessPolicy::new(vec![Principal::System]),
            rotation_state: RotationState::new(),
        };

        let serialized = serde_json::to_string(&vault_entry).expect("serializable");
        let deserialized: VaultEntry = serde_json::from_str(&serialized).expect("deserializable");

        assert_eq!(vault_entry.entry_id(), deserialized.entry_id());
        assert_eq!(
            vault_entry.credential().name(),
            deserialized.credential().name()
        );
        assert_eq!(
            vault_entry.access_policy().audit_enabled(),
            deserialized.access_policy().audit_enabled()
        );
    }

    #[test]
    fn test_encrypted_blob_serialization() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 64], vec![2u8; 16]).unwrap();

        let serialized = serde_json::to_string(&blob).expect("serializable");
        let deserialized: EncryptedBlob =
            serde_json::from_str(&serialized).expect("deserializable");

        assert_eq!(blob.iv, deserialized.iv);
        assert_eq!(blob.ciphertext, deserialized.ciphertext);
        assert_eq!(blob.tag, deserialized.tag);
    }

    #[test]
    fn test_rotation_policy_serialization() {
        let time_based = RotationPolicy::TimeBased {
            interval: DurationMs(86400000),
            overlap_window: DurationMs(60000),
        };

        let serialized = serde_json::to_string(&time_based).expect("serializable");
        let deserialized: RotationPolicy =
            serde_json::from_str(&serialized).expect("deserializable");

        match (&time_based, &deserialized) {
            (
                RotationPolicy::TimeBased {
                    interval: i1,
                    overlap_window: o1,
                },
                RotationPolicy::TimeBased {
                    interval: i2,
                    overlap_window: o2,
                },
            ) => {
                assert_eq!(i1, i2);
                assert_eq!(o1, o2);
            }
            _ => panic!("Deserialized to wrong variant"),
        }
    }

    #[test]
    fn test_multiple_principal_types_in_access_policy() {
        let user = Principal::User(crate::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap());
        let actor = Principal::Actor(crate::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap());
        let workflow = Principal::Workflow(crate::WorkflowName::parse("data_processor").unwrap());
        let system = Principal::System;

        let policy = AccessPolicy::new(vec![user, actor, workflow, system]);

        assert_eq!(policy.allowed_principals().len(), 4);
        assert!(policy.audit_enabled());
        assert!(!policy.require_approval());
    }

    #[test]
    fn test_secret_value_key_versioning() {
        let secret_v1 = SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid");
        let secret_v2 = SecretValue::new(vec![1u8; 32], [1u8; 12], 2).expect("valid");
        let secret_v100 = SecretValue::new(vec![2u8; 32], [2u8; 12], 100).expect("valid");

        assert_eq!(secret_v1.key_version(), 1);
        assert_eq!(secret_v2.key_version(), 2);
        assert_eq!(secret_v100.key_version(), 100);

        assert!(secret_v1.ciphertext() != secret_v2.ciphertext());
        assert!(secret_v1.nonce() != secret_v2.nonce());
    }

    #[test]
    fn test_credential_metadata_storage() {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("environment".to_string(), "production".to_string());
        metadata.insert("team".to_string(), "platform".to_string());
        metadata.insert("owner".to_string(), "alice@example.com".to_string());

        let credential = Credential {
            id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            kind: CredentialKind::ApiKey,
            name: "production-api-key".to_string(),
            current_version: CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
            versions: vec![CredentialVersion::new(
                CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
                SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
                CredentialStatus::Active,
                TimestampMs(1000),
                None,
            )],
            rotation_policy: RotationPolicy::TimeBased {
                interval: DurationMs(86400000),
                overlap_window: DurationMs(60000),
            },
            metadata,
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        let meta = credential.metadata();
        assert_eq!(meta.get("environment"), Some(&"production".to_string()));
        assert_eq!(meta.get("team"), Some(&"platform".to_string()));
        assert_eq!(meta.get("owner"), Some(&"alice@example.com".to_string()));
    }

    #[test]
    fn test_rotation_state_failure_tracking() {
        let state = RotationState {
            state: RotationStatus::Failed("encryption error".to_string()),
            last_rotation: Some(TimestampMs(1000)),
            next_scheduled_rotation: None,
            consecutive_failures: 3,
            last_failure_reason: Some("encryption error".to_string()),
        };

        assert_eq!(
            state.state(),
            RotationStatus::Failed("encryption error".to_string())
        );
        assert_eq!(state.consecutive_failures(), 3);
        assert_eq!(state.last_failure_reason(), Some("encryption error"));
    }

    #[test]
    fn test_credential_expiration_tracking() {
        let created = TimestampMs(1000);
        let expires = TimestampMs(100000);

        let version = CredentialVersion::new(
            CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
            CredentialStatus::Active,
            created,
            Some(expires),
        );

        assert_eq!(version.created_at(), created);
        assert_eq!(version.expires_at(), Some(expires));
    }

    #[test]
    fn test_credential_without_expiration() {
        let version = CredentialVersion::new(
            CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
            CredentialStatus::Active,
            TimestampMs(1000),
            None,
        );

        assert!(version.expires_at().is_none());
    }

    #[test]
    fn test_rotated_version_links() {
        let old_version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let new_version_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();

        let old_version = CredentialVersion::new(
            old_version_id.clone(),
            SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid"),
            CredentialStatus::Superseded,
            TimestampMs(1000),
            None,
        );

        let new_version = CredentialVersion::new(
            new_version_id.clone(),
            SecretValue::new(vec![1u8; 32], [1u8; 12], 2).expect("valid"),
            CredentialStatus::Active,
            TimestampMs(5000),
            None,
        );

        assert!(old_version.rotated_from().is_none());
        assert!(old_version.rotated_to().is_none());
        assert!(new_version.rotated_from().is_none());
        assert!(new_version.rotated_to().is_none());

        let rotated_from = Some(old_version_id.clone());
        let rotated_to = Some(new_version_id.clone());
        let rotated_from_clone = rotated_from.clone();
        let rotated_to_clone = rotated_to.clone();

        let updated_old = CredentialVersion {
            rotated_from: None,
            rotated_to: rotated_from_clone,
            ..old_version
        };

        let updated_new = CredentialVersion {
            rotated_from: rotated_to,
            rotated_to: None,
            ..new_version
        };

        assert_eq!(updated_old.rotated_to(), Some(old_version_id));
        assert_eq!(updated_new.rotated_from(), Some(new_version_id));
    }
}
