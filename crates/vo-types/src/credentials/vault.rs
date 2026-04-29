use serde::{Deserialize, Serialize};
use std::fmt;

use crate::string_types;
use crate::TimestampMs;

use super::credential::Credential;
use super::ids::VaultEntryId;
use super::types::RotationStatus;

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
    User(string_types::InstanceId),
    Actor(string_types::SpawnId),
    Workflow(string_types::WorkflowName),
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
