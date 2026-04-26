//! SignalScope — Signal node scope metadata for deterministic matching (ADR-031, ADR-042).
//!
//! SignalScope captures the signal matching and dedupe scope requirements for Signal/Wait
//! nodes in the canonical WorkflowSpec. This enables the Engine to deterministically match
//! signals to waiting workflows using wait_key-based routing.

use serde::{Deserialize, Serialize};

use super::{BufferPolicy, WaitKey};

/// Signal scope metadata for Signal/Wait nodes in a WorkflowSpec.
///
/// Per ADR-031 Section 2.7 and ADR-042, this type encodes the signal matching
/// dimensions needed for deterministic wait-key routing and dedupe semantics.
///
/// For non-signal nodes (Pure, ManagedEffect, Unsafe), this field is `None` in
/// the canonical spec — only Signal and Wait nodes carry signal_scope metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalScope {
    /// The wait_key used for deterministic signal-to-wait matching.
    ///
    /// Per ADR-042 Section 3, a signal may only resume a workflow if the active
    /// epoch is currently waiting on a matching wait_key.
    pub wait_key: WaitKey,
    /// Determines signal buffering behavior when no matching wait is active.
    ///
    /// Per ADR-042 Section 3, this controls whether unmatched signals are rejected
    /// or buffered (BufferOne/BufferMany).
    #[serde(default)]
    pub buffer_policy: BufferPolicy,
}

impl SignalScope {
    /// Create a new SignalScope with the given wait_key and default Reject buffer policy.
    #[must_use]
    pub fn new(wait_key: WaitKey) -> Self {
        Self {
            wait_key,
            buffer_policy: BufferPolicy::default(),
        }
    }

    /// Create a new SignalScope with the given wait_key and buffer_policy.
    #[must_use]
    pub fn with_buffer_policy(mut self, policy: BufferPolicy) -> Self {
        self.buffer_policy = policy;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_scope_serializes_to_json_with_wait_key() {
        let scope = SignalScope::new(WaitKey::parse("approval").unwrap());
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("\"wait_key\":\"approval\""));
    }

    #[test]
    fn signal_scope_serializes_buffer_policy_when_present() {
        let scope = SignalScope::new(WaitKey::parse("timer").unwrap())
            .with_buffer_policy(BufferPolicy::BufferOne);
        let json = serde_json::to_string(&scope).unwrap();
        assert!(json.contains("\"buffer_policy\":\"buffer_one\""));
    }

    #[test]
    fn signal_scope_default_buffer_policy_is_reject() {
        let scope = SignalScope::new(WaitKey::parse("approval").unwrap());
        assert_eq!(scope.buffer_policy, BufferPolicy::Reject);
    }

    #[test]
    fn signal_scope_round_trips_via_serde() {
        let scope = SignalScope::new(WaitKey::parse("webhook").unwrap())
            .with_buffer_policy(BufferPolicy::BufferMany);
        let json = serde_json::to_string(&scope).unwrap();
        let recovered: SignalScope = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, scope);
    }

    #[test]
    fn signal_scope_wait_key_rejects_empty_string() {
        let result = SignalScope::new(WaitKey::parse(""));
        assert!(result.is_err());
    }
}
