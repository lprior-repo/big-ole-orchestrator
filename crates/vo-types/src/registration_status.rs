//! Registration eligibility state for a workflow (ADR-021).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Registration eligibility state for a workflow.
/// Persisted in Fjall `workflows` partition.
///
/// State transitions (ADR-021 ghost workflow lifecycle):
///   Active -> Deactivated   (file watcher detects binary deletion)
///   Active -> Quarantined   (circuit breaker failure threshold breached)
///   Active -> Deactivated   (operator action)
///   Deactivated -> Deleted  (reaper GC: 0 running instances)
///   Quarantined -> Active   (unquarantine command)
///   Deactivated -> Active   (operator reactivation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegistrationStatus {
    /// Workflow accepts new triggers and binary registrations.
    Active,
    /// Workflow binary deleted. Rejects new triggers, in-flight instances
    /// continue against pinned version. Reaper will sweep to Deleted.
    Deactivated,
    /// Workflow auto-quarantined by circuit breaker. Rejects registrations
    /// until manually unquarantined.
    Quarantined,
    /// Workflow physically purged — binary deleted, registration removed.
    /// Terminal state (no transitions out).
    Deleted,
}

impl fmt::Display for RegistrationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = match self {
            Self::Active => "active",
            Self::Deactivated => "deactivated",
            Self::Quarantined => "quarantined",
            Self::Deleted => "deleted",
        };
        f.write_str(status)
    }
}
