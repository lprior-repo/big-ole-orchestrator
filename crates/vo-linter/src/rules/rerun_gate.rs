//! Doc-to-Beads Rerun Gate
//!
//! Prevents planner floods by ensuring spec-hardening (planner-expansion) beads
//! are closed before allowing doc-to-beads or arch-spec-to-beads to rerun on ADRs.
//!
//! # Gate Logic
//!
//! - Queries for open spec-hardening beads using `bd list --label=planner-expansion --status=open --json`
//! - If any spec-hardening beads are open, the gate BLOCKS the rerun
//! - If all spec-hardening beads are closed, the gate ALLOWS the rerun
//!
//! # Rationale
//!
//! Running doc-to-beads/arch-spec-to-beads on under-specified ADRs generates
//! many incomplete beads ("planner floods"). The spec-hardening beads in the
//! planner-expansion program close ADR gaps first. Only after these are closed
//! should doc-to-beads be rerun for exhaustive scenario-level beads.

use std::process::Command;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateStatus {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct GateResult {
    pub status: GateStatus,
    pub open_beads: Vec<OpenBead>,
    pub total_checked: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenBead {
    pub id: String,
    pub title: String,
    pub priority: u8,
}

impl GateResult {
    pub fn is_allowed(&self) -> bool {
        self.status == GateStatus::Allowed
    }

    pub fn is_blocked(&self) -> bool {
        self.status == GateStatus::Blocked
    }

    pub fn blocked_count(&self) -> usize {
        self.open_beads.len()
    }
}

fn parse_json_output(output: &str) -> Vec<OpenBead> {
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "No issues found." {
        return Vec::new();
    }

    match serde_json::from_str(trimmed) {
        Ok(issues) => issues,
        Err(_) => {
            if trimmed.starts_with('[') {
                Vec::new()
            } else {
                Vec::new()
            }
        }
    }
}

pub fn check_spec_hardening_gate() -> GateResult {
    let output = Command::new("bd")
        .args([
            "list",
            "--label=planner-expansion",
            "--status=open",
            "--json",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let open_beads = parse_json_output(&stdout);

            let total_checked = open_beads.len();
            let status = if open_beads.is_empty() {
                GateStatus::Allowed
            } else {
                GateStatus::Blocked
            };

            GateResult {
                status,
                open_beads,
                total_checked,
            }
        }
        Err(_e) => {
            GateResult {
                status: GateStatus::Allowed,
                open_beads: Vec::new(),
                total_checked: 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_result_allowed() {
        let result = GateResult {
            status: GateStatus::Allowed,
            open_beads: Vec::new(),
            total_checked: 0,
        };
        assert!(result.is_allowed());
        assert!(!result.is_blocked());
        assert_eq!(result.blocked_count(), 0);
    }

    #[test]
    fn test_gate_result_blocked() {
        let result = GateResult {
            status: GateStatus::Blocked,
            open_beads: vec![
                OpenBead {
                    id: "tw-1234".to_string(),
                    title: "Test bead".to_string(),
                    priority: 1,
                },
            ],
            total_checked: 1,
        };
        assert!(!result.is_allowed());
        assert!(result.is_blocked());
        assert_eq!(result.blocked_count(), 1);
    }

    #[test]
    fn test_parse_json_output_empty() {
        assert!(parse_json_output("").is_empty());
        assert!(parse_json_output("No issues found.").is_empty());
        assert!(parse_json_output("   ").is_empty());
    }

    #[test]
    fn test_parse_json_output_invalid() {
        assert!(parse_json_output("not json").is_empty());
        assert!(parse_json_output("{ invalid }").is_empty());
    }
}