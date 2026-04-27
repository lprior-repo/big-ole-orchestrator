#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

//! Quarantine-aware UI badge (ADR-041, ve-uez).
//!
//! Renders a visual badge indicating when a workflow is quarantined due to
//! repeated failures. Uses Tailwind classes with amber styling for warnings
//! and red styling for deactivated/deleted states.

use dioxus::prelude::*;

/// Quarantine state for a workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineState {
    /// Workflow is not quarantined.
    None,
    /// Workflow is quarantined due to exceeding the failure threshold.
    Active {
        /// Human-readable reason (e.g. "failure_threshold_exceeded").
        reason: String,
        /// Current failure count in the sliding window.
        failure_count: usize,
    },
    /// Workflow has been deactivated by the circuit breaker.
    Deactivated,
    /// Workflow registration has been deleted.
    Deleted,
}

/// Workflow quarantine status badge.
///
/// Renders a colored badge indicating the quarantine state:
/// - `None`: no badge shown (returns empty string)
/// - `Active`: amber badge with "quarantined" label and failure count
/// - `Deactivated`: red badge with "deactivated" label
/// - `Deleted`: gray badge with "deleted" label
#[component]
pub fn QuarantineBadge(state: QuarantineState, class: Option<String>) -> Element {
    let extra_class = class.unwrap_or_default();

    match state {
        QuarantineState::None => rsx! {},
        QuarantineState::Active { reason, failure_count } => {
            rsx! {
                span {
                    class: "inline-flex items-center gap-1 rounded-md border border-amber-400 bg-amber-100 px-2 py-0.5 text-[10px] font-medium text-amber-700 {extra_class}",
                    span { "quarantined ({failure_count} failures: {reason})" }
                }
            }
        }
        QuarantineState::Deactivated => {
            rsx! {
                span {
                    class: "inline-flex items-center gap-1 rounded-md border border-red-400 bg-red-100 px-2 py-0.5 text-[10px] font-medium text-red-700 {extra_class}",
                    span { "deactivated" }
                }
            }
        }
        QuarantineState::Deleted => {
            rsx! {
                span {
                    class: "inline-flex items-center gap-1 rounded-md border border-gray-400 bg-gray-100 px-2 py-0.5 text-[10px] font-medium text-gray-700 {extra_class}",
                    span { "deleted" }
                }
            }
        }
    }
}

/// Converts API quarantine fields into a `QuarantineState`.
///
/// - `is_quarantined: true` with a reason -> `Active { reason, failure_count }`
/// - `registration_status` of "deactivated" -> `Deactivated`
/// - `registration_status` of "deleted" -> `Deleted`
/// - Otherwise -> `None`
pub fn quarantine_state(
    is_quarantined: bool,
    quarantine_reason: Option<&str>,
    failure_count: usize,
    registration_status: Option<&str>,
) -> QuarantineState {
    if is_quarantined {
        let reason = quarantine_reason
            .map(|r| r.to_owned())
            .unwrap_or_else(|| "failure_threshold_exceeded".to_owned());
        QuarantineState::Active {
            reason,
            failure_count,
        }
    } else if matches!(registration_status, Some("deactivated")) {
        QuarantineState::Deactivated
    } else if matches!(registration_status, Some("deleted")) {
        QuarantineState::Deleted
    } else {
        QuarantineState::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_state_none_when_not_quarantined() {
        let state = quarantine_state(false, None, 0, None);
        assert_eq!(state, QuarantineState::None);
    }

    #[test]
    fn quarantine_state_active_with_reason() {
        let state = quarantine_state(true, Some("failure_threshold_exceeded"), 5, None);
        if let QuarantineState::Active { reason, failure_count } = state {
            assert_eq!(reason, "failure_threshold_exceeded");
            assert_eq!(failure_count, 5);
        } else {
            panic!("expected Active state");
        }
    }

    #[test]
    fn quarantine_state_defaults_reason_when_none() {
        let state = quarantine_state(true, None, 3, None);
        if let QuarantineState::Active { reason, failure_count } = state {
            assert_eq!(reason, "failure_threshold_exceeded");
            assert_eq!(failure_count, 3);
        } else {
            panic!("expected Active state");
        }
    }

    #[test]
    fn quarantine_state_deactivated() {
        let state = quarantine_state(false, None, 0, Some("deactivated"));
        assert_eq!(state, QuarantineState::Deactivated);
    }

    #[test]
    fn quarantine_state_deleted() {
        let state = quarantine_state(false, None, 0, Some("deleted"));
        assert_eq!(state, QuarantineState::Deleted);
    }

    #[test]
    fn quarantine_state_quarantined_takes_precedence_over_deactivated() {
        let state = quarantine_state(true, Some("failure_threshold_exceeded"), 5, Some("deactivated"));
        if let QuarantineState::Active { reason, .. } = state {
            assert_eq!(reason, "failure_threshold_exceeded");
        } else {
            panic!("expected Active state, got: {state:?}");
        }
    }

    #[test]
    fn quarantine_state_none_for_active_registration() {
        let state = quarantine_state(false, None, 0, Some("active"));
        assert_eq!(state, QuarantineState::None);
    }

    #[test]
    fn quarantine_state_empty_string_for_none_reason() {
        let state = quarantine_state(true, Some(""), 1, None);
        if let QuarantineState::Active { reason, .. } = state {
            assert_eq!(reason, "");
        } else {
            panic!("expected Active state");
        }
    }
}
