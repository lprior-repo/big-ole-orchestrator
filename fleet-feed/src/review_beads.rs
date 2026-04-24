use crate::data::Rig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSkill {
    BlackHat,
    RedQueen,
    ScottDdd,
    ArchitecturalDrift,
    QaEnforcer,
    RustContract,
    TestReviewer,
    TruthSerum,
    FunctionalRust,
}

impl ReviewSkill {
    pub const fn all() -> &'static [Self] {
        &[
            Self::BlackHat,
            Self::RedQueen,
            Self::ScottDdd,
            Self::ArchitecturalDrift,
            Self::QaEnforcer,
            Self::RustContract,
            Self::TestReviewer,
            Self::TruthSerum,
            Self::FunctionalRust,
        ]
    }

    pub const fn prefix(self) -> &'static str {
        match self {
            Self::BlackHat => "BLACKHAT",
            Self::RedQueen => "REDQUEEN",
            Self::ScottDdd => "SCOTT-DDD",
            Self::ArchitecturalDrift => "ARCH-DRIFT",
            Self::QaEnforcer => "QA-ENFORCER",
            Self::RustContract => "RUST-CONTRACT",
            Self::TestReviewer => "TEST-REVIEW",
            Self::TruthSerum => "TRUTH-SERUM",
            Self::FunctionalRust => "FUNCTIONAL-RUST",
        }
    }

    pub const fn skill_name(self) -> &'static str {
        match self {
            Self::BlackHat => "black-hat-reviewer",
            Self::RedQueen => "red-queen",
            Self::ScottDdd => "scott-ddd-refactor",
            Self::ArchitecturalDrift => "architectural-drift",
            Self::QaEnforcer => "qa-enforcer",
            Self::RustContract => "rust-contract",
            Self::TestReviewer => "test-reviewer",
            Self::TruthSerum => "truth-serum",
            Self::FunctionalRust => "functional-rust",
        }
    }

    pub const fn focus(self) -> &'static str {
        match self {
            Self::BlackHat => {
                "attack contract parity, security assumptions, panic vectors, and lazy error handling"
            }
            Self::RedQueen => {
                "create deterministic adversarial checks and survivor beads for regressions"
            }
            Self::ScottDdd => {
                "make illegal states unrepresentable with Scott Wlaschin DDD and typed workflows"
            }
            Self::ArchitecturalDrift => {
                "enforce file-size, cohesion, module boundaries, and ADR conformance"
            }
            Self::QaEnforcer => {
                "execute real CLI/API/workflow probes and file evidence-backed defects"
            }
            Self::RustContract => {
                "write or repair contract-spec and Martin Fowler Given-When-Then plans before implementation"
            }
            Self::TestReviewer => {
                "interrogate weak tests, mutation survivability, exact error assertions, and coverage holes"
            }
            Self::TruthSerum => {
                "prove claims with terminal evidence and expose hallucinated paths, deleted tests, and contract drift"
            }
            Self::FunctionalRust => {
                "enforce Data-Calc-Actions layering, zero unwrap/panic, explicit errors, and pure core logic"
            }
        }
    }
}

const REQUIRED_PLANNING_STEP: &str = "Invoke `planner` first to decompose this review into one or more atomic micro-beads when the scope is larger than a single source file finding.";
const REQUIRED_DRIFT_GATE: &str = "Invoke `architectural-drift` before close to verify file size, module cohesion, DDD boundaries, and ADR conformance.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewBead {
    pub title: String,
    pub description: String,
    pub acceptance: String,
    pub design: String,
}

pub fn build_review_bead(rig: &Rig, module: &str, skill: ReviewSkill) -> ReviewBead {
    ReviewBead {
        title: review_title(module, skill),
        description: review_description(rig, module, skill),
        acceptance: review_acceptance(module, skill),
        design: review_design(rig, module, skill),
    }
}

fn review_title(module: &str, skill: ReviewSkill) -> String {
    format!("{} micro-review: {}", skill.prefix(), module)
}

fn review_description(rig: &Rig, module: &str, skill: ReviewSkill) -> String {
    format!(
        "{planning_step} Then invoke `{skill_name}` and run a narrow health review of `{module}` in `{rig_name}`. Focus: {focus}. {drift_gate} Keep this micro-bead scoped to source health; create discovered-from follow-up beads for fixes that exceed this file or require broader ADR decisions.",
        planning_step = REQUIRED_PLANNING_STEP,
        skill_name = skill.skill_name(),
        rig_name = rig.name,
        focus = skill.focus(),
        drift_gate = REQUIRED_DRIFT_GATE
    )
}

fn review_acceptance(module: &str, skill: ReviewSkill) -> String {
    format!(
        "Use `planner`, `{skill_name}`, and `architectural-drift` in that order. Inspect `{module}` only unless imports force one-hop context. Produce concrete findings with file:line evidence, command evidence when the skill requires execution, ADR impact notes, and explicit close criteria. If the planner finds multiple independent findings, create child beads and link them discovered-from this bead. If code changes are needed, keep them molecular and push to main after verification. If no issue exists, close with the evidence that proved the module healthy and architecture-drift clean.",
        skill_name = skill.skill_name()
    )
}

fn review_design(rig: &Rig, module: &str, skill: ReviewSkill) -> String {
    format!(
        "Fleet-feed generated health matrix bead. Source of truth is `{src_dir}`; do not use stale GT rig clones for bd work. Required sequence: planner decomposition, `{skill_name}` review, architectural-drift gate, then close or child-bead creation. Check existing ADRs or decision docs when present; if ADR coverage is missing or stale, create a separate decision bead instead of inventing policy inside this task. Scope target: `{module}`.",
        src_dir = rig.src_dir,
        skill_name = skill.skill_name()
    )
}
