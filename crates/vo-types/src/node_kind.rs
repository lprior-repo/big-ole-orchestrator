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
}
