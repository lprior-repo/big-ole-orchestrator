//! Managed effect types for exact-once side effects (ADR-030).
//!
//! Architecture: Data (EffectIntent, EffectKind, EffectRecord, CompensationPolicy)
//!             → Calc (apply_effect_transition, is_terminal, all_variants).
//!
//! This module defines the type system for managed effects flowing through the Engine.
//! No I/O, no engine integration — pure types and state machine logic.

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Lifecycle state of a managed effect (ADR-030).
///
/// Transitions are strictly one-directional: Prepared → Committed | RolledBack.
/// Committed and RolledBack are terminal states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectIntent {
    /// Effect has been prepared but not yet committed.
    Prepared,
    /// Effect has been successfully committed (terminal).
    Committed,
    /// Effect has been rolled back (terminal).
    RolledBack,
}

/// Category of managed side-effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectKind {
    /// HTTP API call (Stripe, external REST, etc.)
    HttpCall,
    /// SQL database query/write.
    SqlQuery,
    /// Blob storage write (S3, GCS, etc.)
    BlobWrite,
}

/// Durable execution receipt for a committed managed connector effect (ADR-041 §4).
///
/// Write-once immutable record produced when a connector commit succeeds.
/// Used for operator audit, replay, and exact-once deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExternalReceipt {
    connector_id: String,
    connector_version: String,
    sink_kind: EffectKind,
    receipt_payload: serde_json::Value,
}

impl ExternalReceipt {
    #[must_use]
    pub fn new(
        connector_id: String,
        connector_version: String,
        sink_kind: EffectKind,
        receipt_payload: serde_json::Value,
    ) -> Option<Self> {
        if connector_id.is_empty() {
            return None;
        }
        Some(Self {
            connector_id,
            connector_version,
            sink_kind,
            receipt_payload,
        })
    }

    #[must_use]
    pub fn connector_id(&self) -> &str {
        &self.connector_id
    }

    #[must_use]
    pub fn connector_version(&self) -> &str {
        &self.connector_version
    }

    #[must_use]
    pub fn sink_kind(&self) -> EffectKind {
        self.sink_kind
    }

    #[must_use]
    pub fn receipt_payload(&self) -> &serde_json::Value {
        &self.receipt_payload
    }
}

/// Compensation policy for an effect (ADR-030 §5, ADR-034).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompensationPolicy {
    /// No compensation needed or available.
    None,
    /// Manual compensation — requires human intervention.
    Manual,
    /// Automatic compensation — engine drives rollback.
    Automatic,
}

/// Event that triggers an effect state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectTransitionEvent {
    /// Commit the effect — transition from Prepared to Committed.
    Commit,
    /// Roll back the effect — transition from Prepared to RolledBack.
    Rollback,
}

impl EffectTransitionEvent {
    /// Returns all transition event variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectTransitionEvent] {
        &[
            EffectTransitionEvent::Commit,
            EffectTransitionEvent::Rollback,
        ]
    }
}

/// Error returned when an effect state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectTransitionError {
    #[error("Cannot transition from terminal effect state")]
    TerminalStateTransition,
    #[error("Invalid effect state transition")]
    InvalidTransition,
}

/// Persisted record of a managed effect.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRecord {
    intent_id: String,
    kind: EffectKind,
    params_json: serde_json::Value,
    status: EffectIntent,
    committed_at: Option<crate::types::TimestampMs>,
}

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl EffectIntent {
    /// Check if this state is terminal (Committed or RolledBack).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, EffectIntent::Committed | EffectIntent::RolledBack)
    }

    /// Returns all EffectIntent variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectIntent] {
        &[
            EffectIntent::Prepared,
            EffectIntent::Committed,
            EffectIntent::RolledBack,
        ]
    }
}

impl EffectKind {
    /// Returns all EffectKind variants.
    #[must_use]
    pub const fn all_variants() -> &'static [EffectKind] {
        &[
            EffectKind::HttpCall,
            EffectKind::SqlQuery,
            EffectKind::BlobWrite,
        ]
    }
}

