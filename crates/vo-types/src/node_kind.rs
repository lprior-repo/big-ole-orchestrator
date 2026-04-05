//! Node kind classification for workflow nodes (ADR-031).
//!
//! This module defines the type system for classifying workflow nodes
//! by their side-effect profile. No I/O — pure types.

/// Classification of a workflow node by its side-effect profile (ADR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum NodeKind {
    /// Pure computation — no side effects, deterministic.
    Pure,
    /// Managed side-effect — tracked by the effect journal.
    ManagedEffect,
    /// Waits for an external signal or timer.
    Wait,
    /// Emits signal to waiting workflows.
    Signal,
    /// Escape hatch — no guarantees.
    Unsafe,
}

impl NodeKind {
    /// Returns all NodeKind variants in declaration order.
    #[must_use]
    #[allow(dead_code)]
    pub const fn all_variants() -> &'static [NodeKind] {
        &[
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
            NodeKind::Unsafe,
        ]
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_pure_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Pure).unwrap();
        assert_eq!(json, "\"pure\"");
    }

    #[test]
    fn node_kind_managed_effect_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::ManagedEffect).unwrap();
        assert_eq!(json, "\"managed_effect\"");
    }

    #[test]
    fn node_kind_wait_round_trips_via_serde() {
        let json = "\"wait\"";
        let parsed: NodeKind = serde_json::from_str(json).expect("should deserialize 'wait'");
        assert_eq!(parsed, NodeKind::Wait);
        let roundtrip = serde_json::to_string(&parsed).unwrap();
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn node_kind_signal_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Signal).unwrap();
        assert_eq!(json, "\"signal\"");
    }

    #[test]
    fn node_kind_unsafe_serializes_to_snake_case() {
        let json = serde_json::to_string(&NodeKind::Unsafe).unwrap();
        assert_eq!(json, "\"unsafe\"");
    }

    #[test]
    fn node_kind_all_variants_round_trip_via_serde() {
        let variants = [
            NodeKind::Pure,
            NodeKind::ManagedEffect,
            NodeKind::Wait,
            NodeKind::Signal,
            NodeKind::Unsafe,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let recovered: NodeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, variant, "round-trip failed for {:?}", variant);
        }
    }

    #[test]
    fn node_kind_rejects_unknown_variant_with_data_error() {
        let result: Result<NodeKind, serde_json::Error> = serde_json::from_str("\"nonexistent\"");
        let err = result.expect_err("should reject unknown variant 'nonexistent'");
        assert!(
            err.is_data(),
            "expected data error for unknown variant, got: {:?}",
            err
        );
    }

    #[test]
    fn node_kind_all_variants_returns_five_in_declaration_order() {
        let variants = NodeKind::all_variants();
        assert_eq!(variants.len(), 5);
        assert_eq!(variants[0], NodeKind::Pure);
        assert_eq!(variants[1], NodeKind::ManagedEffect);
        assert_eq!(variants[2], NodeKind::Wait);
        assert_eq!(variants[3], NodeKind::Signal);
        assert_eq!(variants[4], NodeKind::Unsafe);
    }
}
