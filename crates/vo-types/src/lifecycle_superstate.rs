//! Hierarchical lifecycle superstates (ADR-039).
//!
//! Top-level grouping of the flat [`LifecycleState`](super::state::LifecycleState)
//! into broader operational categories used by the scheduler, visibility layer,
//! and compensation planner.

use serde::{Deserialize, Serialize};

/// Hierarchical superstate grouping for the flat [`LifecycleState`](super::state::LifecycleState) (ADR-039).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleSuperstate {
    Active,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn active_serializes_to_snake_case() {
        let json = serde_json::to_string(&LifecycleSuperstate::Active).unwrap();
        assert_eq!(json, "\"active\"");
    }

    // suspended test deferred — variant not yet TDD'd
}