impl CompensationPolicy {
    /// Returns all CompensationPolicy variants.
    #[must_use]
    pub const fn all_variants() -> &'static [CompensationPolicy] {
        &[
            CompensationPolicy::None,
            CompensationPolicy::Manual,
            CompensationPolicy::Automatic,
        ]
    }
}

impl EffectRecord {
    /// Construct a new EffectRecord.
    ///
    /// Returns `None` if `intent_id` is empty (INV-EFF-003).
    #[must_use]
    pub fn new(
        intent_id: String,
        kind: EffectKind,
        params_json: serde_json::Value,
        status: EffectIntent,
        committed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if intent_id.is_empty() {
            return None;
        }
        Some(Self {
            intent_id,
            kind,
            params_json,
            status,
            committed_at,
        })
    }

    #[must_use]
    pub fn intent_id(&self) -> &str {
        &self.intent_id
    }

    #[must_use]
    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub fn params_json(&self) -> &serde_json::Value {
        &self.params_json
    }

    #[must_use]
    pub fn status(&self) -> EffectIntent {
        self.status
    }

    #[must_use]
    pub fn committed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.committed_at.as_ref()
    }
}

/// Apply a state transition to an EffectIntent.
///
/// # Errors
///
/// Returns `EffectTransitionError::TerminalStateTransition` if the current state
/// is Committed or RolledBack (INV-EFF-002).
/// Returns `EffectTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
pub fn apply_effect_transition(
    current: EffectIntent,
    event: EffectTransitionEvent,
) -> Result<EffectIntent, EffectTransitionError> {
    match (current, event) {
        // Valid transitions (INV-EFF-001): one-directional from Prepared
        (EffectIntent::Prepared, EffectTransitionEvent::Commit) => Ok(EffectIntent::Committed),
        (EffectIntent::Prepared, EffectTransitionEvent::Rollback) => Ok(EffectIntent::RolledBack),

        // Terminal states reject all transitions (INV-EFF-002)
        (EffectIntent::Committed | EffectIntent::RolledBack, _) => {
            Err(EffectTransitionError::TerminalStateTransition)
        }
    }
}

// ============================================================================
// Unit Tests

// ===========================================================================
// Receipt Type (ADR-041 §4: Receipts and Identity)
// ===========================================================================

/// Durable execution receipt for a committed managed connector effect.
/// Write-once immutable record produced when a connector commit succeeds.
/// Used for operator audit, replay, and exact-once deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Receipt {
    effect_id: String,
    connector_type: String,
    connector_version: String,
    external_receipt: serde_json::Value,
    committed_at: crate::types::TimestampMs,
}

impl Receipt {
    /// Construct a new Receipt. Returns None if effect_id or connector_type is empty.
    #[must_use]
    pub fn new(
        effect_id: String,
        connector_type: String,
        connector_version: String,
        external_receipt: serde_json::Value,
        committed_at: crate::types::TimestampMs,
    ) -> Option<Self> {
        if effect_id.is_empty() || connector_type.is_empty() {
            return None;
        }
        Some(Self {
            effect_id,
            connector_type,
            connector_version,
            external_receipt,
            committed_at,
        })
    }

    #[must_use]
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }
    #[must_use]
    pub fn connector_type(&self) -> &str {
        &self.connector_type
    }
    #[must_use]
    pub fn connector_version(&self) -> &str {
        &self.connector_version
    }
    #[must_use]
    pub fn external_receipt(&self) -> &serde_json::Value {
        &self.external_receipt
    }
    #[must_use]
    pub fn committed_at(&self) -> crate::types::TimestampMs {
        self.committed_at
    }
}

