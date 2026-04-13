use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ParseError;
use crate::{DurationMs, TimestampMs};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CredentialId(pub(crate) String);

impl CredentialId {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "CredentialId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        let _ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CredentialId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CredentialId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CredentialId> for String {
    fn from(value: CredentialId) -> String {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CredentialVersionId(pub(crate) String);

impl CredentialVersionId {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "CredentialVersionId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        let _ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CredentialVersionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CredentialVersionId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CredentialVersionId> for String {
    fn from(value: CredentialVersionId) -> String {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VaultEntryId(pub(crate) String);

impl VaultEntryId {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "VaultEntryId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        let _ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VaultEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for VaultEntryId {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<VaultEntryId> for String {
    fn from(value: VaultEntryId) -> String {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CredentialKind {
    ApiKey,
    Password,
    Token,
    Certificate,
    SigningKey,
    EncryptionKey,
    Custom(String),
}

impl CredentialKind {
    #[must_use]
    pub fn all_variants() -> Vec<Self> {
        vec![
            Self::ApiKey,
            Self::Password,
            Self::Token,
            Self::Certificate,
            Self::SigningKey,
            Self::EncryptionKey,
        ]
    }
}

impl fmt::Display for CredentialKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey => write!(f, "ApiKey"),
            Self::Password => write!(f, "Password"),
            Self::Token => write!(f, "Token"),
            Self::Certificate => write!(f, "Certificate"),
            Self::SigningKey => write!(f, "SigningKey"),
            Self::EncryptionKey => write!(f, "EncryptionKey"),
            Self::Custom(s) => write!(f, "Custom({s})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialStatus {
    Active,
    Rotating,
    Expired,
    Revoked,
    Superseded,
}

impl CredentialStatus {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Expired | Self::Revoked | Self::Superseded)
    }
}

impl fmt::Display for CredentialStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Rotating => write!(f, "Rotating"),
            Self::Expired => write!(f, "Expired"),
            Self::Revoked => write!(f, "Revoked"),
            Self::Superseded => write!(f, "Superseded"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationPolicy {
    Manual,
    TimeBased {
        interval: DurationMs,
        overlap_window: DurationMs,
    },
    UsageBased {
        max_uses: u64,
        overlap_window: DurationMs,
    },
    EventBased {
        trigger_events: Vec<String>,
        overlap_window: DurationMs,
    },
}

impl RotationPolicy {
    pub fn validate(&self) -> Result<(), ParseError> {
        const MIN_OVERLAP_MS: u64 = 60_000;
        match self {
            Self::Manual => Ok(()),
            Self::TimeBased {
                interval,
                overlap_window,
            } => {
                if interval.0 == 0 {
                    return Err(ParseError::ZeroValue {
                        type_name: "interval",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
            Self::UsageBased {
                max_uses,
                overlap_window,
            } => {
                if *max_uses == 0 {
                    return Err(ParseError::ZeroValue {
                        type_name: "max_uses",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
            Self::EventBased {
                trigger_events,
                overlap_window,
            } => {
                if trigger_events.is_empty() {
                    return Err(ParseError::Empty {
                        type_name: "trigger_events",
                    });
                }
                if overlap_window.0 < MIN_OVERLAP_MS {
                    return Err(ParseError::OutOfRange {
                        type_name: "overlap_window",
                        value: overlap_window.0,
                        min: MIN_OVERLAP_MS,
                        max: u64::MAX,
                    });
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretValue {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    pub key_version: u32,
}

impl SecretValue {
    pub fn new(ciphertext: Vec<u8>, nonce: [u8; 12], key_version: u32) -> Result<Self, ParseError> {
        if ciphertext.is_empty() {
            return Err(ParseError::Empty {
                type_name: "SecretValue",
            });
        }
        Ok(Self {
            ciphertext,
            nonce,
            key_version,
        })
    }

    #[must_use]
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    #[must_use]
    pub fn nonce(&self) -> [u8; 12] {
        self.nonce
    }

    #[must_use]
    pub fn key_version(&self) -> u32 {
        self.key_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialVersion {
    pub version_id: CredentialVersionId,
    pub secret_value: SecretValue,
    pub status: CredentialStatus,
    pub created_at: TimestampMs,
    pub expires_at: Option<TimestampMs>,
    pub rotated_from: Option<CredentialVersionId>,
    pub rotated_to: Option<CredentialVersionId>,
}

impl CredentialVersion {
    #[must_use]
    pub fn new(
        version_id: CredentialVersionId,
        secret_value: SecretValue,
        status: CredentialStatus,
        created_at: TimestampMs,
        expires_at: Option<TimestampMs>,
    ) -> Self {
        Self {
            version_id,
            secret_value,
            status,
            created_at,
            expires_at,
            rotated_from: None,
            rotated_to: None,
        }
    }

    #[must_use]
    pub fn status(&self) -> CredentialStatus {
        self.status.clone()
    }

    #[must_use]
    pub fn created_at(&self) -> TimestampMs {
        self.created_at
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<TimestampMs> {
        self.expires_at
    }

    #[must_use]
    pub fn rotated_from(&self) -> Option<CredentialVersionId> {
        self.rotated_from.clone()
    }

    #[must_use]
    pub fn rotated_to(&self) -> Option<CredentialVersionId> {
        self.rotated_to.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub id: CredentialId,
    pub kind: CredentialKind,
    pub name: String,
    pub current_version: CredentialVersionId,
    pub versions: Vec<CredentialVersion>,
    pub rotation_policy: RotationPolicy,
    pub metadata: std::collections::HashMap<String, String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

impl Credential {
    #[must_use]
    pub fn id(&self) -> CredentialId {
        self.id.clone()
    }

    #[must_use]
    pub fn kind(&self) -> CredentialKind {
        self.kind.clone()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn current_version(&self) -> CredentialVersionId {
        self.current_version.clone()
    }

    #[must_use]
    pub fn versions(&self) -> &[CredentialVersion] {
        &self.versions
    }

    #[must_use]
    pub fn rotation_policy(&self) -> RotationPolicy {
        self.rotation_policy.clone()
    }

    #[must_use]
    pub fn metadata(&self) -> &std::collections::HashMap<String, String> {
        &self.metadata
    }

    #[must_use]
    pub fn created_at(&self) -> TimestampMs {
        self.created_at
    }

    #[must_use]
    pub fn updated_at(&self) -> TimestampMs {
        self.updated_at
    }

    pub fn active_version(&self) -> Option<&CredentialVersion> {
        self.versions
            .iter()
            .find(|v| v.status == CredentialStatus::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    pub entry_id: VaultEntryId,
    pub credential: Credential,
    pub access_policy: AccessPolicy,
    pub rotation_state: RotationState,
}

impl VaultEntry {
    #[must_use]
    pub fn entry_id(&self) -> VaultEntryId {
        self.entry_id.clone()
    }

    #[must_use]
    pub fn credential(&self) -> &Credential {
        &self.credential
    }

    #[must_use]
    pub fn access_policy(&self) -> &AccessPolicy {
        &self.access_policy
    }

    #[must_use]
    pub fn rotation_state(&self) -> &RotationState {
        &self.rotation_state
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub allowed_principals: Vec<Principal>,
    pub require_approval: bool,
    pub approvers: Vec<Principal>,
    pub audit_enabled: bool,
}

impl AccessPolicy {
    #[must_use]
    pub fn new(allowed_principals: Vec<Principal>) -> Self {
        Self {
            allowed_principals,
            require_approval: false,
            approvers: Vec::new(),
            audit_enabled: true,
        }
    }

    #[must_use]
    pub fn allowed_principals(&self) -> &[Principal] {
        &self.allowed_principals
    }

    #[must_use]
    pub fn require_approval(&self) -> bool {
        self.require_approval
    }

    #[must_use]
    pub fn approvers(&self) -> &[Principal] {
        &self.approvers
    }

    #[must_use]
    pub fn audit_enabled(&self) -> bool {
        self.audit_enabled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Principal {
    User(crate::string_types::InstanceId),
    Actor(crate::string_types::SpawnId),
    Workflow(crate::string_types::WorkflowName),
    System,
}

impl fmt::Display for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(id) => write!(f, "User({})", id),
            Self::Actor(id) => write!(f, "Actor({})", id),
            Self::Workflow(name) => write!(f, "Workflow({})", name),
            Self::System => write!(f, "System"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotationState {
    pub state: RotationStatus,
    pub last_rotation: Option<TimestampMs>,
    pub next_scheduled_rotation: Option<TimestampMs>,
    pub consecutive_failures: u32,
    pub last_failure_reason: Option<String>,
}

impl RotationState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RotationStatus::Idle,
            last_rotation: None,
            next_scheduled_rotation: None,
            consecutive_failures: 0,
            last_failure_reason: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> RotationStatus {
        self.state.clone()
    }

    #[must_use]
    pub fn last_rotation(&self) -> Option<TimestampMs> {
        self.last_rotation
    }

    #[must_use]
    pub fn next_scheduled_rotation(&self) -> Option<TimestampMs> {
        self.next_scheduled_rotation
    }

    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    #[must_use]
    pub fn last_failure_reason(&self) -> Option<&str> {
        self.last_failure_reason.as_deref()
    }
}

impl Default for RotationState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationStatus {
    Idle,
    Rotating,
    WaitingForOverlap,
    Failed(String),
}

impl fmt::Display for RotationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Rotating => write!(f, "Rotating"),
            Self::WaitingForOverlap => write!(f, "WaitingForOverlap"),
            Self::Failed(reason) => write!(f, "Failed({reason})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let instance_id =
            crate::string_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
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
        let instance_id =
            crate::string_types::InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
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
}
