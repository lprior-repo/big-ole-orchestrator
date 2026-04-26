//! Semantic newtypes for domain identifiers.
//!
//! Each type wraps a primitive to eliminate ambiguity at the type level
//! (Scott Wlaschin DDD: "Types as documentation").

use std::hash::Hash;

/// Semantic type: Worker node identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeName(String);

impl NodeName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic type: Timer identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

impl TimerId {
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    pub fn inner(&self) -> u64 {
        self.0
    }
}

/// Semantic type: Attempt number (1-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptNumber(u32);

impl AttemptNumber {
    #[must_use]
    pub fn new(num: u32) -> Option<Self> {
        if num == 0 {
            None
        } else {
            Some(Self(num))
        }
    }

    #[must_use]
    pub fn inner(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstanceState {
    pub counter: u64,
    /// Pinned binary hash for this instance (ADR-017, ADR-027).
    /// Set when the instance starts from the WorkflowStarted event.
    /// Remains immutable even if the workflow is redeployed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_hash: Option<String>,
}

impl Default for InstanceState {
    fn default() -> Self {
        Self {
            counter: 0,
            binary_hash: None,
        }
    }
}