impl std::fmt::Display for Receipt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Receipt(effect={}, connector={}:{}, at={})",
            self.effect_id, self.connector_type, self.connector_version, self.committed_at
        )
    }
}
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    // ========================================================================
    // EffectIntent Derive Tests
    // ========================================================================

    #[test]
    fn effectintent_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", EffectIntent::Prepared), "Prepared");
        assert_eq!(format!("{:?}", EffectIntent::Committed), "Committed");
        assert_eq!(format!("{:?}", EffectIntent::RolledBack), "RolledBack");
    }

    #[test]
    fn effectintent_clone_copy_semantics() {
        let state = EffectIntent::Prepared;
        let copy = state;
        assert_eq!(state, copy);

        let state1 = EffectIntent::Committed;
        let state2 = state1;
        assert_eq!(state1, state2);
    }

    #[test]
    fn effectintent_partial_eq_and_hash() {
        assert_eq!(EffectIntent::Prepared, EffectIntent::Prepared);
        assert_ne!(EffectIntent::Prepared, EffectIntent::Committed);
        assert_ne!(EffectIntent::Committed, EffectIntent::RolledBack);

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h1 = DefaultHasher::new();
        EffectIntent::Prepared.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        EffectIntent::Prepared.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // ========================================================================
    // EffectIntent Serde Round-Trip
    // ========================================================================

    #[rstest]
    #[case(EffectIntent::Prepared, "Prepared")]
    #[case(EffectIntent::Committed, "Committed")]
    #[case(EffectIntent::RolledBack, "RolledBack")]
    fn effectintent_serializes_and_deserializes_for_all_variants(
        #[case] variant: EffectIntent,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: EffectIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    // ========================================================================
    // apply_effect_transition — Happy Paths
    // ========================================================================

    #[test]
    fn apply_effect_transition_returns_committed_when_prepared_commit() {
        let result = apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Commit);
        assert_eq!(result, Ok(EffectIntent::Committed));
    }

    #[test]
    fn apply_effect_transition_returns_rolledback_when_prepared_rollback() {
        let result =
            apply_effect_transition(EffectIntent::Prepared, EffectTransitionEvent::Rollback);
        assert_eq!(result, Ok(EffectIntent::RolledBack));
    }

    // ========================================================================
    // apply_effect_transition — Terminal Rejections (INV-EFF-002)
    // ========================================================================

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_committed_commit() {
        let result =
            apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Commit);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_committed_rollback() {
        let result =
            apply_effect_transition(EffectIntent::Committed, EffectTransitionEvent::Rollback);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_rolledback_commit() {
        let result =
            apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Commit);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    #[test]
    fn apply_effect_transition_returns_terminal_error_when_rolledback_rollback() {
        let result =
            apply_effect_transition(EffectIntent::RolledBack, EffectTransitionEvent::Rollback);
        assert_eq!(result, Err(EffectTransitionError::TerminalStateTransition));
    }

    // ========================================================================
    // EffectIntent::is_terminal
    // ========================================================================

    #[test]
    fn effectintent_is_terminal_returns_false_when_prepared() {
        assert!(!EffectIntent::Prepared.is_terminal());
    }

    #[test]
    fn effectintent_is_terminal_returns_true_when_committed() {
        assert!(EffectIntent::Committed.is_terminal());
    }

    #[test]
    fn effectintent_is_terminal_returns_true_when_rolledback() {
        assert!(EffectIntent::RolledBack.is_terminal());
    }

    // ========================================================================
    // EffectIntent::all_variants
    // ========================================================================

    #[test]
    fn effectintent_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectIntent::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectIntent::Prepared);
        assert_eq!(variants[1], EffectIntent::Committed);
        assert_eq!(variants[2], EffectIntent::RolledBack);
    }

    // ========================================================================
    // EffectTransitionEvent Tests
    // ========================================================================

    #[test]
    fn effect_transition_event_all_variants_returns_two_events() {
        let variants = EffectTransitionEvent::all_variants();
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0], EffectTransitionEvent::Commit);
        assert_eq!(variants[1], EffectTransitionEvent::Rollback);
    }

    // ========================================================================
    // EffectKind Derive + Serde Tests
    // ========================================================================

    #[test]
    fn effectkind_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", EffectKind::HttpCall), "HttpCall");
        assert_eq!(format!("{:?}", EffectKind::SqlQuery), "SqlQuery");
        assert_eq!(format!("{:?}", EffectKind::BlobWrite), "BlobWrite");
    }

    #[rstest]
    #[case(EffectKind::HttpCall, "HttpCall")]
    #[case(EffectKind::SqlQuery, "SqlQuery")]
    #[case(EffectKind::BlobWrite, "BlobWrite")]
    fn effectkind_serializes_and_deserializes_for_all_variants(
        #[case] variant: EffectKind,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: EffectKind = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn effectkind_all_variants_returns_three_variants_in_declaration_order() {
        let variants = EffectKind::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], EffectKind::HttpCall);
        assert_eq!(variants[1], EffectKind::SqlQuery);
        assert_eq!(variants[2], EffectKind::BlobWrite);
    }

    // ========================================================================
    // CompensationPolicy Derive + Serde Tests
    // ========================================================================

    #[test]
    fn compensationpolicy_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", CompensationPolicy::None), "None");
        assert_eq!(format!("{:?}", CompensationPolicy::Manual), "Manual");
        assert_eq!(format!("{:?}", CompensationPolicy::Automatic), "Automatic");
    }

    #[rstest]
    #[case(CompensationPolicy::None, "None")]
    #[case(CompensationPolicy::Manual, "Manual")]
    #[case(CompensationPolicy::Automatic, "Automatic")]
    fn compensationpolicy_serializes_and_deserializes_for_all_variants(
        #[case] variant: CompensationPolicy,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: CompensationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn compensationpolicy_all_variants_returns_three_variants_in_declaration_order() {
        let variants = CompensationPolicy::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], CompensationPolicy::None);
        assert_eq!(variants[1], CompensationPolicy::Manual);
        assert_eq!(variants[2], CompensationPolicy::Automatic);
    }

    // ========================================================================
    // EffectRecord Construction
    // ========================================================================

    #[test]
    fn effectrecord_returns_some_when_constructed_with_typical_components() {
        let record = EffectRecord::new(
            "fx-123".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.stripe.com/v1/charges"}),
            EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.intent_id(), "fx-123");
        assert_eq!(r.kind(), EffectKind::HttpCall);
        assert_eq!(
            r.params_json(),
            &json!({"url": "https://api.stripe.com/v1/charges"})
        );
        assert_eq!(r.status(), EffectIntent::Prepared);
        assert_eq!(r.committed_at(), None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_single_char_intent_id() {
        let record = EffectRecord::new(
            "a".to_string(),
            EffectKind::SqlQuery,
            json!({"query": "SELECT 1"}),
            EffectIntent::Prepared,
            None,
        );
        assert!(record.is_some());
        assert_eq!(record.unwrap().intent_id(), "a");
    }

    #[test]
    fn effectrecord_returns_none_when_intent_id_is_empty() {
        let result = EffectRecord::new(
            "".to_string(),
            EffectKind::HttpCall,
            json!({}),
            EffectIntent::Prepared,
            None,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn effectrecord_returns_some_when_constructed_with_committed_status_and_timestamp() {
        let ts = crate::types::TimestampMs(1234);
        let record = EffectRecord::new(
            "fx-456".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "my-bucket", "key": "obj"}),
            EffectIntent::Committed,
            Some(ts),
        );
        assert!(record.is_some());
        let r = record.unwrap();
        assert_eq!(r.status(), EffectIntent::Committed);
        assert_eq!(r.committed_at(), Some(&ts));
    }

    #[test]
    fn effectrecord_serializes_and_deserializes_via_json_round_trip() {
        let record = EffectRecord::new(
            "fx-789".to_string(),
            EffectKind::HttpCall,
            json!({"method": "POST", "url": "https://example.com"}),
            EffectIntent::Prepared,
            None,
        );
        let r = record.unwrap();
        let json = serde_json::to_string(&r).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, r);
    }

    // ========================================================================
    // EffectTransitionError Tests
    // ========================================================================

    #[test]
    fn effect_transition_error_terminal_state_transition_displays_correct_message() {
        let err = EffectTransitionError::TerminalStateTransition;
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal effect state"
        );
    }

    #[test]
    fn effect_transition_error_invalid_transition_displays_correct_message() {
        let err = EffectTransitionError::InvalidTransition;
        assert_eq!(err.to_string(), "Invalid effect state transition");
    }

    #[test]
    fn effect_transition_error_implements_std_error_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(EffectTransitionError::TerminalStateTransition);
        assert_eq!(
            err.to_string(),
            "Cannot transition from terminal effect state"
        );
    }
}

