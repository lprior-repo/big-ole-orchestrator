//! ID newtypes and validation for wtf-engine.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Error returned when an ID contains characters that are illegal in NATS subjects.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("ID contains NATS-illegal characters (., >, *, whitespace): {0:?}")]
pub struct InvalidNatsId(pub String);

/// Return `Ok` if `s` contains no NATS subject delimiter characters.
fn validate_nats_component(s: &str) -> Result<(), InvalidNatsId> {
    match s
        .chars()
        .find(|c| matches!(c, '.' | '>' | '*') || c.is_whitespace())
    {
        None => Ok(()),
        Some(_) => Err(InvalidNatsId(s.to_owned())),
    }
}

/// Unique identifier for a workflow instance (ULID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(pub String);

impl InstanceId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Construct an instance id after validating it is safe for NATS subject usage.
    ///
    /// # Errors
    /// Returns [`InvalidNatsId`] when the id contains `.`, `>`, `*`, or whitespace.
    pub fn try_new(id: impl Into<String>) -> Result<Self, InvalidNatsId> {
        let s = id.into();
        validate_nats_component(&s).map(|()| Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for InstanceId {
    type Err = InvalidNatsId;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

/// Logical namespace grouping workflow instances (e.g. "payments").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(pub String);

impl NamespaceId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Construct a namespace id after validating it is safe for NATS subject usage.
    ///
    /// # Errors
    /// Returns [`InvalidNatsId`] when the namespace contains `.`, `>`, `*`, or whitespace.
    pub fn try_new(id: impl Into<String>) -> Result<Self, InvalidNatsId> {
        let s = id.into();
        validate_nats_component(&s).map(|()| Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for NamespaceId {
    type Err = InvalidNatsId;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

/// Unique identifier for an activity invocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivityId(pub String);

impl ActivityId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn procedural(instance_id: &InstanceId, op_counter: u32) -> Self {
        Self(format!("{}:{}", instance_id.as_str(), op_counter))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActivityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for ActivityId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

/// Unique identifier for a timer (ULID).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimerId(pub String);

impl TimerId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Create a deterministic timer ID for a procedural workflow sleep operation.
    /// Format: `"{instance_id}:t:{op_id}"` — stable across restarts for the same op.
    #[must_use]
    pub fn procedural(instance_id: &InstanceId, op_id: u32) -> Self {
        Self(format!("{}:t:{}", instance_id.as_str(), op_id))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TimerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for TimerId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

#[cfg(test)]
mod timer_id_determinism_tests {
    use super::*;

    /// `TimerId::procedural` must produce a deterministic id from `instance_id` + `op_id`.
    /// Bug: `handle_sleep` uses `ulid::Ulid::new()` — different on each restart.
    /// Fix: `TimerId` must derive from (`instance_id`, `op_id`) so restarts reproduce the same id.
    #[test]
    fn timer_id_procedural_is_deterministic_for_same_op_id() {
        let instance_id = InstanceId::new("inst-01");
        let op_id = 3u32;

        let id1 = TimerId::procedural(&instance_id, op_id);
        let id2 = TimerId::procedural(&instance_id, op_id);

        assert_eq!(
            id1, id2,
            "same op_id must yield the same timer_id on every call"
        );
        assert_eq!(id1.as_str(), "inst-01:t:3");
    }

    #[test]
    fn timer_id_procedural_differs_for_different_op_ids() {
        let instance_id = InstanceId::new("inst-01");
        assert_ne!(
            TimerId::procedural(&instance_id, 0),
            TimerId::procedural(&instance_id, 1),
        );
    }
}
