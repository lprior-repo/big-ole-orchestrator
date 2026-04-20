//! Workload class taxonomy for execution fairness per ADR-033.
//!
//! Defines the four workload classes that govern resume scheduling and
//! execution permit allocation:
//! - `ExactCritical` — never starved, highest dispatch priority
//! - `Standard` — normal workflow execution
//! - `UnsafeBulk` — lower priority, capped under contention
//! - `Recovery` — reserved capacity for crash recovery
//!
//! Also provides `WorkloadBudget` for per-class permit tracking and
//! `RejectionDetail` for load-shedding transparency.

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors for workload class operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkloadClassError {
    /// Returned when an unknown workload class string is parsed.
    #[error("unknown workload class: {0}")]
    UnknownClass(String),

    /// Returned when a workload budget constraint is violated.
    #[error("budget exceeded for {class:?}: requested {requested}, available {available}")]
    BudgetExceeded {
        class: WorkloadClass,
        requested: u32,
        available: u32,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadClass Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Workload classification per ADR-033 for resume fairness.
///
/// Determines scheduling priority, permit reservation, and load-shedding
/// behavior. Classes are ordered by dispatch priority: lower rank = higher priority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    /// Highest priority. Never starved by lower classes.
    ExactCritical,
    /// Default priority for normal workflow execution.
    #[default]
    Standard,
    /// Lower priority. Capped under contention.
    UnsafeBulk,
    /// Reserved capacity for crash recovery.
    Recovery,
}

impl PartialOrd for WorkloadClass {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WorkloadClass {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl WorkloadClass {
    /// Dispatch priority rank (lower = higher priority).
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            WorkloadClass::ExactCritical => 0,
            WorkloadClass::Standard => 1,
            WorkloadClass::Recovery => 2,
            WorkloadClass::UnsafeBulk => 3,
        }
    }

    /// Returns `true` if this class is never starved by lower-priority work.
    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(self, WorkloadClass::ExactCritical | WorkloadClass::Recovery)
    }

    /// Returns `true` if this class is subject to contention caps.
    #[must_use]
    pub fn is_capped_under_contention(self) -> bool {
        matches!(self, WorkloadClass::UnsafeBulk)
    }

    /// Parses a string into a `WorkloadClass`.
    pub fn parse(s: &str) -> Result<WorkloadClass, WorkloadClassError> {
        match s {
            "exact_critical" => Ok(WorkloadClass::ExactCritical),
            "standard" => Ok(WorkloadClass::Standard),
            "unsafe_bulk" => Ok(WorkloadClass::UnsafeBulk),
            "recovery" => Ok(WorkloadClass::Recovery),
            _ => Err(WorkloadClassError::UnknownClass(s.to_string())),
        }
    }

    /// Returns the canonical snake_case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WorkloadClass::ExactCritical => "exact_critical",
            WorkloadClass::Standard => "standard",
            WorkloadClass::UnsafeBulk => "unsafe_bulk",
            WorkloadClass::Recovery => "recovery",
        }
    }

    /// Returns all workload class variants ordered by priority (highest first).
    #[must_use]
    pub fn all_by_priority() -> &'static [WorkloadClass] {
        &[
            WorkloadClass::ExactCritical,
            WorkloadClass::Standard,
            WorkloadClass::Recovery,
            WorkloadClass::UnsafeBulk,
        ]
    }
}

impl FromStr for WorkloadClass {
    type Err = WorkloadClassError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        WorkloadClass::parse(s)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadBudget
// ─────────────────────────────────────────────────────────────────────────────

use std::cell::RefCell;

/// Per-class execution permit budget per ADR-033.
///
/// Tracks reserved and used permits per workload class.
///
/// # Invariants
/// - `used[class] <= reserved[class]` always holds.
#[derive(Clone, Debug)]
pub struct WorkloadBudget {
    reserved: [u32; 4],
    used: RefCell<[u32; 4]>,
}

impl WorkloadBudget {
    fn class_index(class: WorkloadClass) -> usize {
        class.rank() as usize
    }