// ============================================================================
// Proptest Invariants
// ============================================================================

#[cfg(feature = "proptest")]
#[allow(clippy::unwrap_used)]
mod proptests {
    use super::*;
    use proptest::prop_assert;
    use proptest::prop_assert_eq;

    proptest::proptest! {
        /// INV: Serde round-trip preserves EffectIntent equality for all variants.
        #[test]
        fn effectintent_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(&[
                EffectIntent::Prepared,
                EffectIntent::Committed,
                EffectIntent::RolledBack,
            ])
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: EffectIntent = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: Serde round-trip preserves EffectKind equality for all variants.
        #[test]
        fn effectkind_serde_roundtrip_preserves_equality(
            variant in proptest::sample::select(&[
                EffectKind::HttpCall,
                EffectKind::SqlQuery,
                EffectKind::BlobWrite,
            ])
        ) {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: EffectKind = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(variant, recovered);
        }

        /// INV: EffectRecord field immutability — accessors return construction values.
        #[test]
        fn effectrecord_accessors_return_construction_values(
            id in "[a-zA-Z0-9_-]{1,100}",
            kind_idx in 0usize..3,
            status_idx in 0usize..3,
        ) {
            let kinds = [EffectKind::HttpCall, EffectKind::SqlQuery, EffectKind::BlobWrite];
            let statuses = [EffectIntent::Prepared, EffectIntent::Committed, EffectIntent::RolledBack];
            let kind = kinds[kind_idx];
            let status = statuses[status_idx];
            let params = serde_json::json!({"test": "value"});
            let ts = crate::types::TimestampMs(42);

            let record = EffectRecord::new(id.clone(), kind, params.clone(), status, Some(ts));
            prop_assert!(record.is_some());
            let r = record.unwrap();
            prop_assert_eq!(r.intent_id(), id);
            prop_assert_eq!(r.kind(), kind);
            prop_assert_eq!(r.status(), status);
        }
    }
}

// ============================================================================
// Kani Verification Harnesses
// ============================================================================

#[cfg(kani)]
mod verification {
    use super::*;

    /// K-01: Verify apply_effect_transition exhaustiveness.
    /// All 3×2 = 6 combinations must be covered without panic.
    #[kani::proof]
    fn verify_effect_transition_exhaustiveness() {
        let state: u8 = kani::any();
        let event: u8 = kani::any();
        kani::assume(state < 3);
        kani::assume(event < 2);

        let current = match state {
            0 => EffectIntent::Prepared,
            1 => EffectIntent::Committed,
            _ => EffectIntent::RolledBack,
        };
        let evt = match event {
            0 => EffectTransitionEvent::Commit,
            _ => EffectTransitionEvent::Rollback,
        };

        // Must not panic — all combinations handled
        let _ = apply_effect_transition(current, evt);
    }

    /// K-02: Verify EffectRecord::new rejects empty intent_id.
    #[kani::proof]
    fn verify_effect_record_rejects_empty_intent_id() {
        let intent_id = String::new();
        let result = EffectRecord::new(
            intent_id,
            EffectKind::HttpCall,
            serde_json::Value::Null,
            EffectIntent::Prepared,
            None,
        );
        assert!(result.is_none());
    }
}
