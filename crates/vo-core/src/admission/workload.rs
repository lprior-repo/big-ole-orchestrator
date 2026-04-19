//! Workload class taxonomy for budget-based admission control per ADR-013.
//!
//! This module implements the coupling between write pressure indicators and
//! degraded mode admission. When storage health degrades, non-critical workload
//! classes are restricted while critical classes (Live, Recovery) preserve budgets.
//!
//! # Workload Classes
//!
//! - **Live**: Highest priority, receives reserved budget, never rejected in degraded mode
//! - **Recovery**: Reserved budget for crash recovery, cannot starve Live
//! - **TimerResume**: Shares budget with Recovery
//! - **NonCritical**: First to be rejected in degraded mode
//! - **Background**: Deferred in degraded mode (blob writes, projections)
//!
//! # Degraded Mode State Machine
//!
//! - **Normal**: All classes accepted
//! - **Degraded**: NonCritical and Background restricted
//! - **Critical**: Only Live and Recovery accepted

use serde::{Deserialize, Serialize};

use super::types::PressureIndicator;

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadClass Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Workload classification for budget-based admission per ADR-013.
///
/// Variants are ordered by priority (highest first) for admission decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadClass {
    /// Highest priority. Never rejected in any degraded mode.
    Live,
    /// Reserved capacity for crash recovery.
    Recovery,
    /// Timer-based resume, shares budget with Recovery.
    TimerResume,
    /// First to be rejected when degraded.
    NonCritical,
    /// Background tasks (blob writes, projections), deferred in degraded mode.
    Background,
}

impl WorkloadClass {
    /// Returns `true` if this class is never starved (always gets budget).
    #[must_use]
    pub fn never_starved(self) -> bool {
        matches!(self, WorkloadClass::Live | WorkloadClass::Recovery)
    }

    /// Returns `true` if this class is deferred in degraded mode.
    #[must_use]
    pub fn is_deferred_in_degraded(self) -> bool {
        matches!(self, WorkloadClass::NonCritical | WorkloadClass::Background)
    }

    /// Returns `true` if this class is accepted in Critical degraded mode.
    ///
    /// Only Live and Recovery are accepted when system is critical.
    #[must_use]
    pub fn is_accepted_in_critical(self) -> bool {
        matches!(self, WorkloadClass::Live | WorkloadClass::Recovery)
    }

