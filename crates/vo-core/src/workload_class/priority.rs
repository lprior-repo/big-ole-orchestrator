//! Priority layer — load-shedding transparency types per ADR-033 §3.
//!
//! Contains `RejectionDetail` and `RejectionReason` for describing why
//! a resume or execution request was rejected during contention.

use super::classification::WorkloadClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionDetail {
    pub class: WorkloadClass,
    pub reason: RejectionReason,
}

impl RejectionDetail {
    #[must_use]
    pub fn budget_exhausted(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::BudgetExhausted,
        }
    }

    #[must_use]
    pub fn workflow_cap_exceeded(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::WorkflowCapExceeded,
        }
    }

    #[must_use]
    pub fn global_limit(class: WorkloadClass) -> Self {
        Self {
            class,
            reason: RejectionReason::GlobalConcurrencyLimit,
        }
    }
}

impl std::fmt::Display for RejectionDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rejected {:?}: {}",
            self.class,
            match &self.reason {
                RejectionReason::BudgetExhausted => "class budget exhausted",
                RejectionReason::WorkflowCapExceeded => "per-workflow cap exceeded",
                RejectionReason::GlobalConcurrencyLimit => "global concurrency limit reached",
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    BudgetExhausted,
    WorkflowCapExceeded,
    GlobalConcurrencyLimit,
}
