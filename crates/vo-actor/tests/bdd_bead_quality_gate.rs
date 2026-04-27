#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::disallowed_methods)]
//! BDD Semantic Bead Quality Gate (tw-4y6h.20.4)
//!
//! Verifies that all semantic-gap and micro-bead descriptions contain:
//! - ADR refs
//! - Given/When/Then BDD scenario
//! - Required proof command
//!
//! This is a release quality gate: beads without proper BDD text fail here.

use std::process::Command;

fn run_bd_list(label: &str) -> Vec<serde_json::Value> {
    let output = Command::new("bd")
        .args(["list", "--label", label, "-n", "0", "--json"])
        .output()
        .expect("bd list command failed");

    if !output.status.success() {
        eprintln!(
            "bd list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return vec![];
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("failed to parse bd list output as JSON")
}

#[derive(Debug)]
struct BeadQualityIssue {
    id: String,
    missing: Vec<&'static str>,
}

fn check_bdd_fields(bead: &serde_json::Value) -> Option<BeadQualityIssue> {
    let desc = bead
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let id = bead.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");

    let mut missing = vec![];

    if !desc.contains("ADR refs") && !desc.contains("ADR-") {
        missing.push("ADR refs");
    }
    if !desc.contains("Given") {
        missing.push("Given");
    }
    if !desc.contains("When") {
        missing.push("When");
    }
    if !desc.contains("Then") {
        missing.push("Then");
    }
    if !desc.contains("Required proof command") {
        missing.push("Required proof command");
    }

    if missing.is_empty() {
        None
    } else {
        Some(BeadQualityIssue {
            id: id.to_string(),
            missing,
        })
    }
}

// =============================================================================
// BDD Scenario: Given/When/Then for Micro-bead Descriptions
// =============================================================================

mod bdd_micro_bead_quality {
    use super::*;

    fn all_micro_beads_have_bdd_text() -> Vec<BeadQualityIssue> {
        let beads = run_bd_list("micro-bead");
        beads.iter().filter_map(check_bdd_fields).collect()
    }

    #[test]
    fn given_micro_bead_when_quality_check_runs_then_contains_adr_refs() {
        let issues = all_micro_beads_have_bdd_text();
        let missing_adr: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"ADR refs"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_adr.is_empty(),
            "micro-beads missing ADR refs: {missing_adr:?}"
        );
    }

    #[test]
    fn given_micro_bead_when_quality_check_runs_then_contains_given() {
        let issues = all_micro_beads_have_bdd_text();
        let missing_given: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Given"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_given.is_empty(),
            "micro-beads missing Given: {missing_given:?}"
        );
    }

    #[test]
    fn given_micro_bead_when_quality_check_runs_then_contains_when() {
        let issues = all_micro_beads_have_bdd_text();
        let missing_when: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"When"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_when.is_empty(),
            "micro-beads missing When: {missing_when:?}"
        );
    }

    #[test]
    fn given_micro_bead_when_quality_check_runs_then_contains_then() {
        let issues = all_micro_beads_have_bdd_text();
        let missing_then: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Then"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_then.is_empty(),
            "micro-beads missing Then: {missing_then:?}"
        );
    }

    #[test]
    fn given_micro_bead_when_quality_check_runs_then_contains_proof_command() {
        let issues = all_micro_beads_have_bdd_text();
        let missing_proof: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Required proof command"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_proof.is_empty(),
            "micro-beads missing Required proof command: {missing_proof:?}"
        );
    }
}

// =============================================================================
// BDD Scenario: Given/When/Then for Semantic-gap Bead Descriptions
// =============================================================================

mod bdd_semantic_gap_bead_quality {
    use super::*;

    fn all_semantic_gap_beads_have_bdd_text() -> Vec<BeadQualityIssue> {
        let beads = run_bd_list("semantic-gap");
        beads.iter().filter_map(check_bdd_fields).collect()
    }

    #[test]
    fn given_semantic_gap_bead_when_quality_check_runs_then_contains_adr_refs() {
        let issues = all_semantic_gap_beads_have_bdd_text();
        let missing_adr: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"ADR refs"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_adr.is_empty(),
            "semantic-gap beads missing ADR refs: {missing_adr:?}"
        );
    }

    #[test]
    fn given_semantic_gap_bead_when_quality_check_runs_then_contains_given() {
        let issues = all_semantic_gap_beads_have_bdd_text();
        let missing_given: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Given"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_given.is_empty(),
            "semantic-gap beads missing Given: {missing_given:?}"
        );
    }

    #[test]
    fn given_semantic_gap_bead_when_quality_check_runs_then_contains_when() {
        let issues = all_semantic_gap_beads_have_bdd_text();
        let missing_when: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"When"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_when.is_empty(),
            "semantic-gap beads missing When: {missing_when:?}"
        );
    }

    #[test]
    fn given_semantic_gap_bead_when_quality_check_runs_then_contains_then() {
        let issues = all_semantic_gap_beads_have_bdd_text();
        let missing_then: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Then"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_then.is_empty(),
            "semantic-gap beads missing Then: {missing_then:?}"
        );
    }

    #[test]
    fn given_semantic_gap_bead_when_quality_check_runs_then_contains_proof_command() {
        let issues = all_semantic_gap_beads_have_bdd_text();
        let missing_proof: Vec<_> = issues
            .iter()
            .filter(|i| i.missing.contains(&"Required proof command"))
            .map(|i| i.id.as_str())
            .collect();

        assert!(
            missing_proof.is_empty(),
            "semantic-gap beads missing Required proof command: {missing_proof:?}"
        );
    }
}

// =============================================================================
// Combined BDD Scenario (ADR-043 Gate)
// =============================================================================

mod bdd_combined_quality_gate {
    use super::*;

    #[test]
    fn given_bead_with_semantic_gap_or_micro_bead_label_when_quality_check_runs_then_all_bdd_fields_present(
    ) {
        let micro_beads = run_bd_list("micro-bead");
        let semantic_beads = run_bd_list("semantic-gap");

        let mut all_issues = vec![];

        for bead in micro_beads.iter().chain(semantic_beads.iter()) {
            if let Some(issue) = check_bdd_fields(bead) {
                all_issues.push(issue);
            }
        }

        if !all_issues.is_empty() {
            let report: String = all_issues
                .iter()
                .map(|i| format!("  {} missing: {:?}", i.id, i.missing))
                .collect::<Vec<_>>()
                .join("\n");

            panic!("BDD quality gate FAILED - beads missing required fields:\n{report}");
        }
    }

    #[test]
    fn given_micro_beads_total_count_is_reasonable() {
        let beads = run_bd_list("micro-bead");
        assert!(
            !beads.is_empty(),
            "Expected at least some micro-beads to exist"
        );
    }

    #[test]
    fn given_semantic_gap_beads_total_count_is_reasonable() {
        let beads = run_bd_list("semantic-gap");
        assert!(
            !beads.is_empty(),
            "Expected at least some semantic-gap beads to exist"
        );
    }
}
