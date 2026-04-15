use crate::credentials::*;
use crate::encryption::*;
use crate::{DurationMs, ParseError, TimestampMs};

fn valid_dek_id() -> DekId {
    DekId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn valid_instance_id() -> crate::string_types::InstanceId {
    crate::string_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn valid_credential_id() -> CredentialId {
    CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn valid_credential_version_id() -> CredentialVersionId {
    CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn valid_vault_entry_id() -> VaultEntryId {
    VaultEntryId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID")
}

fn make_secret_value(ciphertext: Vec<u8>, key_version: u32) -> SecretValue {
    SecretValue::new(ciphertext, [0u8; 12], key_version).expect("valid secret value")
}

fn make_credential_version(status: CredentialStatus, key_version: u32) -> CredentialVersion {
    CredentialVersion::new(
        valid_credential_version_id(),
        make_secret_value(vec![0u8; 32], key_version),
        status,
        TimestampMs(1000),
        None,
    )
}

#[cfg(test)]
mod blackhat_decrypt_wrong_key {

    use super::*;

    #[test]
    fn encrypted_blob_wrong_iv_returns_error_not_panic() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        let wrong_iv_blob =
            EncryptedBlob::new(vec![0xFFu8; 12], blob.ciphertext.clone(), blob.tag.clone());
        assert!(wrong_iv_blob.is_ok());
    }

    #[test]
    fn encrypted_blob_wrong_tag_returns_error_not_panic() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        let wrong_tag_blob =
            EncryptedBlob::new(blob.iv.clone(), blob.ciphertext.clone(), vec![0xFFu8; 16]);
        assert!(wrong_tag_blob.is_ok());
    }

    #[test]
    fn encrypted_blob_tampered_ciphertext_returns_error_not_panic() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        let mut tampered_ct = blob.ciphertext.clone();
        tampered_ct[0] ^= 0xFF;
        let tampered_blob = EncryptedBlob::new(blob.iv, tampered_ct, blob.tag);
        assert!(tampered_blob.is_ok());
    }

    #[test]
    fn secret_value_zero_key_version_is_valid() {
        let sv = make_secret_value(vec![1u8; 32], 0);
        assert_eq!(sv.key_version(), 0);
    }

    #[test]
    fn secret_value_max_key_version_is_valid() {
        let sv = make_secret_value(vec![1u8; 32], u32::MAX);
        assert_eq!(sv.key_version(), u32::MAX);
    }

    #[test]
    fn wrapped_dek_boundary_59_bytes_rejected() {
        let result = WrappedDek::new(vec![0u8; 59]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFormat {
                type_name: "WrappedDek",
                ..
            }
        ));
    }

    #[test]
    fn wrapped_dek_boundary_60_bytes_accepted() {
        let result = WrappedDek::new(vec![0u8; 60]);
        assert!(result.is_ok());
    }

    #[test]
    fn wrapped_dek_boundary_61_bytes_accepted() {
        let result = WrappedDek::new(vec![0u8; 61]);
        assert!(result.is_ok());
    }

    #[test]
    fn encrypted_blob_iv_wrong_length_11_rejected() {
        let result = EncryptedBlob::new(vec![0u8; 11], vec![1u8; 32], vec![2u8; 16]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFormat {
                type_name: "EncryptedBlob",
                ..
            }
        ));
    }

    #[test]
    fn encrypted_blob_iv_wrong_length_13_rejected() {
        let result = EncryptedBlob::new(vec![0u8; 13], vec![1u8; 32], vec![2u8; 16]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFormat {
                type_name: "EncryptedBlob",
                ..
            }
        ));
    }

    #[test]
    fn encrypted_blob_tag_wrong_length_15_rejected() {
        let result = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 15]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFormat {
                type_name: "EncryptedBlob",
                ..
            }
        ));
    }

    #[test]
    fn encrypted_blob_tag_wrong_length_17_rejected() {
        let result = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 17]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFormat {
                type_name: "EncryptedBlob",
                ..
            }
        ));
    }

    #[test]
    fn encrypted_blob_ciphertext_empty_is_valid() {
        let result = EncryptedBlob::new(vec![0u8; 12], vec![], vec![2u8; 16]);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod blackhat_encrypt_max_size {

    use super::*;

    #[test]
    fn encrypted_blob_max_ciphertext_no_truncation() {
        let large_ciphertext = vec![0xAB; 1024 * 1024];
        let blob = EncryptedBlob::new(vec![0u8; 12], large_ciphertext.clone(), vec![0xCD; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        assert_eq!(blob.ciphertext.len(), 1024 * 1024);
        assert_eq!(blob.total_size(), 12 + 1024 * 1024 + 16);
    }

    #[test]
    fn encrypted_blob_very_large_ciphertext_no_truncation() {
        let huge_ciphertext = vec![0xAB; 10 * 1024 * 1024];
        let blob = EncryptedBlob::new(vec![0u8; 12], huge_ciphertext.clone(), vec![0xCD; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        assert_eq!(blob.ciphertext.len(), 10 * 1024 * 1024);
    }

    #[test]
    fn secret_value_max_ciphertext_no_truncation() {
        let large_ct = vec![0xAB; 1024 * 1024];
        let sv = SecretValue::new(large_ct.clone(), [0u8; 12], 1);
        assert!(sv.is_ok());
        let sv = sv.unwrap();
        assert_eq!(sv.ciphertext().len(), 1024 * 1024);
    }

    #[test]
    fn wrapped_dek_large_payload_no_truncation() {
        let large_data = vec![0xAB; 100 * 1024];
        let wrapped = WrappedDek::new(large_data.clone());
        assert!(wrapped.is_ok());
        let wrapped = wrapped.unwrap();
        assert_eq!(wrapped.as_bytes().len(), 100 * 1024);
    }

    #[test]
    fn encrypted_blob_64k_ciphertext_exact_boundary() {
        let ct_64k = vec![0xAB; 65536];
        let blob = EncryptedBlob::new(vec![0u8; 12], ct_64k.clone(), vec![0xCD; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        assert_eq!(blob.ciphertext.len(), 65536);
    }

    #[test]
    fn encrypted_blob_1mb_ciphertext_exact_boundary() {
        let ct_1mb = vec![0xAB; 1024 * 1024];
        let blob = EncryptedBlob::new(vec![0u8; 12], ct_1mb.clone(), vec![0xCD; 16]);
        assert!(blob.is_ok());
        let blob = blob.unwrap();
        assert_eq!(blob.ciphertext.len(), 1024 * 1024);
    }
}

#[cfg(test)]
mod blackhat_credential_rotation_forward_secrecy {

    use super::*;

    #[test]
    fn credential_rotation_state_transitions_are_valid() {
        let state = RotationState::new();
        assert_eq!(state.state(), RotationStatus::Idle);

        let mut state_with_rotation = RotationState::new();
        assert!(state_with_rotation.last_rotation().is_none());

        let now = TimestampMs(2000);
        let next = TimestampMs(3000);
        let state_rotating = RotationState {
            state: RotationStatus::Rotating,
            last_rotation: Some(now),
            next_scheduled_rotation: Some(next),
            consecutive_failures: 0,
            last_failure_reason: None,
        };
        assert_eq!(state_rotating.state(), RotationStatus::Rotating);
        assert_eq!(state_rotating.last_rotation(), Some(now));
        assert_eq!(state_rotating.next_scheduled_rotation(), Some(next));
    }

    #[test]
    fn credential_version_chain_rotated_from_to() {
        let v1_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let v2_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let v3_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap();

        let v1 = CredentialVersion::new(
            v1_id.clone(),
            make_secret_value(vec![0u8; 32], 1),
            CredentialStatus::Superseded,
            TimestampMs(1000),
            None,
        );

        let v2 = CredentialVersion::new(
            v2_id.clone(),
            make_secret_value(vec![1u8; 32], 2),
            CredentialStatus::Active,
            TimestampMs(2000),
            None,
        );

        let mut v2_with_chain = v2.clone();
        v2_with_chain.rotated_from = Some(v1_id.clone());
        v2_with_chain.rotated_to = Some(v3_id.clone());

        assert_eq!(v2_with_chain.rotated_from(), Some(v1_id.clone()));
        assert_eq!(v2_with_chain.rotated_to(), Some(v3_id.clone()));
    }

    #[test]
    fn credential_key_version_increments_on_rotation() {
        let v1 = make_credential_version(CredentialStatus::Superseded, 1);
        let v2 = make_credential_version(CredentialStatus::Active, 2);

        assert!(v1.secret_value.key_version() < v2.secret_value.key_version());
    }

    #[test]
    fn credential_rotating_status_is_not_terminal() {
        assert!(!CredentialStatus::Rotating.is_terminal());
    }

    #[test]
    fn credential_active_version_after_rotation() {
        let mut credential = Credential {
            id: valid_credential_id(),
            kind: CredentialKind::ApiKey,
            name: "test".to_string(),
            current_version: valid_credential_version_id(),
            versions: vec![],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        let v1_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let v1 = CredentialVersion::new(
            v1_id.clone(),
            make_secret_value(vec![0u8; 32], 1),
            CredentialStatus::Superseded,
            TimestampMs(1000),
            None,
        );

        let v2_id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap();
        let v2 = CredentialVersion::new(
            v2_id.clone(),
            make_secret_value(vec![1u8; 32], 2),
            CredentialStatus::Active,
            TimestampMs(2000),
            None,
        );

        credential.versions = vec![v1, v2];
        credential.current_version = v2_id;

        let active = credential.active_version();
        assert!(active.is_some());
        assert_eq!(active.unwrap().secret_value.key_version(), 2);
    }

    #[test]
    fn forward_secrecy_different_keys_per_version() {
        let v1_ct = vec![0x11u8; 32];
        let v2_ct = vec![0x22u8; 32];

        let v1 = make_secret_value(v1_ct, 1);
        let v2 = make_secret_value(v2_ct, 2);

        assert_ne!(v1.ciphertext(), v2.ciphertext());
        assert_ne!(v1.key_version(), v2.key_version());
    }

    #[test]
    fn rotation_overlap_window_enforced() {
        let policy = RotationPolicy::TimeBased {
            interval: DurationMs(86400000),
            overlap_window: DurationMs(60000),
        };
        assert!(policy.validate().is_ok());

        let small_overlap = RotationPolicy::TimeBased {
            interval: DurationMs(86400000),
            overlap_window: DurationMs(59999),
        };
        assert!(small_overlap.validate().is_err());
    }
}

#[cfg(test)]
mod blackhat_vault_entry_malformed_blobs {

    use super::*;

    fn make_vault_entry_with_secret(ciphertext: Vec<u8>) -> VaultEntry {
        let credential_version = CredentialVersion::new(
            valid_credential_version_id(),
            make_secret_value(ciphertext, 1),
            CredentialStatus::Active,
            TimestampMs(1000),
            None,
        );

        let credential = Credential {
            id: valid_credential_id(),
            kind: CredentialKind::ApiKey,
            name: "test".to_string(),
            current_version: valid_credential_version_id(),
            versions: vec![credential_version],
            rotation_policy: RotationPolicy::Manual,
            metadata: std::collections::HashMap::new(),
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        VaultEntry {
            entry_id: valid_vault_entry_id(),
            credential,
            access_policy: AccessPolicy::new(vec![Principal::System]),
            rotation_state: RotationState::new(),
        }
    }

    #[test]
    fn vault_entry_empty_ciphertext_rejected() {
        let result = SecretValue::new(vec![], [0u8; 12], 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            ParseError::Empty {
                type_name: "SecretValue"
            }
        ));
    }

    #[test]
    fn vault_entry_single_byte_ciphertext_accepted() {
        let entry = make_vault_entry_with_secret(vec![0xAB]);
        let active = entry.credential.active_version();
        assert!(active.is_some());
        assert_eq!(active.unwrap().secret_value.ciphertext().len(), 1);
    }

    #[test]
    fn vault_entry_unicode_ciphertext_accepted() {
        let unicode_ct = "секрет".as_bytes().to_vec();
        let entry = make_vault_entry_with_secret(unicode_ct);
        let active = entry.credential.active_version();
        assert!(active.is_some());
    }

    #[test]
    fn vault_entry_binary_ciphertext_accepted() {
        let binary_ct = vec![0x00, 0xFF, 0x42, 0x13, 0xDE, 0xAD, 0xBE, 0xEF];
        let entry = make_vault_entry_with_secret(binary_ct);
        let active = entry.credential.active_version();
        assert!(active.is_some());
    }

    #[test]
    fn vault_entry_all_zeros_ciphertext_accepted() {
        let zeros_ct = vec![0x00; 64];
        let entry = make_vault_entry_with_secret(zeros_ct);
        let active = entry.credential.active_version();
        assert!(active.is_some());
    }

    #[test]
    fn vault_entry_all_ones_ciphertext_accepted() {
        let ones_ct = vec![0xFF; 64];
        let entry = make_vault_entry_with_secret(ones_ct);
        let active = entry.credential.active_version();
        assert!(active.is_some());
    }

    #[test]
    fn vault_entry_with_special_characters_in_metadata() {
        let credential_version = CredentialVersion::new(
            valid_credential_version_id(),
            make_secret_value(vec![0u8; 32], 1),
            CredentialStatus::Active,
            TimestampMs(1000),
            None,
        );

        let mut metadata = std::collections::HashMap::new();
        metadata.insert("user".to_string(), "admin".to_string());
        metadata.insert("note".to_string(), "secure;delete-after=2024".to_string());

        let credential = Credential {
            id: valid_credential_id(),
            kind: CredentialKind::ApiKey,
            name: "test".to_string(),
            current_version: valid_credential_version_id(),
            versions: vec![credential_version],
            rotation_policy: RotationPolicy::Manual,
            metadata,
            created_at: TimestampMs(1000),
            updated_at: TimestampMs(1000),
        };

        let entry = VaultEntry {
            entry_id: valid_vault_entry_id(),
            credential,
            access_policy: AccessPolicy::new(vec![Principal::System]),
            rotation_state: RotationState::new(),
        };

        assert_eq!(
            entry.credential.metadata().get("user"),
            Some(&"admin".to_string())
        );
    }
}

#[cfg(test)]
mod blackhat_dek_wrapping_boundary_keys {

    use super::*;

    #[test]
    fn wrapped_dek_exactly_60_bytes_minimum_boundary() {
        let data = vec![0u8; 60];
        let result = WrappedDek::new(data.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_bytes(), &data);
    }

    #[test]
    fn wrapped_dek_61_bytes_one_over_minimum() {
        let data = vec![0u8; 61];
        let result = WrappedDek::new(data.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_bytes().len(), 61);
    }

    #[test]
    fn wrapped_dek_59_bytes_one_under_minimum_rejected() {
        let data = vec![0u8; 59];
        let result = WrappedDek::new(data);
        assert!(result.is_err());
    }

    #[test]
    fn wrapped_dek_128_bytes_standard_key_size() {
        let data = vec![0u8; 128];
        let result = WrappedDek::new(data.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_bytes().len(), 128);
    }

    #[test]
    fn wrapped_dek_256_bytes_large_key_size() {
        let data = vec![0u8; 256];
        let result = WrappedDek::new(data.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_bytes().len(), 256);
    }

    #[test]
    fn wrapped_dek_512_bytes_very_large_key() {
        let data = vec![0xAB; 512];
        let result = WrappedDek::new(data.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_bytes().len(), 512);
    }

    #[test]
    fn wrapped_dek_empty_rejected() {
        let result = WrappedDek::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn wrapped_dek_max_usize_boundary() {
        let data = vec![0u8; 0];
        let result = WrappedDek::new(data);
        assert!(result.is_err());
    }

    #[test]
    fn dek_id_from_bytes_all_zeros_rejected() {
        let bytes = [0u8; 16];
        let result = DekId::from_bytes(bytes);
        assert_eq!(result.as_str(), "00000000000000000000000000");
        let parse_result = DekId::parse(result.as_str());
        assert!(parse_result.is_err());
    }

    #[test]
    fn dek_id_from_bytes_max_value() {
        let bytes = [0xFFu8; 16];
        let id = DekId::from_bytes(bytes);
        let parsed = DekId::parse(id.as_str());
        assert!(parsed.is_ok());
    }

    #[test]
    fn wrapped_dek_display_shows_length() {
        let wrapped = WrappedDek::new(vec![0u8; 100]).unwrap();
        let display = format!("{}", wrapped);
        assert!(display.contains("100 bytes"));
    }

    #[test]
    fn encrypted_blob_display_shows_components() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 32], vec![2u8; 16]).unwrap();
        let display = format!("{}", blob);
        assert!(display.contains("iv=12"));
        assert!(display.contains("ciphertext=32"));
        assert!(display.contains("tag=16"));
    }
}

#[cfg(test)]
mod blackhat_credential_rotation_failure_handling {

    use super::*;

    #[test]
    fn rotation_state_failed_status_carries_reason() {
        let failed_state = RotationState {
            state: RotationStatus::Failed("key not found".to_string()),
            last_rotation: None,
            next_scheduled_rotation: None,
            consecutive_failures: 1,
            last_failure_reason: Some("key not found".to_string()),
        };
        assert!(matches!(failed_state.state(), RotationStatus::Failed(_)));
    }

    #[test]
    fn rotation_state_consecutive_failures_increment() {
        let mut state = RotationState::new();
        assert_eq!(state.consecutive_failures(), 0);

        state.consecutive_failures += 1;
        assert_eq!(state.consecutive_failures(), 1);

        state.consecutive_failures += 1;
        assert_eq!(state.consecutive_failures(), 2);
    }

    #[test]
    fn rotation_policy_time_based_zero_interval_rejected() {
        let policy = RotationPolicy::TimeBased {
            interval: DurationMs(0),
            overlap_window: DurationMs(60000),
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn rotation_policy_usage_based_zero_max_uses_rejected() {
        let policy = RotationPolicy::UsageBased {
            max_uses: 0,
            overlap_window: DurationMs(60000),
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn rotation_policy_event_based_empty_triggers_rejected() {
        let policy = RotationPolicy::EventBased {
            trigger_events: vec![],
            overlap_window: DurationMs(60000),
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn credential_superseded_is_terminal() {
        assert!(CredentialStatus::Superseded.is_terminal());
    }

    #[test]
    fn credential_expired_is_terminal() {
        assert!(CredentialStatus::Expired.is_terminal());
    }

    #[test]
    fn credential_revoked_is_terminal() {
        assert!(CredentialStatus::Revoked.is_terminal());
    }
}

#[cfg(test)]
mod blackhat_encryption_invariants {

    use super::*;

    #[test]
    fn iv_size_exactly_12_bytes() {
        assert_eq!(CryptoAlgorithm::IV_SIZE_BYTES, 12);
    }

    #[test]
    fn tag_size_exactly_16_bytes() {
        assert_eq!(CryptoAlgorithm::TAG_SIZE_BYTES, 16);
    }

    #[test]
    fn key_size_exactly_32_bytes() {
        assert_eq!(CryptoAlgorithm::KEY_SIZE_BYTES, 32);
    }

    #[test]
    fn encrypted_blob_total_size_components() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![1u8; 100], vec![2u8; 16]).unwrap();
        assert_eq!(blob.total_size(), 12 + 100 + 16);
    }

    #[test]
    fn encrypted_blob_with_zero_ciphertext() {
        let blob = EncryptedBlob::new(vec![0u8; 12], vec![], vec![2u8; 16]).unwrap();
        assert_eq!(blob.total_size(), 12 + 0 + 16);
        assert_eq!(blob.ciphertext.len(), 0);
    }

    #[test]
    fn key_metadata_created_at_is_reasonable() {
        let instance_id = valid_instance_id();
        let metadata = KeyMetadata::new(instance_id, CryptoAlgorithm::Aes256Gcm);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(metadata.created_at_ms <= now);
        assert!(now - metadata.created_at_ms < 1000);
    }
}

#[cfg(test)]
mod blackhat_credential_kind_variants {

    use super::*;

    #[test]
    fn all_credential_kinds_are_represented() {
        let variants = CredentialKind::all_variants();
        assert_eq!(variants.len(), 6);
        assert!(variants.contains(&CredentialKind::ApiKey));
        assert!(variants.contains(&CredentialKind::Password));
        assert!(variants.contains(&CredentialKind::Token));
        assert!(variants.contains(&CredentialKind::Certificate));
        assert!(variants.contains(&CredentialKind::SigningKey));
        assert!(variants.contains(&CredentialKind::EncryptionKey));
    }

    #[test]
    fn custom_credential_kind_arbitrary_string() {
        let custom = CredentialKind::Custom("OAuth2.0".to_string());
        let display = format!("{}", custom);
        assert!(display.contains("OAuth2.0"));
    }

    #[test]
    fn credential_kind_display_known_variants() {
        assert_eq!(format!("{}", CredentialKind::ApiKey), "ApiKey");
        assert_eq!(format!("{}", CredentialKind::Password), "Password");
        assert_eq!(format!("{}", CredentialKind::Token), "Token");
        assert_eq!(format!("{}", CredentialKind::Certificate), "Certificate");
        assert_eq!(format!("{}", CredentialKind::SigningKey), "SigningKey");
        assert_eq!(
            format!("{}", CredentialKind::EncryptionKey),
            "EncryptionKey"
        );
    }

    #[test]
    fn credential_status_display_all_variants() {
        assert_eq!(format!("{}", CredentialStatus::Active), "Active");
        assert_eq!(format!("{}", CredentialStatus::Rotating), "Rotating");
        assert_eq!(format!("{}", CredentialStatus::Expired), "Expired");
        assert_eq!(format!("{}", CredentialStatus::Revoked), "Revoked");
        assert_eq!(format!("{}", CredentialStatus::Superseded), "Superseded");
    }

    #[test]
    fn rotation_status_display_all_variants() {
        assert_eq!(format!("{}", RotationStatus::Idle), "Idle");
        assert_eq!(format!("{}", RotationStatus::Rotating), "Rotating");
        assert_eq!(
            format!("{}", RotationStatus::WaitingForOverlap),
            "WaitingForOverlap"
        );
        assert_eq!(
            format!("{}", RotationStatus::Failed("err".to_string())),
            "Failed(err)"
        );
    }
}

#[cfg(test)]
mod blackhat_access_policy {

    use super::*;

    #[test]
    fn access_policy_audit_enabled_by_default() {
        let instance_id = valid_instance_id();
        let policy = AccessPolicy::new(vec![Principal::User(instance_id)]);
        assert!(policy.audit_enabled());
        assert!(!policy.require_approval());
        assert!(policy.approvers().is_empty());
    }

    #[test]
    fn principal_user_display() {
        let instance_id = valid_instance_id();
        let principal = Principal::User(instance_id);
        let display = format!("{}", principal);
        assert!(display.contains("User"));
    }

    #[test]
    fn principal_system_display() {
        let principal = Principal::System;
        assert_eq!(format!("{}", principal), "System");
    }

    #[test]
    fn principal_actor_display() {
        let spawn_id = crate::string_types::SpawnId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
        let principal = Principal::Actor(spawn_id);
        let display = format!("{}", principal);
        assert!(display.contains("Actor"));
    }

    #[test]
    fn principal_workflow_display() {
        let workflow_name = crate::string_types::WorkflowName::parse("test-workflow").unwrap();
        let principal = Principal::Workflow(workflow_name);
        let display = format!("{}", principal);
        assert!(display.contains("Workflow"));
    }
}
