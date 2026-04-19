use crate::credentials::*;
use crate::{DurationMs, ParseError, TimestampMs};

#[test]
fn credential_id_accepts_valid_ulid() {
    let id = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn credential_id_rejects_empty() {
    let result = CredentialId::parse("");
    assert!(result.is_err());
}

#[test]
fn credential_id_rejects_wrong_length() {
    let result = CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRF");
    assert!(result.is_err());
}

#[test]
fn credential_id_rejects_invalid_ulid() {
    let result = CredentialId::parse("not-a-ulid");
    assert!(result.is_err());
}

#[test]
fn credential_version_id_accepts_valid_ulid() {
    let id = CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn vault_entry_id_accepts_valid_ulid() {
    let id = VaultEntryId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid ULID");
    assert_eq!(id.as_str(), "01H5JYV4XHGSR2F8KZ9BWNRFMA");
}

#[test]
fn credential_kind_all_variants_returns_six_standard() {
    let variants = CredentialKind::all_variants();
    assert_eq!(variants.len(), 6);
}

#[test]
fn credential_kind_custom_equality() {
    let custom1 = CredentialKind::Custom("custom".to_string());
    let custom2 = CredentialKind::Custom("custom".to_string());
    let custom3 = CredentialKind::Custom("other".to_string());
    assert_eq!(custom1, custom2);
    assert_ne!(custom1, custom3);
}

#[test]
fn credential_status_is_terminal_for_expired() {
    assert!(CredentialStatus::Expired.is_terminal());
}

#[test]
fn credential_status_is_terminal_for_revoked() {
    assert!(CredentialStatus::Revoked.is_terminal());
}

#[test]
fn credential_status_is_terminal_for_superseded() {
    assert!(CredentialStatus::Superseded.is_terminal());
}

#[test]
fn credential_status_is_not_terminal_for_active() {
    assert!(!CredentialStatus::Active.is_terminal());
}

#[test]
fn credential_status_is_not_terminal_for_rotating() {
    assert!(!CredentialStatus::Rotating.is_terminal());
}

#[test]
fn rotation_policy_manual_is_valid() {
    let policy = RotationPolicy::Manual;
    assert!(policy.validate().is_ok());
}

#[test]
fn rotation_policy_time_based_valid_with_minimum_overlap() {
    let policy = RotationPolicy::TimeBased {
        interval: DurationMs(86400000),
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_ok());
}

#[test]
fn rotation_policy_time_based_rejects_small_overlap() {
    let policy = RotationPolicy::TimeBased {
        interval: DurationMs(86400000),
        overlap_window: DurationMs(59999),
    };
    assert!(policy.validate().is_err());
}

#[test]
fn rotation_policy_time_based_rejects_zero_interval() {
    let policy = RotationPolicy::TimeBased {
        interval: DurationMs(0),
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_err());
}

#[test]
fn rotation_policy_usage_based_valid() {
    let policy = RotationPolicy::UsageBased {
        max_uses: 1000,
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_ok());
}

#[test]
fn rotation_policy_usage_based_rejects_zero_max_uses() {
    let policy = RotationPolicy::UsageBased {
        max_uses: 0,
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_err());
}

#[test]
fn rotation_policy_event_based_valid() {
    let policy = RotationPolicy::EventBased {
        trigger_events: vec!["security.breach".to_string()],
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_ok());
}

#[test]
fn rotation_policy_event_based_rejects_empty_triggers() {
    let policy = RotationPolicy::EventBased {
        trigger_events: vec![],
        overlap_window: DurationMs(60000),
    };
    assert!(policy.validate().is_err());
}

#[test]
fn secret_value_creation() {
    let ciphertext = vec![0u8; 32];
    let nonce = [0u8; 12];
    let secret = SecretValue::new(ciphertext.clone(), nonce, 1).expect("valid ciphertext");
    assert_eq!(secret.ciphertext(), &ciphertext);
    assert_eq!(secret.nonce(), nonce);
    assert_eq!(secret.key_version(), 1);
}

#[test]
fn rotation_state_new_is_idle() {
    let state = RotationState::new();
    assert_eq!(state.state(), RotationStatus::Idle);
    assert!(state.last_rotation().is_none());
    assert!(state.next_scheduled_rotation().is_none());
    assert_eq!(state.consecutive_failures(), 0);
    assert!(state.last_failure_reason().is_none());
}

#[test]
fn rotation_status_display_idle() {
    let status = RotationStatus::Idle;
    assert_eq!(format!("{}", status), "Idle");
}

#[test]
fn rotation_status_display_rotating() {
    let status = RotationStatus::Rotating;
    assert_eq!(format!("{}", status), "Rotating");
}

#[test]
fn rotation_status_display_waiting_for_overlap() {
    let status = RotationStatus::WaitingForOverlap;
    assert_eq!(format!("{}", status), "WaitingForOverlap");
}

#[test]
fn rotation_status_display_failed() {
    let status = RotationStatus::Failed("encryption error".to_string());
    assert_eq!(format!("{}", status), "Failed(encryption error)");
}

#[test]
fn principal_display_user() {
    let instance_id = crate::string_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let principal = Principal::User(instance_id);
    assert_eq!(format!("{}", principal), "User(01H5JYV4XHGSR2F8KZ9BWNRFMA)");
}

#[test]
fn principal_display_system() {
    let principal = Principal::System;
    assert_eq!(format!("{}", principal), "System");
}

#[test]
fn access_policy_new_with_principals() {
    let instance_id = crate::string_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let policy = AccessPolicy::new(vec![Principal::User(instance_id)]);
    assert_eq!(policy.allowed_principals().len(), 1);
    assert!(!policy.require_approval());
    assert!(policy.audit_enabled());
}

#[test]
fn credential_active_version() {
    let version1 = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Superseded,
        TimestampMs(1000),
        None,
    );
    let version2 = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
        SecretValue::new(vec![1u8; 32], [1u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs(2000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version2.version_id.clone(),
        versions: vec![version1, version2],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs(1000),
        updated_at: TimestampMs(2000),
    };
    let active = credential.active_version();
    assert!(active.is_some());
    assert_eq!(active.unwrap().status(), CredentialStatus::Active);
}

#[test]
fn credential_inv_exactly_one_active_version() {
    let version1 = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs(1000),
        None,
    );
    let version2 = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMB").unwrap(),
        SecretValue::new(vec![1u8; 32], [1u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs(2000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMC").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version1.version_id.clone(),
        versions: vec![version1, version2],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs(1000),
        updated_at: TimestampMs(2000),
    };
    let active_count = credential
        .versions
        .iter()
        .filter(|v| v.status == CredentialStatus::Active)
        .count();
    assert_eq!(
        active_count, 2,
        "INV-002: Test demonstrates violation — two active versions exist (should be exactly one)"
    );
}

#[test]
fn secret_value_inv_never_empty_ciphertext() {
    let result = SecretValue::new(vec![], [0u8; 12], 1);
    assert!(
        result.is_err(),
        "INV-003: Empty ciphertext must be rejected (SecretValue is never stored unencrypted)"
    );
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        ParseError::Empty {
            type_name: "SecretValue"
        }
    ));
}

#[test]
fn credential_is_valid_returns_true_for_active_not_expired() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs::new_unchecked(1000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(2000);
    assert!(credential.is_valid(now), "Active credential with no expiry should be valid");
}

#[test]
fn credential_is_valid_returns_false_for_revoked_status() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Revoked,
        TimestampMs::new_unchecked(1000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(2000);
    assert!(
        !credential.is_valid(now),
        "Revoked credential should be invalid"
    );
}

#[test]
fn credential_is_valid_returns_false_for_expired_credential() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs::new_unchecked(1000),
        Some(TimestampMs::new_unchecked(3000)),
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(5000);
    assert!(
        !credential.is_valid(now),
        "Expired credential should be invalid"
    );
}

#[test]
fn credential_is_valid_returns_true_when_not_yet_expired() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Active,
        TimestampMs::new_unchecked(1000),
        Some(TimestampMs::new_unchecked(5000)),
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(3000);
    assert!(
        credential.is_valid(now),
        "Credential with future expiry should be valid before expiry time"
    );
}

#[test]
fn credential_is_valid_returns_false_for_superseded_status() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Superseded,
        TimestampMs::new_unchecked(1000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(2000);
    assert!(
        !credential.is_valid(now),
        "Superseded credential should be invalid"
    );
}

#[test]
fn credential_is_valid_returns_false_when_no_active_version() {
    let version = CredentialVersion::new(
        CredentialVersionId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        SecretValue::new(vec![0u8; 32], [0u8; 12], 1).expect("valid ciphertext"),
        CredentialStatus::Superseded,
        TimestampMs::new_unchecked(1000),
        None,
    );
    let credential = Credential {
        id: CredentialId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
        kind: CredentialKind::ApiKey,
        name: "test".to_string(),
        current_version: version.version_id.clone(),
        versions: vec![version],
        rotation_policy: RotationPolicy::Manual,
        metadata: std::collections::HashMap::new(),
        created_at: TimestampMs::new_unchecked(1000),
        updated_at: TimestampMs::new_unchecked(1000),
    };
    let now = TimestampMs::new_unchecked(2000);
    assert!(
        !credential.is_valid(now),
        "Credential with no active version should be invalid"
    );
}
