//! Strongly-typed ID wrappers and shared error types for wtf-engine.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── NATS subject safety ───────────────────────────────────────────────────────

/// Error returned when an ID contains characters that are illegal in NATS subjects.
///
/// NATS subject components must not contain `.`, `>`, `*`, or whitespace.
/// A dot in a `NamespaceId` would corrupt the subject `wtf.log.<ns>.<id>` by adding
/// extra segments, causing subscriptions to miss events (ADR-013).
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

// ── ID newtypes ───────────────────────────────────────────────────────────────

/// Unique identifier for a workflow instance (ULID).
///
/// Must not contain NATS subject delimiters (`.`, `>`, `*`, whitespace).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(pub String);

impl InstanceId {
    /// Wrap an existing string as an `InstanceId` without validation.
    ///
    /// Prefer [`InstanceId::try_new`] for untrusted input.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Validated constructor — rejects NATS-illegal characters.
    ///
    /// # Errors
    /// Returns [`InvalidNatsId`] if `id` contains `.`, `>`, `*`, or whitespace.
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
///
/// Used as the first segment in NATS subjects: `wtf.log.<namespace>.<instance_id>`.
/// Must not contain NATS subject delimiters (`.`, `>`, `*`, whitespace).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(pub String);

impl NamespaceId {
    /// Wrap an existing string as a `NamespaceId` without validation.
    ///
    /// Prefer [`NamespaceId::try_new`] for untrusted input.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Validated constructor — rejects NATS-illegal characters.
    ///
    /// # Errors
    /// Returns [`InvalidNatsId`] if `id` contains `.`, `>`, `*`, or whitespace.
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
///
/// For Procedural paradigm: `<instance_id>:<op_counter>` (deterministic).
/// For FSM/DAG paradigms: a ULID assigned at dispatch time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivityId(pub String);

impl ActivityId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Build the deterministic procedural operation ID.
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

// ── Shared error types ────────────────────────────────────────────────────────

/// Top-level error type for wtf-engine public API surface.
#[derive(Debug, Error)]
pub enum WtfError {
    /// NATS `JetStream` publish failed before receiving a `PublishAck` (ADR-015).
    #[error("NATS publish failed: {message}")]
    NatsPublish { message: String },

    /// NATS operation timed out.
    #[error("NATS timeout after {timeout_ms}ms: {operation}")]
    NatsTimeout { operation: String, timeout_ms: u64 },

    /// sled snapshot store I/O failure.
    #[error("sled error: {message}")]
    SledError { message: String },

    /// Replayed log produced different state (non-deterministic workflow — ADR-016).
    #[error("replay divergence at seq {seq}: {detail}")]
    ReplayDivergence { seq: u64, detail: String },

    /// Engine at maximum concurrent instance capacity.
    #[error("capacity exceeded: {running}/{max} instances running")]
    CapacityExceeded { running: usize, max: usize },

    /// No instance found for the given ID.
    #[error("instance not found: {instance_id}")]
    InstanceNotFound { instance_id: String },

