//! Release-gate closure check: black-hat review gate for ADR-043.
//!
//! This module implements the closure check that prevents release-gate or
//! exact-once crash beads from being closed without a present and non-rejected
//! black-hat review.
//!
//! ## BDD Scenario (ADR-043)
//!
//! Given a release-gate or exact-once crash bead is ready to close
//! When closure check runs
//! Then black-hat review notes/status are present and not rejected
//!
//! ## Architecture
//!
//! ```text
//! BeadData (label, notes, status)
//!     └── GateConfig (labels_to_check, status_filter)
//!             └── ClosureCheck { config, beads }
//!                     └── check() -> GateResult
//! ```
//!
//! ## Proof Command
//!
//! The shell-level proof that release-gate beads carry black-hat notes:
//!
//! ```bash
//! bd list --label release-gate -n 0 --json | jq -e 'all(.[]; (.notes // "") | contains("black-hat"))'
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A bead record as used by the closure check.
///
/// Mirrors the structure of bead JSON output from `bd list --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeadData {
    pub id: String,
    pub labels: Vec<String>,
    pub notes: Option<String>,
    pub status: String,
}

impl BeadData {
    pub fn new(id: impl Into<String>, labels: Vec<String>, notes: Option<String>, status: String) -> Self {
        Self {
            id: id.into(),
            labels,
            notes,
            status,
        }
    }
}

/// Configuration for a closure check.
///
/// Defines which labels trigger the gate and which bead statuses
/// are considered "ready to close".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateConfig {
    /// Labels that require black-hat review before closure.
    pub release_labels: HashSet<String>,
    /// Bead statuses considered ready for closure.
    pub closure_statuses: HashSet<String>,
    /// The minimum required note substring for black-hat review.
    pub required_note_marker: String,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            release_labels: [String::from("release-gate")].into_iter().collect(),
            closure_statuses: ["in_progress", "closed"].into_iter().map(String::from).collect(),
            required_note_marker: String::from("black-hat"),
        }
    }
}

/// Result of a closure check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateResult {
    /// All release-gate beads have black-hat review present and not rejected.
    Pass {
        checked_count: usize,
        passing_ids: Vec<String>,
    },
    /// At least one release-gate bead is missing black-hat review or is rejected.
    Reject {
        checked_count: usize,
        failing: Vec<FailedCheck>,
    },
}

impl GateResult {
    /// Returns true if the gate check passed.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, GateResult::Pass { .. })
    }

    /// Returns true if the gate check rejected closure.
    #[must_use]
    pub fn is_reject(&self) -> bool {
        matches!(self, GateResult::Reject { .. })
    }
}

/// A single bead that failed the black-hat review gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedCheck {
    pub bead_id: String,
    pub reason: FailedReason,
}

/// Why a bead failed the black-hat review gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailedReason {
    /// The bead lacks any notes field.
    MissingNotes,
    /// The bead notes do not contain the required black-hat marker.
    MissingBlackHatReview,
    /// The bead notes indicate the review was rejected.
    ReviewRejected,
}

impl std::fmt::Display for FailedReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailedReason::MissingNotes => write!(f, "no notes present"),
            FailedReason::MissingBlackHatReview => write!(f, "black-hat review not present"),
            FailedReason::ReviewRejected => write!(f, "black-hat review was rejected"),
        }
    }
}

/// The closure check that gates release of release-gate beads.
///
/// This is the production path for ADR-043: before a release-gate or
/// exact-once crash bead can be closed, it must pass this check.
pub struct ClosureCheck {
    config: GateConfig,
    beads: Vec<BeadData>,
}