    /// Creates a budget with per-class reserved permit counts.
    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            reserved: [exact_critical, standard, recovery, unsafe_bulk],
            used: RefCell::new([0, 0, 0, 0]),
        }
    }

    /// Returns a budget with sensible defaults for a medium-scale deployment.
    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    /// Returns remaining permits for a given class.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        let idx = Self::class_index(class);
        let used = self.used.borrow();
        self.reserved[idx].saturating_sub(used[idx])
    }

    /// Checks if a permit can be acquired for the given class.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        self.remaining(class) > 0
    }

    /// Acquires a permit for the given class.
    pub fn acquire(&self, class: WorkloadClass) -> Result<(), WorkloadClassError> {
        let idx = Self::class_index(class);
        if self.remaining(class) == 0 {
            return Err(WorkloadClassError::BudgetExceeded {
                class,
                requested: 1,
                available: 0,
            });
        }
        self.used.borrow_mut()[idx] += 1;
        Ok(())
    }

    /// Releases a previously acquired permit.
    pub fn release(&self, class: WorkloadClass) {
        let idx = Self::class_index(class);
        let used = &mut self.used.borrow_mut()[idx];
        *used = used.saturating_sub(1);
    }

    /// Returns the total reserved permits across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.reserved.iter().sum()
    }

    /// Returns the total used permits across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.used.borrow().iter().sum()
    }

    /// Returns the reserved permit count for a given class.
    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.reserved[Self::class_index(class)]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RejectionDetail — Load-Shedding Transparency
// ─────────────────────────────────────────────────────────────────────────────

/// Describes why a resume or execution request was rejected (ADR-033 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectionDetail {
    /// The workload class of the rejected request.
    pub class: WorkloadClass,
    /// Human-readable reason for rejection.
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

