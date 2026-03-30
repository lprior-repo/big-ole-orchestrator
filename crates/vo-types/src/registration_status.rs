//! Registration eligibility state for a workflow.

use serde::{Deserialize, Serialize};

/// Registration eligibility state for a workflow.
/// Persisted in Fjall `workflows` partition.
///
/// State transitions:
///   Active -> Quarantined  (when failure threshold breached)
///   Active -> Deactivated  (operator action)
///   Quarantined -> Active  (unquarantine command)
///   Deactivated -> Active  (operator action)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegistrationStatus {
    /// Workflow accepts new binary registrations (subject to rate limit).
    Active,
    /// Workflow manually deactivated by operator. Rejects registrations.
    Deactivated,
    /// Workflow auto-quarantined by circuit breaker. Rejects registrations
    /// until manually unquarantined.
    Quarantined,
}