impl ClosureCheck {
    /// Create a new closure check with the default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: GateConfig::default(),
            beads: Vec::new(),
        }
    }

    /// Create a new closure check with explicit bead data.
    #[must_use]
    pub fn with_beads(beads: Vec<BeadData>) -> Self {
        Self {
            config: GateConfig::default(),
            beads,
        }
    }

    /// Create a new closure check with custom configuration.
    #[must_use]
    pub fn with_config(config: GateConfig, beads: Vec<BeadData>) -> Self {
        Self { config, beads }
    }

    /// Run the closure check against all beads.
    ///
    /// This is the main entry point for the Given/When/Then scenario:
    /// - **Given**: release-gate beads are in the bead list
    /// - **When**: check() is called
    /// - **Then**: returns Pass if all have black-hat notes, Reject otherwise
    pub fn check(&self) -> GateResult {
        // Filter beads that are release-gate AND ready to close
        let releasable: Vec<&BeadData> = self
            .beads
            .iter()
            .filter(|b| {
                self.config.release_labels.iter().any(|label| b.labels.contains(label))
                    && self.config.closure_statuses.contains(&b.status)
            })
            .collect();

        if releasable.is_empty() {
            return GateResult::Pass {
                checked_count: 0,
                passing_ids: Vec::new(),
            };
        }

        let mut failing = Vec::new();
        let mut passing_ids = Vec::new();

        for bead in &releasable {
            match self.evaluate_bead(bead) {
                Ok(()) => passing_ids.push(bead.id.clone()),
                Err(reason) => {
                    failing.push(FailedCheck {
                        bead_id: bead.id.clone(),
                        reason,
                    });
                }
            }
        }

        if failing.is_empty() {
            GateResult::Pass {
                checked_count: releasable.len(),
                passing_ids,
            }
        } else {
            GateResult::Reject {
                checked_count: releasable.len(),
                failing,
            }
        }
    }

    /// Evaluate a single bead for black-hat review compliance.
    fn evaluate_bead(&self, bead: &BeadData) -> Result<(), FailedReason> {
        // Check for missing notes
        let notes = match &bead.notes {
            Some(n) => n,
            None => return Err(FailedReason::MissingNotes),
        };

        // Check for black-hat marker
        if !notes.contains(&self.config.required_note_marker) {
            return Err(FailedReason::MissingBlackHatReview);
        }

        // Check for rejected marker (adversarial: review present but rejected)
        let notes_lower = notes.to_lowercase();
        if notes_lower.contains("rejected") {
            return Err(FailedReason::ReviewRejected);
        }

        Ok(())
    }

    /// Returns the number of beads loaded into the check.
    #[must_use]
    pub fn bead_count(&self) -> usize {
        self.beads.len()
    }
}

impl Default for ClosureCheck {
    fn default() -> Self {
        Self::new()
    }
}

