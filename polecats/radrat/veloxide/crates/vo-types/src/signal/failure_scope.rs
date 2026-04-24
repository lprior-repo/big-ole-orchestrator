//! FailureScope — Failure classification scope for workflow errors per ADR-042 Section 5
//!
//! This module defines the failure scope classification that determines whether
//! a failure terminates only the current epoch or the entire lineage.

use serde::{Deserialize, Serialize};

/// Failure scope classification per ADR-042 Section 5.
///
/// Determines the consequence of a failure:
/// - `Epoch`: Failure terminates the current epoch but allows retry/continue-as-new
/// - `Lineage`: Failure permanently tombstones the lineage (no further epochs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureScope {
    /// Epoch-scoped failure: allows retry or continue-as-new within the same lineage.
    Epoch,
    /// Lineage-scoped failure: permanently prevents further epochs in this lineage.
    Lineage,
}

impl FailureScope {
    /// Returns `true` if this scope is epoch-scoped.
    #[must_use]
    pub const fn is_epoch(&self) -> bool {
        matches!(self, Self::Epoch)
    }

    /// Returns `true` if this scope is lineage-scoped.
    #[must_use]
    pub const fn is_lineage(&self) -> bool {
        matches!(self, Self::Lineage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_scope_epoch_is_epoch() {
        let scope = FailureScope::Epoch;
        assert!(scope.is_epoch());
        assert!(!scope.is_lineage());
    }

    #[test]
    fn failure_scope_lineage_is_lineage() {
        let scope = FailureScope::Lineage;
        assert!(!scope.is_epoch());
        assert!(scope.is_lineage());
    }

    #[test]
    fn failure_scope_debug() {
        let epoch = FailureScope::Epoch;
        let lineage = FailureScope::Lineage;
        assert_eq!(format!("{:?}", epoch), "Epoch");
        assert_eq!(format!("{:?}", lineage), "Lineage");
    }

    #[test]
    fn failure_scope_eq() {
        assert_eq!(FailureScope::Epoch, FailureScope::Epoch);
        assert_eq!(FailureScope::Lineage, FailureScope::Lineage);
        assert_ne!(FailureScope::Epoch, FailureScope::Lineage);
    }
}