    /// Returns all variants ordered by priority (highest first).
    #[must_use]
    pub fn all_by_priority() -> &'static [WorkloadClass] {
        &[
            WorkloadClass::Live,
            WorkloadClass::Recovery,
            WorkloadClass::TimerResume,
            WorkloadClass::NonCritical,
            WorkloadClass::Background,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DegradedMode Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Degraded mode state machine per ADR-013.
///
/// Represents the system's resilience state based on storage health indicators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "triggers", rename_all = "snake_case")]
pub enum DegradedMode {
    /// Normal operation — all workload classes accepted.
    Normal,
    /// Pressure detected — non-critical classes restricted.
    Degraded {
        /// Which pressure indicators triggered degraded mode.
        triggers: Vec<PressureIndicator>,
    },
    /// Critical pressure — only Live and Recovery accepted.
    Critical {
        /// Which pressure indicators triggered critical mode.
        triggers: Vec<PressureIndicator>,
    },
}

impl DegradedMode {
    /// Returns `true` if this mode is Normal.
    #[must_use]
    pub fn is_normal(self) -> bool {
        matches!(self, DegradedMode::Normal)
    }

    /// Returns `true` if this mode is Degraded or Critical.
    #[must_use]
    pub fn is_degraded(self) -> bool {
        matches!(
            self,
            DegradedMode::Degraded { .. } | DegradedMode::Critical { .. }
        )
    }

    /// Returns the triggers that caused this degraded mode.
    #[must_use]
    pub fn triggers(self) -> Vec<PressureIndicator> {
        match self {
            DegradedMode::Normal => Vec::new(),
            DegradedMode::Degraded { triggers } | DegradedMode::Critical { triggers } => triggers,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetAllocation Struct
// ─────────────────────────────────────────────────────────────────────────────

/// Budget allocation for a single workload class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetAllocation {
    /// The workload class this allocation is for.
    pub class: WorkloadClass,
    /// Maximum concurrent slots for this class.
    pub max_slots: u32,
    /// Currently used slots.
    pub used_slots: u32,
    /// Minimum reserved slots (cannot be borrowed by other classes).
    pub reserved_min: u32,
}

impl BudgetAllocation {
    /// Creates a new budget allocation.
    #[must_use]
    pub fn new(class: WorkloadClass, max_slots: u32, reserved_min: u32) -> Self {
        Self {
            class,
            max_slots,
            used_slots: 0,
            reserved_min,
        }
    }

    /// Returns the remaining slots for this class.
    #[must_use]
    pub fn remaining(&self) -> u32 {
        self.max_slots.saturating_sub(self.used_slots)
    }

    /// Returns `true` if a slot can be acquired for this class.
    #[must_use]
    pub fn can_acquire(&self) -> bool {
        self.used_slots < self.max_slots
    }

    /// Returns `true` if this allocation is exhausted.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.used_slots >= self.max_slots
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WorkloadBudget Struct
// ─────────────────────────────────────────────────────────────────────────────

/// Total budget across all workload classes per ADR-013.
///
/// Tracks per-class slot usage and degraded mode state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadBudget {
    /// Per-class allocations ordered by priority.
    allocations: Vec<BudgetAllocation>,
    /// Total maximum slots across all classes.
    total_max_slots: u32,
    /// Total currently used slots.
    total_used_slots: u32,
    /// Current degraded mode state.
    degraded_mode: DegradedMode,
}

impl Default for WorkloadBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkloadBudget {
    /// Creates a new workload budget with sensible defaults.
    ///
    /// Default allocation per class:
    /// - Live: 50 slots, 50 reserved
    /// - Recovery: 30 slots, 30 reserved
    /// - TimerResume: 20 slots, 10 reserved
    /// - NonCritical: 100 slots, 0 reserved
    /// - Background: 200 slots, 0 reserved
    #[must_use]
    pub fn new() -> Self {
        let allocations = vec![
            BudgetAllocation::new(WorkloadClass::Live, 50, 50),
            BudgetAllocation::new(WorkloadClass::Recovery, 30, 30),
            BudgetAllocation::new(WorkloadClass::TimerResume, 20, 10),
            BudgetAllocation::new(WorkloadClass::NonCritical, 100, 0),
            BudgetAllocation::new(WorkloadClass::Background, 200, 0),
        ];
        let total_max_slots: u32 = allocations.iter().map(|a| a.max_slots).sum();
        Self {
            allocations,
            total_max_slots,
            total_used_slots: 0,
            degraded_mode: DegradedMode::Normal,
        }
    }

    /// Creates a budget with custom allocations.
    ///
    /// The arrays must have exactly 5 elements matching the 5 WorkloadClass variants
    /// in priority order: Live, Recovery, TimerResume, NonCritical, Background.
    #[must_use]
    pub fn with_allocations(max_slots: [u32; 5], reserved_min: [u32; 5]) -> Self {
        let classes = WorkloadClass::all_by_priority();
        let allocations: Vec<BudgetAllocation> = classes
            .iter()
            .zip(max_slots.iter().zip(reserved_min.iter()))
            .map(|(class, (max, reserved))| BudgetAllocation::new(*class, *max, *reserved))
            .collect();
        let total_max_slots: u32 = allocations.iter().map(|a| a.max_slots).sum();
        Self {
            allocations,
            total_max_slots,
            total_used_slots: 0,
            degraded_mode: DegradedMode::Normal,
        }
    }

    /// Returns the allocation for a specific class.
    #[must_use]
    pub fn allocation_for(&self, class: WorkloadClass) -> Option<&BudgetAllocation> {
        self.allocations.iter().find(|a| a.class == class)
    }

    /// Returns the remaining slots for a specific class.
    #[must_use]
    pub fn remaining(&self, class: WorkloadClass) -> u32 {
        self.allocation_for(class)
            .map(|a| a.remaining())
            .unwrap_or(0)
    }

    /// Returns `true` if a slot can be acquired for the given class.
    #[must_use]
    pub fn can_acquire(&self, class: WorkloadClass) -> bool {
        match self.allocation_for(class) {
            Some(a) => a.can_acquire(),
            None => false,
        }
    }

    /// Returns the current degraded mode.
    #[must_use]
    pub fn degraded_mode(&self) -> DegradedMode {
        self.degraded_mode.clone()
    }

    /// Returns the total reserved slots across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.allocations.iter().map(|a| a.reserved_min).sum()
    }

    /// Returns the total used slots across all classes.
    #[must_use]
    pub fn total_used(&self) -> u32 {
        self.total_used_slots
    }

    /// Returns the total maximum slots.
    #[must_use]
    pub fn total_max(&self) -> u32 {
        self.total_max_slots
    }

    /// Returns all allocations.
    #[must_use]
    pub fn allocations(&self) -> &[BudgetAllocation] {
        &self.allocations
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetCheckResult Enum
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a budget check for a workload class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetCheckResult {
    /// Slot available, work accepted.
    Accepted { remaining: u32 },
    /// No slots available for this class.
    Rejected { reason: BudgetRejectionReason },
    /// Class is blocked by degraded mode.
    DegradedBlocked { mode: DegradedMode },
}

/// Reasons for budget rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetRejectionReason {
    /// All slots for this class are in use.
    SlotsExhausted { class: WorkloadClass, max: u32 },
    /// Global budget exhausted (total across all classes).
    GlobalBudgetExhausted,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure Calculation Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a workload class can acquire a slot.
///
/// Pure function, no I/O or side effects.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class requesting admission
///
/// # Returns
/// `BudgetCheckResult::Accepted` if slot is available,
/// `BudgetCheckResult::Rejected` if exhausted,
/// `BudgetCheckResult::DegradedBlocked` if degraded mode prevents admission.
#[must_use]
pub fn check_budget(budget: &WorkloadBudget, class: WorkloadClass) -> BudgetCheckResult {
    if !is_class_accepted_in_mode(class, budget.degraded_mode()) {
        return BudgetCheckResult::DegradedBlocked {
            mode: budget.degraded_mode(),
        };
    }

    match budget.allocation_for(class) {
        Some(allocation) if allocation.can_acquire() => BudgetCheckResult::Accepted {
            remaining: allocation.remaining(),
        },
        Some(allocation) => BudgetCheckResult::Rejected {
            reason: BudgetRejectionReason::SlotsExhausted {
                class,
                max: allocation.max_slots,
            },
        },
        None => BudgetCheckResult::Rejected {
            reason: BudgetRejectionReason::SlotsExhausted { class, max: 0 },
        },
    }
}

/// Acquire a slot for a workload class.
///
/// Pure function - returns a new budget with the slot reserved.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class acquiring a slot
///
/// # Returns
/// `Ok(updated_budget)` if slot acquired, `Err(reason)` if rejected.
pub fn acquire_slot(
    budget: &WorkloadBudget,
    class: WorkloadClass,
) -> Result<WorkloadBudget, BudgetRejectionReason> {
    match check_budget(budget, class) {
        BudgetCheckResult::Accepted { .. } => {
            let mut new_allocations = budget.allocations().to_vec();
            if let Some(idx) = new_allocations.iter().position(|a| a.class == class) {
                new_allocations[idx].used_slots += 1;
            }
            Ok(WorkloadBudget {
                allocations: new_allocations,
                total_max_slots: budget.total_max_slots,
                total_used_slots: budget.total_used_slots + 1,
                degraded_mode: budget.degraded_mode.clone(),
            })
        }
        BudgetCheckResult::Rejected { reason } => Err(reason),
        BudgetCheckResult::DegradedBlocked { .. } => {
            Err(BudgetRejectionReason::SlotsExhausted { class, max: 0 })
        }
    }
}

/// Release a slot for a workload class.
///
/// Pure function - returns a new budget with the slot freed.
///
/// # Arguments
/// * `budget` - The current workload budget
/// * `class` - The workload class releasing a slot
///
/// # Returns
/// Updated budget with the slot released. Never fails.
#[must_use]
pub fn release_slot(budget: &WorkloadBudget, class: WorkloadClass) -> WorkloadBudget {
    let mut new_allocations = budget.allocations().to_vec();
    if let Some(idx) = new_allocations.iter().position(|a| a.class == class) {
        new_allocations[idx].used_slots = new_allocations[idx].used_slots.saturating_sub(1);
    }
    WorkloadBudget {
        allocations: new_allocations,
        total_max_slots: budget.total_max_slots,
        total_used_slots: budget.total_used_slots.saturating_sub(1),
        degraded_mode: budget.degraded_mode.clone(),
    }
}

/// Compute degraded mode from pressure state.
///
/// Pure function that determines the appropriate degraded mode based on
/// which pressure indicators are active.
///
/// # Arguments
/// * `pressure` - Current write pressure state
///
/// # Returns
/// `DegradedMode::Normal` if no indicators active,
/// `DegradedMode::Degraded` if 1-2 indicators,
/// `DegradedMode::Critical` if 3+ indicators or critical stalls active.
#[must_use]
pub fn compute_degraded_mode(pressure: &super::types::WritePressureState) -> DegradedMode {
    use super::types::PressureIndicator;

    let mut indicators = Vec::new();

    if pressure.writer_queue_depth > 0 {
        indicators.push(PressureIndicator::WriterQueueDepth);
    }
    if pressure.batch_commit_latency_ms > 0 {
        indicators.push(PressureIndicator::BatchCommitLatency);
    }
    if pressure.blob_queue_depth > 0 {
        indicators.push(PressureIndicator::BlobQueueDepth);
    }
    if pressure.compaction_stall_active {
        indicators.push(PressureIndicator::CompactionStall);
    }
    if pressure.storage_stall_active {
        indicators.push(PressureIndicator::StorageStall);
    }

    match indicators.len() {
        0 => DegradedMode::Normal,
        1..=2 => DegradedMode::Degraded {
            triggers: indicators,
        },
        _ => DegradedMode::Critical {
            triggers: indicators,
        },
    }
}

/// Check if a class is accepted in the given degraded mode.
///
/// Pure function that encodes the admission rules:
///
/// - Normal: All classes accepted
/// - Degraded: Live, Recovery, TimerResume accepted; NonCritical, Background restricted
/// - Critical: Only Live, Recovery accepted
#[must_use]
pub fn is_class_accepted_in_mode(class: WorkloadClass, mode: DegradedMode) -> bool {
    match mode {
        DegradedMode::Normal => true,
        DegradedMode::Degraded { .. } => !class.is_deferred_in_degraded(),
        DegradedMode::Critical { .. } => class.is_accepted_in_critical(),
    }
}

/// Set the degraded mode on a budget.
///
/// Pure function - returns a new budget with the updated mode.
#[must_use]
pub fn set_degraded_mode(budget: WorkloadBudget, mode: DegradedMode) -> WorkloadBudget {
    WorkloadBudget {
        allocations: budget.allocations,
        total_max_slots: budget.total_max_slots,
        total_used_slots: budget.total_used_slots,
        degraded_mode: mode,
    }
}

#[cfg(test)]
mod priority_inheritance_tests {
    use super::*;
    use crate::admission::types::{PressureIndicator, WritePressureState};

    // ========================================================================
    // Priority Inheritance Through Degraded Mode (ve-9rko8)
    // ========================================================================

    /// Given: System in Normal mode
    /// When: All workload classes request admission
    /// Then: All classes are accepted (full priority inheritance)
    #[test]
    fn all_classes_accepted_in_normal_mode() {
        for class in WorkloadClass::all_by_priority() {
            assert!(
                is_class_accepted_in_mode(*class, DegradedMode::Normal),
                "{class:?} should be accepted in Normal mode"
            );
        }
    }

    /// Given: System in Degraded mode
    /// When: High-priority classes (Live, Recovery, TimerResume) request admission
    /// Then: They are accepted (priority inheritance preserves their admission)
    #[test]
    fn high_priority_classes_inherit_admission_in_degraded_mode() {
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };

        assert!(
            is_class_accepted_in_mode(WorkloadClass::Live, degraded.clone()),
            "Live must inherit priority through Degraded mode"
        );
        assert!(
            is_class_accepted_in_mode(WorkloadClass::Recovery, degraded.clone()),
            "Recovery must inherit priority through Degraded mode"
        );
        assert!(
            is_class_accepted_in_mode(WorkloadClass::TimerResume, degraded.clone()),
            "TimerResume must inherit priority through Degraded mode"
        );
    }

    /// Given: System in Degraded mode
    /// When: Low-priority classes (NonCritical, Background) request admission
    /// Then: They are blocked (priority does not escalate)
    #[test]
    fn low_priority_classes_blocked_in_degraded_mode() {
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };

        assert!(
            !is_class_accepted_in_mode(WorkloadClass::NonCritical, degraded.clone()),
            "NonCritical must be blocked in Degraded mode"
        );
        assert!(
            !is_class_accepted_in_mode(WorkloadClass::Background, degraded),
            "Background must be blocked in Degraded mode"
        );
    }

    /// Given: System in Critical mode
    /// When: Only Live and Recovery request admission
    /// Then: They are accepted (highest priority inheritance)
    #[test]
    fn only_live_and_recovery_survive_critical_mode() {
        let critical = DegradedMode::Critical {
            triggers: vec![
                PressureIndicator::WriterQueueDepth,
                PressureIndicator::BatchCommitLatency,
                PressureIndicator::StorageStall,
            ],
        };

        assert!(
            is_class_accepted_in_mode(WorkloadClass::Live, critical.clone()),
            "Live must survive Critical mode"
        );
        assert!(
            is_class_accepted_in_mode(WorkloadClass::Recovery, critical.clone()),
            "Recovery must survive Critical mode"
        );
        assert!(
            !is_class_accepted_in_mode(WorkloadClass::TimerResume, critical.clone()),
            "TimerResume must be blocked in Critical mode"
        );
        assert!(
            !is_class_accepted_in_mode(WorkloadClass::NonCritical, critical.clone()),
            "NonCritical must be blocked in Critical mode"
        );
        assert!(
            !is_class_accepted_in_mode(WorkloadClass::Background, critical),
            "Background must be blocked in Critical mode"
        );
    }

    /// Given: A budget with slots available
    /// When: check_budget is called for each class in Normal mode
    /// Then: All classes are Accepted
    #[test]
    fn budget_check_all_accepted_normal() {
        let budget = WorkloadBudget::new();
        for class in WorkloadClass::all_by_priority() {
            match check_budget(&budget, *class) {
                BudgetCheckResult::Accepted { .. } => {}
                other => panic!("{class:?} should be Accepted, got {other:?}"),
            }
        }
    }

    /// Given: A budget in Degraded mode
    /// When: check_budget is called for NonCritical
    /// Then: Result is DegradedBlocked
    #[test]
    fn budget_check_degraded_blocks_noncritical() {
        let budget = WorkloadBudget::new();
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::BlobQueueDepth],
        };
        let budget = set_degraded_mode(budget, degraded);

        match check_budget(&budget, WorkloadClass::NonCritical) {
            BudgetCheckResult::DegradedBlocked { mode } => {
                assert!(mode.is_degraded());
            }
            other => panic!("Expected DegradedBlocked, got {other:?}"),
        }
    }

    /// Given: Priority escalation chain: Normal → Degraded → Critical
    /// When: System transitions through each mode
    /// Then: Accepted classes shrink at each step (priority inheritance narrows)
    #[test]
    fn priority_escalation_chain_narrows_accepted_classes() {
        let all_classes = WorkloadClass::all_by_priority();

        // Normal: all accepted
        let normal_count = all_classes
            .iter()
            .filter(|c| is_class_accepted_in_mode(**c, DegradedMode::Normal))
            .count();
        assert_eq!(normal_count, 5);

        // Degraded: 3 accepted (Live, Recovery, TimerResume)
        let degraded = DegradedMode::Degraded {
            triggers: vec![PressureIndicator::WriterQueueDepth],
        };
        let degraded_count = all_classes
            .iter()
            .filter(|c| is_class_accepted_in_mode(**c, degraded.clone()))
            .count();
        assert_eq!(degraded_count, 3);

        // Critical: 2 accepted (Live, Recovery)
        let critical = DegradedMode::Critical {
            triggers: vec![
                PressureIndicator::WriterQueueDepth,
                PressureIndicator::BatchCommitLatency,
                PressureIndicator::StorageStall,
            ],
        };
        let critical_count = all_classes
            .iter()
            .filter(|c| is_class_accepted_in_mode(**c, critical.clone()))
            .count();
        assert_eq!(critical_count, 2);
    }

    /// Given: Never-starved classes
    /// When: Checking which classes have reserved budget
    /// Then: Only Live and Recovery are never-starved
    #[test]
    fn never_starved_classes_match_critical_survivors() {
        let never_starved: Vec<_> = WorkloadClass::all_by_priority()
            .iter()
            .filter(|c| c.never_starved())
            .copied()
            .collect();

        assert_eq!(never_starved, vec![WorkloadClass::Live, WorkloadClass::Recovery]);

        // These are exactly the classes that survive Critical mode
        for class in &never_starved {
            assert!(class.is_accepted_in_critical());
        }
    }
}