// ─── BDD-style tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Given: release-gate bead with black-hat notes → When check → Then Pass ──

    #[test]
    fn given_release_gate_bead_with_black_hat_notes_when_check_then_passes() {
        let beads = vec![BeadData::new(
            "ve-crash-001",
            vec!["release-gate".to_string(), "adr-043".to_string()],
            Some("black-hat review completed by pipboy".to_string()),
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_pass());
        match result {
            GateResult::Pass {
                checked_count,
                passing_ids,
            } => {
                assert_eq!(checked_count, 1);
                assert_eq!(passing_ids, vec!["ve-crash-001".to_string()]);
            }
            _ => panic!("expected Pass"),
        }
    }

    #[test]
    fn given_exact_once_crash_bead_with_black_hat_when_check_then_passes() {
        let beads = vec![BeadData::new(
            "ve-crash-002",
            vec!["release-gate".to_string(), "exact-once".to_string()],
            Some("black-hat review: APPROVED WITH CONDITIONS".to_string()),
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_pass());
    }

    // ── Given: release-gate bead WITHOUT black-hat notes → When check → Then Reject ──

    #[test]
    fn given_release_gate_bead_without_black_hat_notes_when_check_then_rejects() {
        let beads = vec![BeadData::new(
            "ve-crash-003",
            vec!["release-gate".to_string()],
            Some("standard review completed".to_string()),
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_reject());
        match result {
            GateResult::Reject { checked_count, failing } => {
                assert_eq!(checked_count, 1);
                assert_eq!(failing.len(), 1);
                assert!(matches!(
                    failing[0].reason,
                    FailedReason::MissingBlackHatReview
                ));
            }
            _ => panic!("expected Reject"),
        }
    }

    #[test]
    fn given_release_gate_bead_with_no_notes_when_check_then_rejects() {
        let beads = vec![BeadData::new(
            "ve-crash-004",
            vec!["release-gate".to_string()],
            None,
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_reject());
        match result {
            GateResult::Reject { failing, .. } => {
                assert_eq!(failing.len(), 1);
                assert!(matches!(failing[0].reason, FailedReason::MissingNotes));
            }
            _ => panic!("expected Reject"),
        }
    }

    // ── Given: release-gate bead with rejected review → When check → Then Reject ──

    #[test]
    fn given_release_gate_bead_with_rejected_review_when_check_then_rejects() {
        let beads = vec![BeadData::new(
            "ve-crash-005",
            vec!["release-gate".to_string()],
            Some("black-hat review: REJECTED — critical finding F-1 unresolved".to_string()),
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_reject());
        match result {
            GateResult::Reject { failing, .. } => {
                assert_eq!(failing.len(), 1);
                assert!(matches!(failing[0].reason, FailedReason::ReviewRejected));
            }
            _ => panic!("expected Reject"),
        }
    }

    // ── Given: non-release-gate bead → When check → Then Pass (ignored) ──

    #[test]
    fn given_non_release_gate_bead_when_check_then_passes_no_checks() {
        let beads = vec![BeadData::new(
            "ve-123",
            vec!["adr".to_string(), "bdd".to_string()],
            Some("no black-hat review needed".to_string()),
            "in_progress".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_pass());
        match result {
            GateResult::Pass { checked_count, .. } => {
                assert_eq!(checked_count, 0);
            }
            _ => panic!("expected Pass with zero checked"),
        }
    }

    // ── Given: empty bead list → When check → Then Pass ──

    #[test]
    fn given_empty_bead_list_when_check_then_passes() {
        let check = ClosureCheck::with_beads(Vec::new());
        let result = check.check();

        assert!(result.is_pass());
        match result {
            GateResult::Pass {
                checked_count,
                passing_ids,
            } => {
                assert_eq!(checked_count, 0);
                assert!(passing_ids.is_empty());
            }
            _ => panic!("expected Pass"),
        }
    }

    // ── Given: multiple release-gate beads (mixed pass/fail) → When check → Then Reject ──

    #[test]
    fn given_multiple_release_gate_beads_mixed_when_check_then_rejects() {
        let beads = vec![
            BeadData::new(
                "ve-crash-pass",
                vec!["release-gate".to_string()],
                Some("black-hat review: approved".to_string()),
                "in_progress".to_string(),
            ),
            BeadData::new(
                "ve-crash-fail",
                vec!["release-gate".to_string()],
                None,
                "in_progress".to_string(),
            ),
        ];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_reject());
        match result {
            GateResult::Reject {
                checked_count,
                failing,
            } => {
                assert_eq!(checked_count, 2);
                assert_eq!(failing.len(), 1);
                assert_eq!(failing[0].bead_id, "ve-crash-fail");
            }
            _ => panic!("expected Reject"),
        }
    }

    // ── Given: multiple release-gate beads (all pass) → When check → Then Pass ──

    #[test]
    fn given_multiple_release_gate_beads_all_pass_when_check_then_passes() {
        let beads = vec![
            BeadData::new(
                "ve-crash-a",
                vec!["release-gate".to_string()],
                Some("black-hat review: approved".to_string()),
                "in_progress".to_string(),
            ),
            BeadData::new(
                "ve-crash-b",
                vec!["release-gate".to_string()],
                Some("black-hat review completed by radrat".to_string()),
                "in_progress".to_string(),
            ),
            BeadData::new(
                "ve-crash-c",
                vec!["release-gate".to_string(), "exact-once".to_string()],
                Some("black-hat review: APPROVED WITH CONDITIONS".to_string()),
                "closed".to_string(),
            ),
        ];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        assert!(result.is_pass());
        match result {
            GateResult::Pass {
                checked_count,
                passing_ids,
            } => {
                assert_eq!(checked_count, 3);
                assert_eq!(passing_ids.len(), 3);
            }
            _ => panic!("expected Pass"),
        }
    }

    // ── Given: case-insensitive black-hat marker → When check → Then Pass ──

    #[test]
    fn given_black_hat_case_variations_when_check_then_passes() {
        let test_cases = vec![
            "black-hat review passed",
            "Black-Hat review passed",
            "BLACK-HAT review passed",
            "adversarial black-hat review approved",
        ];

        for notes_text in test_cases {
            let beads = vec![BeadData::new(
                "ve-case-test",
                vec!["release-gate".to_string()],
                Some(notes_text.to_string()),
                "in_progress".to_string(),
            )];
            let check = ClosureCheck::with_beads(beads);
            let result = check.check();
            assert!(
                result.is_pass(),
                "expected Pass for notes: '{}'",
                notes_text
            );
        }
    }

    // ── Given: case-insensitive rejected marker → When check → Then Reject ──

    #[test]
    fn given_rejected_case_variations_when_check_then_rejects() {
        let test_cases = vec![
            "black-hat review rejected",
            "BLACK-HAT review REJECTED",
            "black-hat: REJECTED pending fixes",
        ];

        for notes_text in test_cases {
            let beads = vec![BeadData::new(
                "ve-rejected-test",
                vec!["release-gate".to_string()],
                Some(notes_text.to_string()),
                "in_progress".to_string(),
            )];
            let check = ClosureCheck::with_beads(beads);
            let result = check.check();
            assert!(
                result.is_reject(),
                "expected Reject for notes: '{}'",
                notes_text
            );
        }
    }

    // ── Given: status filter excludes non-closure statuses → When check → Then Pass (no check) ──

    #[test]
    fn given_release_gate_bead_not_ready_to_close_when_check_then_passes() {
        let beads = vec![BeadData::new(
            "ve-deferred",
            vec!["release-gate".to_string()],
            Some("no black-hat yet".to_string()),
            "deferred".to_string(),
        )];

        let check = ClosureCheck::with_beads(beads);
        let result = check.check();

        // deferred is not in closure_statuses, so it's filtered out
        assert!(result.is_pass());
        match result {
            GateResult::Pass { checked_count, .. } => {
                assert_eq!(checked_count, 0);
            }
            _ => panic!("expected Pass with zero checked"),
        }
    }

    // ── Given: custom config with multiple release labels → When check → Then Gates ──

    #[test]
    fn given_custom_config_with_exact_once_label_when_check_then_gates() {
        let mut config = GateConfig::default();
        config
            .release_labels
            .insert("exact-once-crash".to_string());

        let beads = vec![
            BeadData::new(
                "ve-eo-001",
                vec!["exact-once-crash".to_string()],
                Some("black-hat review passed".to_string()),
                "in_progress".to_string(),
            ),
            BeadData::new(
                "ve-eo-002",
                vec!["exact-once-crash".to_string()],
                Some("no review".to_string()),
                "in_progress".to_string(),
            ),
        ];

        let check = ClosureCheck::with_config(config, beads);
        let result = check.check();

        assert!(result.is_reject());
        match result {
            GateResult::Reject { checked_count, failing } => {
                assert_eq!(checked_count, 2);
                assert_eq!(failing.len(), 1);
            }
            _ => panic!("expected Reject"),
        }
    }

    // ── Idempotency: multiple check() calls produce same result ──

    #[test]
    fn given_closure_check_when_called_twice_then_same_result() {
        let beads = vec![
            BeadData::new(
                "ve-idem-1",
                vec!["release-gate".to_string()],
                Some("black-hat review passed".to_string()),
                "in_progress".to_string(),
            ),
            BeadData::new(
                "ve-idem-2",
                vec!["release-gate".to_string()],
                None,
                "in_progress".to_string(),
            ),
        ];

        let check = ClosureCheck::with_beads(beads);
        let result1 = check.check();
        let result2 = check.check();

        assert_eq!(result1, result2);
    }

    // ── BeadData serialization round-trip ──

    #[test]
    fn given_bead_data_when_serialized_and_deserialized_then_equivalent() {
        let bead = BeadData::new(
            "ve-serde-001",
            vec!["release-gate".to_string(), "adr-043".to_string()],
            Some("black-hat review approved".to_string()),
            "in_progress".to_string(),
        );

        let json = serde_json::to_string(&bead).expect("serde should succeed");
        let restored: BeadData =
            serde_json::from_str(&json).expect("serde should deserialize");

        assert_eq!(bead, restored);
    }
}