    /// Workflow name is empty or contains illegal characters.
    #[error("invalid workflow name: {name:?}")]
    InvalidWorkflowName { name: String },
}

impl WtfError {
    #[must_use]
    pub fn nats_publish(message: impl Into<String>) -> Self {
        Self::NatsPublish {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn nats_timeout(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::NatsTimeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    #[must_use]
    pub fn sled_error(message: impl Into<String>) -> Self {
        Self::SledError {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn replay_divergence(seq: u64, detail: impl Into<String>) -> Self {
        Self::ReplayDivergence {
            seq,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub fn instance_not_found(instance_id: impl Into<String>) -> Self {
        Self::InstanceNotFound {
            instance_id: instance_id.into(),
        }
    }

    #[must_use]
    pub fn invalid_workflow_name(name: impl Into<String>) -> Self {
        Self::InvalidWorkflowName { name: name.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_display_returns_inner() {
        let id = InstanceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn instance_id_from_str_roundtrips() {
        let id: InstanceId = "01ARZ".parse().expect("parse");
        assert_eq!(id.as_str(), "01ARZ");
    }

    #[test]
    fn instance_id_serializes_as_plain_string() {
        let id = InstanceId::new("abc");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, "\"abc\"");
    }

    #[test]
    fn activity_id_procedural_format() {
        let inst = InstanceId::new("01ARZ");
        assert_eq!(ActivityId::procedural(&inst, 7).as_str(), "01ARZ:7");
    }

    #[test]
    fn activity_id_procedural_counter_zero() {
        let inst = InstanceId::new("inst");
        assert_eq!(ActivityId::procedural(&inst, 0).as_str(), "inst:0");
    }

    #[test]
    fn wtf_error_nats_publish_in_display() {
        let err = WtfError::nats_publish("connection refused");
        assert!(err.to_string().contains("connection refused"));
    }

    #[test]
    fn wtf_error_capacity_exceeded_shows_counts() {
        let err = WtfError::CapacityExceeded {
            running: 10,
            max: 10,
        };
        assert!(err.to_string().contains("10/10"));
    }

    #[test]
    fn wtf_error_replay_divergence_shows_seq_and_detail() {
        let err = WtfError::replay_divergence(42, "counter mismatch");
        let msg = err.to_string();
        assert!(
            msg.contains("42") && msg.contains("counter mismatch"),
            "got: {msg}"
        );
    }

    #[test]
    fn wtf_error_instance_not_found_shows_id() {
        let err = WtfError::instance_not_found("01ARZ");
        assert!(err.to_string().contains("01ARZ"));
    }

    #[test]
    fn wtf_error_nats_timeout_shows_op_and_ms() {
        let err = WtfError::nats_timeout("publish", 5_000);
        let msg = err.to_string();
        assert!(
            msg.contains("publish") && msg.contains("5000"),
            "got: {msg}"
        );
    }

    #[test]
    fn all_id_types_hash_consistently() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(InstanceId::new("a"));
        set.insert(InstanceId::new("a"));
        assert_eq!(set.len(), 1);
    }

    // ── NATS component validation (Red Queen survivors) ───────────────────────

    #[test]
    fn namespace_id_rejects_dot() {
        // A dot in namespace → "wtf.log.pay.ments.id" (5 segments, not 4) → misrouted
        assert!(NamespaceId::try_new("pay.ments").is_err());
    }

    #[test]
    fn namespace_id_rejects_wildcard_star() {
        assert!(NamespaceId::try_new("pay*").is_err());
    }

    #[test]
    fn namespace_id_rejects_wildcard_gt() {
        assert!(NamespaceId::try_new("pay>").is_err());
    }

    #[test]
    fn namespace_id_rejects_whitespace() {
        assert!(NamespaceId::try_new("pay ments").is_err());
    }

    #[test]
    fn namespace_id_accepts_valid_slug() {
        assert!(NamespaceId::try_new("payments").is_ok());
        assert!(NamespaceId::try_new("order-processing").is_ok());
        assert!(NamespaceId::try_new("onboarding_v2").is_ok());
    }

    #[test]
    fn instance_id_rejects_dot() {
        // Dots in instance IDs would corrupt subject routing
        assert!(InstanceId::try_new("01ARZ.BAD").is_err());
    }

    #[test]
    fn instance_id_accepts_ulid() {
        // Standard ULID format must always be accepted
        assert!(InstanceId::try_new("01ARZ3NDEKTSV4RRFFQ69G5FAV").is_ok());
    }

    #[test]
    fn namespace_id_from_str_rejects_dot() {
        let result: Result<NamespaceId, _> = "a.b".parse();
        assert!(result.is_err());
    }

    #[test]
    fn instance_id_from_str_accepts_ulid() {
        let result: Result<InstanceId, _> = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse();
        assert!(result.is_ok());
    }
}
