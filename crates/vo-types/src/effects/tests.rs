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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectCompressionError {
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),
}

/// Persisted record of a managed effect.
///
/// Schema evolution guarantees (ADR-035 alignment):
/// - `committed_at` has `#[serde(default)]` so records persisted before this
///   field was added (or when it was `None`) deserialize correctly.
/// - Unknown fields are silently ignored (forward compatibility: old code can
///   read records written by newer versions without error).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRecord {
    intent_id: String,
    kind: EffectKind,
    params_json: serde_json::Value,
    status: EffectIntent,
    #[serde(default)]
    committed_at: Option<crate::types::TimestampMs>,
}

/// Error returned when effect idempotency key validation fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectIdempotencyError {
    #[error("Empty idempotency key not permitted")]
    EmptyKey,
    #[error("Idempotency key exceeds maximum length of {max} characters (got {actual})")]
    KeyTooLong { max: usize, actual: usize },
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
    /// Maximum length for effect idempotency keys.
    pub const MAX_IDEMPOTENCY_KEY_LEN: usize = 256;

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

    /// Validate an idempotency key for effect records.
    ///
    /// Returns `Ok(())` if the key is valid, or `EffectIdempotencyError` if invalid.
    ///
    /// # Rules
    ///
    /// - Keys must be non-empty
    /// - Keys must not exceed `MAX_IDEMPOTENCY_KEY_LEN` characters
    /// - Same key can be reused across different EffectKind variants (per-type uniqueness)
    #[must_use]
    pub fn validate_idempotency_key(key: &str) -> Result<(), EffectIdempotencyError> {
        if key.is_empty() {
            return Err(EffectIdempotencyError::EmptyKey);
        }
        if key.len() > Self::MAX_IDEMPOTENCY_KEY_LEN {
            return Err(EffectIdempotencyError::KeyTooLong {
                max: Self::MAX_IDEMPOTENCY_KEY_LEN,
                actual: key.len(),
            });
        }
        Ok(())
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

    pub fn compress(&self) -> Result<Vec<u8>, EffectCompressionError> {
        let json = serde_json::to_string(self)
            .map_err(|e| EffectCompressionError::SerializationFailed(e.to_string()))?;
        let bytes = json.as_bytes();
        zstd::encode_all(bytes, 0)
            .map_err(|e| EffectCompressionError::CompressionFailed(e.to_string()))
    }

    pub fn decompress(compressed: &[u8]) -> Result<Self, EffectCompressionError> {
        let decompressed = zstd::decode_all(compressed)
            .map_err(|e| EffectCompressionError::DecompressionFailed(e.to_string()))?;
        let json = String::from_utf8(decompressed)
            .map_err(|e| EffectCompressionError::DecompressionFailed(e.to_string()))?;
        serde_json::from_str(&json)
            .map_err(|e| EffectCompressionError::DeserializationFailed(e.to_string()))
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
    use super::super::transitions::*;
    use super::super::types::*;
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
    // Schema Evolution Tests
    // ========================================================================

    /// Old-format record (no `committed_at` field) deserializes correctly.
    /// This simulates records persisted before `committed_at` was added or when
    /// a future version drops the field. The `#[serde(default)]` ensures it
    /// becomes `None`.
    #[test]
    fn schema_evolution_old_format_without_committed_at_deserializes() {
        let old_json = r#"{
            "intent_id": "fx-old-1",
            "kind": "HttpCall",
            "params_json": {"url": "https://legacy.example.com"},
            "status": "Prepared"
        }"#;
        let record: EffectRecord = serde_json::from_str(old_json).unwrap();
        assert_eq!(record.intent_id(), "fx-old-1");
        assert_eq!(record.kind(), EffectKind::HttpCall);
        assert_eq!(record.status(), EffectIntent::Prepared);
        assert_eq!(record.committed_at(), None);
    }

    /// Old-format record with all statuses deserializes without `committed_at`.
    #[test]
    fn schema_evolution_old_format_all_statuses() {
        for (status_str, expected) in [
            ("\"Prepared\"", EffectIntent::Prepared),
            ("\"Committed\"", EffectIntent::Committed),
            ("\"RolledBack\"", EffectIntent::RolledBack),
        ] {
            let old_json = format!(
                r#"{{"intent_id": "fx-old-status", "kind": "SqlQuery", "params_json": {{}}, "status": {status_str}}}"#
            );
            let record: EffectRecord = serde_json::from_str(&old_json).unwrap();
            assert_eq!(record.status(), expected);
            assert_eq!(record.committed_at(), None);
        }
    }

    /// New-format record with all fields including `committed_at` deserializes.
    #[test]
    fn schema_evolution_new_format_with_committed_at_deserializes() {
        let new_json = r#"{
            "intent_id": "fx-new-1",
            "kind": "BlobWrite",
            "params_json": {"bucket": "data"},
            "status": "Committed",
            "committed_at": 1700000000
        }"#;
        let record: EffectRecord = serde_json::from_str(new_json).unwrap();
        assert_eq!(record.intent_id(), "fx-new-1");
        assert_eq!(record.kind(), EffectKind::BlobWrite);
        assert_eq!(record.status(), EffectIntent::Committed);
        assert!(record.committed_at().is_some());
    }

    /// Forward compatibility: unknown fields from a future version are ignored.
    /// Old code reading records written by a newer version must not break.
    #[test]
    fn schema_evolution_unknown_fields_ignored() {
        let future_json = r#"{
            "intent_id": "fx-future-1",
            "kind": "HttpCall",
            "params_json": {},
            "status": "Prepared",
            "committed_at": null,
            "future_field_a": "some value",
            "future_field_b": 42,
            "nested_future": {"deep": [1, 2, 3]}
        }"#;
        let record: EffectRecord = serde_json::from_str(future_json).unwrap();
        assert_eq!(record.intent_id(), "fx-future-1");
        assert_eq!(record.kind(), EffectKind::HttpCall);
        assert_eq!(record.status(), EffectIntent::Prepared);
        assert_eq!(record.committed_at(), None);
    }

    /// Round-trip preserves `committed_at: Some(...)` across serialize/deserialize.
    #[test]
    fn schema_evolution_roundtrip_with_committed_at_some() {
        let ts = crate::types::TimestampMs(1700000000);
        let record = EffectRecord::new(
            "fx-rt-ts".to_string(),
            EffectKind::SqlQuery,
            json!({"q": "SELECT 1"}),
            EffectIntent::Committed,
            Some(ts),
        )
        .unwrap();
        let json = serde_json::to_string(&record).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.committed_at(), Some(&ts));
    }

    /// Round-trip preserves `committed_at: None` — serialized JSON omits it
    /// when using serde default, or includes null. Either way, deserialization
    /// must yield None.
    #[test]
    fn schema_evolution_roundtrip_with_committed_at_none() {
        let record = EffectRecord::new(
            "fx-rt-none".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "b"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let json = serde_json::to_string(&record).unwrap();
        let recovered: EffectRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.committed_at(), None);
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

    // ========================================================================
    // EffectIdempotencyError Tests
    // ========================================================================

    #[test]
    fn effect_idempotency_error_empty_key_displays_correct_message() {
        let err = EffectIdempotencyError::EmptyKey;
        assert_eq!(err.to_string(), "Empty idempotency key not permitted");
    }

    #[test]
    fn effect_idempotency_error_key_too_long_displays_correct_message() {
        let err = EffectIdempotencyError::KeyTooLong {
            max: 256,
            actual: 512,
        };
        assert_eq!(
            err.to_string(),
            "Idempotency key exceeds maximum length of 256 characters (got 512)"
        );
    }

    // ========================================================================
    // EffectRecord::validate_idempotency_key Tests
    // ========================================================================

    #[test]
    fn validate_idempotency_key_accepts_non_empty_key() {
        assert!(EffectRecord::validate_idempotency_key("valid-key").is_ok());
        assert!(EffectRecord::validate_idempotency_key("a").is_ok());
        assert!(EffectRecord::validate_idempotency_key("key_with_123_numbers").is_ok());
    }

    #[test]
    fn validate_idempotency_key_rejects_empty_key() {
        let result = EffectRecord::validate_idempotency_key("");
        assert!(matches!(
            result,
            Err(EffectIdempotencyError::EmptyKey)
        ));
    }

    #[test]
    fn validate_idempotency_key_rejects_key_exceeding_max_length() {
        let too_long_key = "a".repeat(257);
        let result = EffectRecord::validate_idempotency_key(&too_long_key);
        assert!(matches!(
            result,
            Err(EffectIdempotencyError::KeyTooLong {
                max: 256,
                actual: 257
            })
        ));
    }

    #[test]
    fn validate_idempotency_key_accepts_key_at_max_length_boundary() {
        let max_key = "a".repeat(256);
        assert!(EffectRecord::validate_idempotency_key(&max_key).is_ok());
    }

    #[test]
    fn validate_idempotency_key_accepts_single_character_key() {
        assert!(EffectRecord::validate_idempotency_key("x").is_ok());
    }

    #[test]
    fn validate_idempotency_key_accepts_key_with_special_characters() {
        assert!(EffectRecord::validate_idempotency_key("key-123_abc-def").is_ok());
        assert!(EffectRecord::validate_idempotency_key("KEY_UPPERCASE").is_ok());
    }

    // ========================================================================
    // Effect Idempotency Key Uniqueness Tests (Per-Type)
    // ========================================================================

    #[test]
    fn same_intent_id_allowed_across_different_effect_kinds() {
        // Same intent_id should be allowed for different EffectKind variants
        let record1 = EffectRecord::new(
            "fx-same-id".to_string(),
            EffectKind::HttpCall,
            json!({"url": "https://api.example.com"}),
            EffectIntent::Prepared,
            None,
        );
        let record2 = EffectRecord::new(
            "fx-same-id".to_string(),
            EffectKind::SqlQuery,
            json!({"query": "SELECT 1"}),
            EffectIntent::Prepared,
            None,
        );
        let record3 = EffectRecord::new(
            "fx-same-id".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "test", "key": "obj"}),
            EffectIntent::Prepared,
            None,
        );

        assert!(record1.is_some());
        assert!(record2.is_some());
        assert!(record3.is_some());

        let r1 = record1.unwrap();
        let r2 = record2.unwrap();
        let r3 = record3.unwrap();

        assert_eq!(r1.intent_id(), r2.intent_id());
        assert_eq!(r2.intent_id(), r3.intent_id());
        assert_ne!(r1.kind(), r2.kind());
        assert_ne!(r2.kind(), r3.kind());
    }

    #[test]
    fn effect_records_with_same_kind_same_id_same_params_are_equal() {
        let kind = EffectKind::HttpCall;
        let id = "fx-duplicate-test";

        let record1 = EffectRecord::new(
            id.to_string(),
            kind,
            json!({"param": "value1"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let record2 = EffectRecord::new(
            id.to_string(),
            kind,
            json!({"param": "value1"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        // Same all fields should be equal
        assert_eq!(record1, record2);
    }

    #[test]
    fn effect_kind_variant_has_unique_hash_for_same_intent_id() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let id = "fx-hash-test";
        let kind1 = EffectKind::HttpCall;
        let kind2 = EffectKind::SqlQuery;

        let record1 = EffectRecord::new(
            id.to_string(),
            kind1,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let record2 = EffectRecord::new(
            id.to_string(),
            kind2,
            json!({}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();

        let mut h1 = DefaultHasher::new();
        record1.hash(&mut h1);
        let hash1 = h1.finish();

        let mut h2 = DefaultHasher::new();
        record2.hash(&mut h2);
        let hash2 = h2.finish();

        // Different effect kinds should produce different hashes even with same intent_id
        assert_ne!(hash1, hash2);
    }

    // ========================================================================
    // Key Exhaustion Tests
    // ========================================================================

    #[test]
    fn idempotency_key_exhaustion_boundary_at_max_length() {
        let max_key = "a".repeat(256);
        assert!(EffectRecord::validate_idempotency_key(&max_key).is_ok());

        let over_key = "a".repeat(257);
        assert!(matches!(
            EffectRecord::validate_idempotency_key(&over_key),
            Err(EffectIdempotencyError::KeyTooLong {
                max: 256,
                actual: 257
            })
        ));
    }

    #[test]
    fn idempotency_key_empty_string_validation() {
        assert!(matches!(
            EffectRecord::validate_idempotency_key(""),
            Err(EffectIdempotencyError::EmptyKey)
        ));
    }

    #[test]
    fn idempotency_key_with_whitespace_is_valid() {
        // Whitespace is allowed (not restricted by this validator)
        assert!(EffectRecord::validate_idempotency_key("key with spaces").is_ok());
        assert!(EffectRecord::validate_idempotency_key("  leading").is_ok());
        assert!(EffectRecord::validate_idempotency_key("trailing  ").is_ok());
    }

    #[test]
    fn idempotency_key_unicode_is_valid() {
        // Unicode characters are allowed (not restricted by this validator)
        assert!(EffectRecord::validate_idempotency_key("key-émoji-🎉").is_ok());
        assert!(EffectRecord::validate_idempotency_key("日本語キー").is_ok());
    }

    #[test]
    fn max_idempotency_key_len_constant_is_exposed() {
        assert_eq!(EffectRecord::MAX_IDEMPOTENCY_KEY_LEN, 256);
    }
}

    // ========================================================================
    // EffectRecord Compression — Round-Trip Correctness
    // ========================================================================

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

        /// INV: Effect idempotency key validation accepts any non-empty string <= MAX_LEN.
        #[test]
        fn idempotency_key_validation_accepts_valid_keys(
            key in ".{1,256}"
        ) {
            prop_assert!(EffectRecord::validate_idempotency_key(&key).is_ok());
        }

        /// INV: Effect idempotency key validation rejects empty strings.
        #[test]
        fn idempotency_key_validation_rejects_empty_key() {
            prop_assert!(matches!(
                EffectRecord::validate_idempotency_key(""),
                Err(EffectIdempotencyError::EmptyKey)
            ));
        }

        /// INV: Effect idempotency key validation rejects strings > MAX_LEN.
        #[test]
        fn idempotency_key_validation_rejects_keys_exceeding_max_length(
            key in ".{257,1000}"
        ) {
            prop_assert!(matches!(
                EffectRecord::validate_idempotency_key(&key),
                Err(EffectIdempotencyError::KeyTooLong {
                    max: 256,
                    actual: len
                }) if len == key.len()
            ));
        }

        /// INV: Same intent_id can be used with different EffectKind variants.
        #[test]
        fn idempotency_key_uniqueness_per_effect_kind(
            id in "[a-zA-Z0-9_-]{1,50}",
            kind_idx in 0usize..3,
        ) {
            let kinds = [EffectKind::HttpCall, EffectKind::SqlQuery, EffectKind::BlobWrite];
            let kind = kinds[kind_idx];

            let record = EffectRecord::new(
                id.clone(),
                kind,
                serde_json::json!({}),
                EffectIntent::Prepared,
                None,
            );

            prop_assert!(record.is_some());
            let r = record.unwrap();
            prop_assert_eq!(r.intent_id(), id);
            prop_assert_eq!(r.kind(), kind);
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
            json!({"url": "https://api.example.com"}),
            status,
            committed_at,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    #[rstest]
    #[case(EffectKind::HttpCall)]
    #[case(EffectKind::SqlQuery)]
    #[case(EffectKind::BlobWrite)]
    fn effectrecord_compress_decompress_roundtrip_preserves_all_kinds(
        #[case] kind: EffectKind,
    ) {
        let record = EffectRecord::new(
            "fx-test-kind".to_string(),
            kind,
            json!({"query": "SELECT * FROM table"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    #[test]
    fn effectrecord_compress_decompress_roundtrip_with_complex_params() {
        let record = EffectRecord::new(
            "fx-complex".to_string(),
            EffectKind::HttpCall,
            json!({
                "headers": {"Authorization": "Bearer token123", "Content-Type": "application/json"},
                "body": {"items": [{"id": 1, "name": "first"}, {"id": 2, "name": "second"}]},
                "timeout_ms": 5000
            }),
            EffectIntent::Committed,
            Some(crate::types::TimestampMs(9999)),
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let decompressed = EffectRecord::decompress(&compressed).unwrap();
        assert_eq!(decompressed, record);
    }

    // ========================================================================
    // EffectRecord Compression — Ratio Verification
    // ========================================================================

    #[test]
    fn effectrecord_compression_reduces_size_for_text_heavy_payload() {
        let record = EffectRecord::new(
            "fx-large".to_string(),
            EffectKind::HttpCall,
            json!({
                "body": "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(100)
            }),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let json_bytes = serde_json::to_vec(&record).unwrap();
        let compressed = record.compress().unwrap();
        assert!(
            compressed.len() < json_bytes.len(),
            "Compressed size ({}) should be smaller than JSON size ({})",
            compressed.len(),
            json_bytes.len()
        );
    }

    #[test]
    fn effectrecord_compression_still_smaller_for_small_records() {
        let record = EffectRecord::new(
            "a".to_string(),
            EffectKind::SqlQuery,
            json!({"q": "x"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let json_bytes = serde_json::to_vec(&record).unwrap();
        let compressed = record.compress().unwrap();
        assert!(
            compressed.len() < json_bytes.len(),
            "Even small records should compress (compressed: {}, json: {})",
            compressed.len(),
            json_bytes.len()
        );
    }

    // ========================================================================
    // EffectRecord Compression — Decompression Error Handling
    // ========================================================================

    #[test]
    fn effectrecord_decompress_returns_error_for_empty_input() {
        let result = EffectRecord::decompress(&[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_random_bytes() {
        let result = EffectRecord::decompress(b"not zstd compressed data at all");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_truncated_zstd() {
        let record = EffectRecord::new(
            "fx-trunc".to_string(),
            EffectKind::BlobWrite,
            json!({"bucket": "test"}),
            EffectIntent::Prepared,
            None,
        )
        .unwrap();
        let compressed = record.compress().unwrap();
        let truncated = &compressed[..compressed.len() / 2];
        let result = EffectRecord::decompress(truncated);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_corrupted_zstd() {
        let mut corrupted = vec![0x28, 0xb5, 0x2f, 0xfd];
        corrupted.extend_from_slice(b"invalid zstd frame");
        let result = EffectRecord::decompress(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn effectrecord_decompress_returns_error_for_valid_zstd_invalid_utf8() {
        let valid_zstd_magic = [0x28, 0xb5, 0x2f, 0xfd];
        let result = EffectRecord::decompress(&valid_zstd_magic);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EffectCompressionError::DecompressionFailed(_)
        ));
    }
}