/// The specific reason for a rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    BudgetExhausted,
    WorkflowCapExceeded,
    GlobalConcurrencyLimit,
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exact_critical() {
        assert_eq!(
            WorkloadClass::parse("exact_critical"),
            Ok(WorkloadClass::ExactCritical)
        );
    }

    #[test]
    fn parse_standard() {
        assert_eq!(
            WorkloadClass::parse("standard"),
            Ok(WorkloadClass::Standard)
        );
    }

    #[test]
    fn parse_unsafe_bulk() {
        assert_eq!(
            WorkloadClass::parse("unsafe_bulk"),
            Ok(WorkloadClass::UnsafeBulk)
        );
    }

    #[test]
    fn parse_recovery() {
        assert_eq!(
            WorkloadClass::parse("recovery"),
            Ok(WorkloadClass::Recovery)
        );
    }

    #[test]
    fn parse_unknown_returns_err() {
        assert!(WorkloadClass::parse("garbage").is_err());
    }

    #[test]
    fn parse_empty_returns_err() {
        assert!(WorkloadClass::parse("").is_err());
    }

    #[test]
    fn rank_exact_critical_is_0() {
        assert_eq!(WorkloadClass::ExactCritical.rank(), 0);
    }

    #[test]
    fn rank_standard_is_1() {
        assert_eq!(WorkloadClass::Standard.rank(), 1);
    }

    #[test]
    fn rank_recovery_is_2() {
        assert_eq!(WorkloadClass::Recovery.rank(), 2);
    }

    #[test]
    fn rank_unsafe_bulk_is_3() {
        assert_eq!(WorkloadClass::UnsafeBulk.rank(), 3);
    }

    #[test]
    fn classes_ordered_by_priority() {
        assert!(WorkloadClass::ExactCritical < WorkloadClass::Standard);
        assert!(WorkloadClass::Standard < WorkloadClass::Recovery);
        assert!(WorkloadClass::Recovery < WorkloadClass::UnsafeBulk);
    }

    #[test]
    fn never_starved_exact_critical() {
        assert!(WorkloadClass::ExactCritical.never_starved());
    }

    #[test]
    fn never_starved_recovery() {
        assert!(WorkloadClass::Recovery.never_starved());
    }

    #[test]
    fn not_never_starved_standard() {
        assert!(!WorkloadClass::Standard.never_starved());
    }

    #[test]
    fn not_never_starved_unsafe_bulk() {
        assert!(!WorkloadClass::UnsafeBulk.never_starved());
    }

    #[test]
    fn only_unsafe_bulk_is_capped() {
        assert!(WorkloadClass::UnsafeBulk.is_capped_under_contention());
        assert!(!WorkloadClass::ExactCritical.is_capped_under_contention());
        assert!(!WorkloadClass::Standard.is_capped_under_contention());
        assert!(!WorkloadClass::Recovery.is_capped_under_contention());
    }

    #[test]
    fn as_str_roundtrips() {
        for class in WorkloadClass::all_by_priority() {
            assert_eq!(WorkloadClass::parse(class.as_str()), Ok(*class));
        }
    }

    #[test]
    fn all_by_priority_contains_all() {
        assert_eq!(WorkloadClass::all_by_priority().len(), 4);
    }

    #[test]
    fn default_is_standard() {
        assert_eq!(WorkloadClass::default(), WorkloadClass::Standard);
    }

    #[test]
    fn from_str_delegates_to_parse() {
        assert_eq!(
            "exact_critical".parse::<WorkloadClass>(),
            Ok(WorkloadClass::ExactCritical)
        );
    }

    #[test]
    fn json_roundtrip_preserves_variant() {
        for class in WorkloadClass::all_by_priority() {
            let json = serde_json::to_string(&class).unwrap();
            let parsed: WorkloadClass = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, *class);
        }
    }

    // ── WorkloadBudget ─────────────────────────────────────────────────────

    #[test]
    fn budget_remaining_matches_initial() {
        let budget = WorkloadBudget::new(10, 20, 8, 5);
        assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
        assert_eq!(budget.remaining(WorkloadClass::Standard), 20);
        assert_eq!(budget.remaining(WorkloadClass::UnsafeBulk), 5);
        assert_eq!(budget.remaining(WorkloadClass::Recovery), 8);
    }

    #[test]
    fn budget_acquire_deducts_permit() {
        let budget = WorkloadBudget::new(10, 20, 8, 5);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 9);
    }

    #[test]
    fn budget_release_restores_permit() {
        let budget = WorkloadBudget::new(10, 20, 8, 5);
        budget.acquire(WorkloadClass::Standard).unwrap();
        budget.release(WorkloadClass::Standard);
        assert_eq!(budget.remaining(WorkloadClass::Standard), 20);
    }

    #[test]
    fn budget_acquire_fails_when_exhausted() {
        let budget = WorkloadBudget::new(1, 0, 0, 0);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert!(budget.acquire(WorkloadClass::ExactCritical).is_err());
    }

    #[test]
    fn budget_can_acquire_reflects_state() {
        let budget = WorkloadBudget::new(1, 0, 0, 0);
        assert!(budget.can_acquire(WorkloadClass::ExactCritical));
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert!(!budget.can_acquire(WorkloadClass::ExactCritical));
    }

    #[test]
    fn budget_classes_dont_interfere() {
        let budget = WorkloadBudget::new(1, 1, 1, 1);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert!(budget.can_acquire(WorkloadClass::Standard));
    }

    #[test]
    fn budget_total_reserved_and_used() {
        let budget = WorkloadBudget::new(10, 20, 8, 5);
        assert_eq!(budget.total_reserved(), 43);
        assert_eq!(budget.total_used(), 0);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert_eq!(budget.total_used(), 1);
    }

    #[test]
    fn budget_reserved_for() {
        let budget = WorkloadBudget::new(10, 20, 8, 5);
        assert_eq!(budget.reserved_for(WorkloadClass::ExactCritical), 10);
    }

    #[test]
    fn budget_default_budget() {
        let budget = WorkloadBudget::default_budget();
        assert!(budget.total_reserved() > 0);
        for class in WorkloadClass::all_by_priority() {
            assert!(budget.can_acquire(*class));
        }
    }

    // ── RejectionDetail ────────────────────────────────────────────────────

    #[test]
    fn rejection_detail_budget_exhausted() {
        let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
        assert_eq!(detail.reason, RejectionReason::BudgetExhausted);
    }

    #[test]
    fn rejection_detail_workflow_cap() {
        let detail = RejectionDetail::workflow_cap_exceeded(WorkloadClass::Standard);
        assert_eq!(detail.reason, RejectionReason::WorkflowCapExceeded);
    }

    #[test]
    fn rejection_detail_global_limit() {
        let detail = RejectionDetail::global_limit(WorkloadClass::ExactCritical);
        assert_eq!(detail.reason, RejectionReason::GlobalConcurrencyLimit);
    }

    #[test]
    fn rejection_detail_display_includes_class() {
        let detail = RejectionDetail::budget_exhausted(WorkloadClass::UnsafeBulk);
        let msg = detail.to_string();
        assert!(msg.contains("UnsafeBulk"));
        assert!(msg.contains("budget exhausted"));
    }

    // ── is_non_critical / is_protected ─────────────────────────────────────

    #[test]
    fn exact_critical_is_protected() {
        assert!(WorkloadClass::ExactCritical.is_protected());
        assert!(!WorkloadClass::ExactCritical.is_non_critical());
    }

    #[test]
    fn recovery_is_protected() {
        assert!(WorkloadClass::Recovery.is_protected());
        assert!(!WorkloadClass::Recovery.is_non_critical());
    }

    #[test]
    fn standard_is_non_critical() {
        assert!(WorkloadClass::Standard.is_non_critical());
        assert!(!WorkloadClass::Standard.is_protected());
    }

    #[test]
    fn unsafe_bulk_is_non_critical() {
        assert!(WorkloadClass::UnsafeBulk.is_non_critical());
        assert!(!WorkloadClass::UnsafeBulk.is_protected());
    }

    // ── DegradedBudget ─────────────────────────────────────────────────────

    #[test]
    fn degraded_budget_starts_non_degraded() {
        let budget = DegradedBudget::default_budget();
        assert!(!budget.is_degraded());
    }

    #[test]
    fn degraded_budget_enter_exits_degraded() {
        let mut budget = DegradedBudget::default_budget();
        assert!(!budget.is_degraded());
        budget.enter_degraded();
        assert!(budget.is_degraded());
        budget.exit_degraded();
        assert!(!budget.is_degraded());
    }

    #[test]
    fn protected_class_admitted_during_degraded_mode() {
        let budget = DegradedBudget::new(10, 0, 10, 0);
        assert!(budget.can_acquire(WorkloadClass::ExactCritical));
        assert!(budget.can_acquire(WorkloadClass::Recovery));
    }

    #[test]
    fn non_critical_class_rejected_during_degraded_mode() {
        let mut budget = DegradedBudget::new(10, 20, 10, 20);
        budget.enter_degraded();
        assert!(!budget.can_acquire(WorkloadClass::Standard));
        assert!(!budget.can_acquire(WorkloadClass::UnsafeBulk));
    }

    #[test]
    fn high_priority_class_admitted_during_degraded_mode() {
        let mut budget = DegradedBudget::new(50, 200, 30, 20);
        budget.enter_degraded();
        assert!(budget.can_acquire(WorkloadClass::ExactCritical));
        assert!(budget.can_acquire(WorkloadClass::Recovery));
    }

    #[test]
    fn low_priority_class_rejected_during_degraded_mode() {
        let mut budget = DegradedBudget::new(50, 200, 30, 20);
        budget.enter_degraded();
        assert!(!budget.can_acquire(WorkloadClass::Standard));
        assert!(!budget.can_acquire(WorkloadClass::UnsafeBulk));
    }

    #[test]
    fn acquire_protected_class_during_degraded_mode() {
        let mut budget = DegradedBudget::new(10, 0, 10, 0);
        budget.enter_degraded();
        assert!(budget.acquire(WorkloadClass::ExactCritical).is_ok());
        assert!(budget.acquire(WorkloadClass::Recovery).is_ok());
    }

    #[test]
    fn acquire_non_critical_class_fails_during_degraded_mode() {
        let mut budget = DegradedBudget::new(10, 20, 10, 20);
        budget.enter_degraded();
        let err = budget.acquire(WorkloadClass::Standard).unwrap_err();
        assert!(matches!(
            err,
            WorkloadClassError::BudgetExceeded {
                class: WorkloadClass::Standard,
                ..
            }
        ));
        let err = budget.acquire(WorkloadClass::UnsafeBulk).unwrap_err();
        assert!(matches!(
            err,
            WorkloadClassError::BudgetExceeded {
                class: WorkloadClass::UnsafeBulk,
                ..
            }
        ));
    }

    #[test]
    fn remaining_returns_zero_for_non_critical_when_degraded() {
        let mut budget = DegradedBudget::new(10, 20, 10, 20);
        budget.enter_degraded();
        assert_eq!(budget.remaining(WorkloadClass::Standard), 0);
        assert_eq!(budget.remaining(WorkloadClass::UnsafeBulk), 0);
    }

    #[test]
    fn remaining_preserved_for_protected_when_degraded() {
        let mut budget = DegradedBudget::new(10, 20, 10, 20);
        budget.enter_degraded();
        assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
        assert_eq!(budget.remaining(WorkloadClass::Recovery), 10);
    }

    #[test]
    fn degraded_budget_respects_inner_workload_budget_exhaustion() {
        let mut budget = DegradedBudget::new(1, 0, 0, 0);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        assert!(!budget.can_acquire(WorkloadClass::ExactCritical));
        assert!(!budget.can_acquire(WorkloadClass::Standard));
        budget.enter_degraded();
        assert!(!budget.can_acquire(WorkloadClass::ExactCritical));
        assert!(!budget.can_acquire(WorkloadClass::Standard));
    }

    #[test]
    fn degraded_budget_default_has_sensible_values() {
        let budget = DegradedBudget::default_budget();
        assert!(budget.total_reserved() > 0);
        assert!(!budget.is_degraded());
        for class in WorkloadClass::all_by_priority() {
            assert!(budget.inner().can_acquire(*class));
        }
    }

    #[test]
    fn release_works_normally_during_degraded_mode() {
        let mut budget = DegradedBudget::new(10, 20, 10, 20);
        budget.acquire(WorkloadClass::ExactCritical).unwrap();
        budget.acquire(WorkloadClass::Recovery).unwrap();
        budget.enter_degraded();
        budget.release(WorkloadClass::ExactCritical);
        budget.release(WorkloadClass::Recovery);
        assert_eq!(budget.remaining(WorkloadClass::ExactCritical), 10);
        assert_eq!(budget.remaining(WorkloadClass::Recovery), 10);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DegradedBudget — Degraded Mode Budget Restrictions (ADR-013)
// ─────────────────────────────────────────────────────────────────────────────

/// Budget wrapper that enforces degraded-mode admission per ADR-013.
///
/// When degraded mode is active, non-critical workload classes are restricted:
/// - `ExactCritical` and `Recovery` remain fully available (protected classes)
/// - `Standard` and `UnsafeBulk` are blocked (non-critical classes)
///
/// # Invariants
/// - `inner` is always valid (WorkloadBudget invariant preserved)
/// - When `degraded` is `true`, only protected classes can acquire permits
#[derive(Clone, Debug)]
pub struct DegradedBudget {
    inner: WorkloadBudget,
    degraded: bool,
}

impl DegradedBudget {
    /// Creates a new `DegradedBudget` with the given reserved permit counts.
    #[must_use]
    pub fn new(exact_critical: u32, standard: u32, recovery: u32, unsafe_bulk: u32) -> Self {
        Self {
            inner: WorkloadBudget::new(exact_critical, standard, recovery, unsafe_bulk),
            degraded: false,
        }
    }

    /// Creates a `DegradedBudget` with sensible defaults for a medium-scale deployment.
    #[must_use]
    pub fn default_budget() -> Self {
        Self::new(50, 200, 30, 20)
    }

    /// Returns the current degraded mode status.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Enters degraded mode, restricting non-critical workload classes.
    pub fn enter_degraded(&mut self) {
        self.degraded = true;
    }

    /// Exits degraded mode, restoring normal admission for all classes.
    pub fn exit_degraded(&mut self) {
        self.degraded = false;
    }

    /// Returns whether a permit can be acquired for the given class.
    ///
    /// Returns `false` for non-critical classes (`Standard`, `UnsafeBulk`)
    /// when degraded mode is active, regardless of remaining budget.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        if self.degraded && class.is_non_critical() {
            return false;
        }
        self.inner.can_acquire(class)
    }

    /// Acquires a permit for the given class.
    ///
    /// Returns an error if:
    /// - The class is non-critical and degraded mode is active
    /// - The class budget is exhausted
    pub fn acquire(&self, class: WorkloadClass) -> Result<(), WorkloadClassError> {
        if self.degraded && class.is_non_critical() {
            return Err(WorkloadClassError::BudgetExceeded {
                class,
                requested: 1,
                available: 0,
            });
        }
        self.inner.acquire(class)
    }

    /// Releases a previously acquired permit.
    pub fn release(&self, class: WorkloadClass) {
        self.inner.release(class)
    }

    /// Returns remaining permits for a given class.
    ///
    /// Returns 0 for non-critical classes when degraded mode is active.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        if self.degraded && class.is_non_critical() {
            return 0;
        }
        self.inner.remaining(class)
    }

    /// Returns the total reserved permits across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.inner.total_reserved()
    }

    /// Returns the total used permits across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.inner.total_used()
    }

    /// Returns the reserved permit count for a given class.
    #[must_use]
    pub fn reserved_for(&self, class: WorkloadClass) -> u32 {
        self.inner.reserved_for(class)
    }

    /// Returns the inner budget for cases that need direct access.
    #[must_use]
    pub fn inner(&self) -> &WorkloadBudget {
        &self.inner
    }
}

impl WorkloadClass {
    /// Returns `true` if this class is non-critical and subject to degraded-mode restrictions.
    ///
    /// Non-critical classes are `Standard` and `UnsafeBulk`.
    /// Protected classes (`ExactCritical`, `Recovery`) are never restricted by degraded mode.
    #[must_use]
    pub fn is_non_critical(self) -> bool {
        matches!(self, WorkloadClass::Standard | WorkloadClass::UnsafeBulk)
    }

    /// Returns `true` if this class is protected and always admitted during degraded mode.
    ///
    /// Protected classes are `ExactCritical` and `Recovery`.
    #[must_use]
    pub fn is_protected(self) -> bool {
        !self.is_non_critical()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Proptest Invariants
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(unused_doc_comments)]
mod proptest_workload_invariants {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn rank_in_range(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            prop_assert!((0..=3u8).contains(&variant.rank()));
        }
    }

    proptest! {
        #[test]
        fn never_starved_matches_protected(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            let never = variant.never_starved();
            let is_protected = matches!(variant, WorkloadClass::ExactCritical | WorkloadClass::Recovery);
            prop_assert_eq!(never, is_protected);
        }
    }

    proptest! {
        #[test]
        fn as_str_roundtrips(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            prop_assert_eq!(WorkloadClass::parse(variant.as_str()), Ok(variant));
        }
    }

    proptest! {
        #[test]
        fn json_roundtrip(variant in proptest::sample::select(
            WorkloadClass::all_by_priority().to_vec()
        )) {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WorkloadClass = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(parsed, variant);
        }
    }

    proptest! {
        #[test]
        fn budget_never_negative(
            reserved in 0u32..=100,
            acquires in 0u32..=100,
            releases in 0u32..=100,
        ) {
            let class = WorkloadClass::Standard;
            let budget = WorkloadBudget::new(reserved, reserved, reserved, reserved);
            for _ in 0..acquires { let _ = budget.acquire(class); }
            for _ in 0..releases { budget.release(class); }
            prop_assert!(budget.remaining(class) <= reserved);
        }
    }

    proptest! {
        #[test]
        fn can_acquire_consistent(reserved in 1u32..=50) {
            let class = WorkloadClass::ExactCritical;
            let budget = WorkloadBudget::new(reserved, 0, 0, 0);
            for _ in 0..reserved {
                let can = budget.can_acquire(class);
                let result = budget.acquire(class);
                prop_assert_eq!(can, result.is_ok());
            }
            prop_assert!(!budget.can_acquire(class));
        }
    }
}
