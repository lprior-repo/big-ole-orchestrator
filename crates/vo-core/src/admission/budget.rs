//! Budget data types for admission-level budget allocation.
//!
//! This module implements the **Data** layer of the D-C-A (Data-Calc-Actions)
//! pattern for admission budget management per ADR-013 (System Resilience)
//! and ADR-033 (Fairness and Workload Classes).
//!
//! All types here are pure data — no side effects, no mutation.
//! See `budget_ops` for the pure calculation functions operating on these types.

use serde::{Deserialize, Serialize};

use super::workload::{DegradedMode, WorkloadClass};

// ─────────────────────────────────────────────────────────────────────────────
// ClassBudgetConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Budget configuration for a single workload class.
///
/// Defines the maximum concurrent slots, reserved minimum, and scheduling
/// weight for fair-share allocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassBudgetConfig {
    /// The workload class this configuration applies to.
    pub class: WorkloadClass,
    /// Maximum concurrent slots for this class.
    pub max_slots: u32,
    /// Minimum reserved slots that cannot be borrowed by other classes.
    pub reserved_min: u32,
    /// Scheduling weight for fair-share allocation (higher = more shares).
    pub weight: u32,
}

impl ClassBudgetConfig {
    /// Creates a new class budget configuration.
    #[must_use]
    pub fn new(class: WorkloadClass, max_slots: u32, reserved_min: u32, weight: u32) -> Self {
        Self {
            class,
            max_slots,
            reserved_min,
            weight,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AdmissionBudgetConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the admission budget system.
///
/// Parameterizes per-class slot allocation and degraded-mode thresholds
/// per ADR-013 and ADR-033.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionBudgetConfig {
    /// Per-class budget configurations ordered by priority.
    pub class_configs: Vec<ClassBudgetConfig>,
    /// Number of active pressure indicators to trigger Degraded mode.
    pub degraded_indicator_threshold: usize,
    /// Number of active pressure indicators to trigger Critical mode.
    pub critical_indicator_threshold: usize,
    /// Whether compaction or storage stall alone triggers Critical mode.
    pub critical_stall_is_critical: bool,
}

impl Default for AdmissionBudgetConfig {
    fn default() -> Self {
        Self::standard()
    }
}

impl AdmissionBudgetConfig {
    /// Standard configuration matching the default `WorkloadBudget` allocation.
    ///
    /// - Live: 50 slots, 50 reserved, weight 10
    /// - Recovery: 30 slots, 30 reserved, weight 8
    /// - TimerResume: 20 slots, 10 reserved, weight 5
    /// - NonCritical: 100 slots, 0 reserved, weight 2
    /// - Background: 200 slots, 0 reserved, weight 1
    #[must_use]
    pub fn standard() -> Self {
        Self {
            class_configs: vec![
                ClassBudgetConfig::new(WorkloadClass::Live, 50, 50, 10),
                ClassBudgetConfig::new(WorkloadClass::Recovery, 30, 30, 8),
                ClassBudgetConfig::new(WorkloadClass::TimerResume, 20, 10, 5),
                ClassBudgetConfig::new(WorkloadClass::NonCritical, 100, 0, 2),
                ClassBudgetConfig::new(WorkloadClass::Background, 200, 0, 1),
            ],
            degraded_indicator_threshold: 1,
            critical_indicator_threshold: 3,
            critical_stall_is_critical: true,
        }
    }

    /// Returns the configuration for a specific workload class.
    #[must_use]
    pub fn class_config(&self, class: WorkloadClass) -> Option<&ClassBudgetConfig> {
        self.class_configs.iter().find(|c| c.class == class)
    }

    /// Returns the total maximum slots across all classes.
    #[must_use]
    pub fn total_max_slots(&self) -> u32 {
        self.class_configs.iter().map(|c| c.max_slots).sum()
    }

    /// Returns the total reserved slots across all classes.
    #[must_use]
    pub fn total_reserved(&self) -> u32 {
        self.class_configs.iter().map(|c| c.reserved_min).sum()
    }

    /// Returns the total scheduling weight across all classes.
    #[must_use]
    pub fn total_weight(&self) -> u32 {
        self.class_configs.iter().map(|c| c.weight).sum()
    }

    /// Validates that reserved_min <= max_slots for every class.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.class_configs
            .iter()
            .all(|c| c.reserved_min <= c.max_slots)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetSnapshot
// ─────────────────────────────────────────────────────────────────────────────

/// Immutable point-in-time snapshot of budget utilization.
///
/// Captures the full state of the admission budget for observability,
/// metrics export, and throttling decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    /// Total slot capacity across all classes.
    pub total_capacity: u32,
    /// Total slots currently in use.
    pub total_used: u32,
    /// Total reserved slots across all classes.
    pub total_reserved: u32,
    /// Per-class utilization details.
    pub class_snapshots: Vec<ClassBudgetSnapshot>,
    /// Current degraded mode state.
    pub degraded_mode: DegradedMode,
}

impl BudgetSnapshot {
    /// Returns the overall utilization as permille (0–1000).
    ///
    /// 0 means completely idle, 1000 means fully utilized.
    #[must_use]
    pub fn utilization_permille(&self) -> u32 {
        if self.total_capacity == 0 {
            return 0;
        }
        (self.total_used as u64 * 1000 / self.total_capacity as u64) as u32
    }

    /// Returns `true` if total utilization exceeds the given permille threshold.
    #[must_use]
    pub fn is_above_threshold(&self, threshold_permille: u32) -> bool {
        self.utilization_permille() > threshold_permille
    }

    /// Returns the snapshot for a specific workload class.
    #[must_use]
    pub fn class_snapshot(&self, class: WorkloadClass) -> Option<&ClassBudgetSnapshot> {
        self.class_snapshots.iter().find(|s| s.class == class)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ClassBudgetSnapshot
// ─────────────────────────────────────────────────────────────────────────────

/// Per-class budget utilization snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassBudgetSnapshot {
    /// The workload class this snapshot describes.
    pub class: WorkloadClass,
    /// Maximum concurrent slots for this class.
    pub capacity: u32,
    /// Currently used slots.
    pub used: u32,
    /// Minimum reserved slots.
    pub reserved: u32,
    /// Remaining available slots.
    pub available: u32,
    /// Utilization as permille (0–1000).
    pub utilization_permille: u32,
}

impl ClassBudgetSnapshot {
    /// Returns `true` if this class has no remaining capacity.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.available == 0
    }

    /// Returns `true` if utilization is above the given permille threshold.
    #[must_use]
    pub fn is_above_threshold(&self, threshold_permille: u32) -> bool {
        self.utilization_permille > threshold_permille
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BudgetThresholds
// ─────────────────────────────────────────────────────────────────────────────

/// Thresholds for budget-based throttling decisions.
///
/// Used by the Calc layer to determine when classes should be throttled
/// or when the system should transition degraded modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetThresholds {
    /// Permille (0–1000) at which a class is considered high utilization.
    pub high_utilization_permille: u32,
    /// Permille at which a class is considered critical utilization.
    pub critical_utilization_permille: u32,
    /// Global permille at which new admissions should be throttled.
    pub global_throttle_permille: u32,
}

impl Default for BudgetThresholds {
    fn default() -> Self {
        Self {
            high_utilization_permille: 800,
            critical_utilization_permille: 950,
            global_throttle_permille: 900,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_config_is_valid() {
        let config = AdmissionBudgetConfig::standard();
        assert!(config.is_valid());
    }

    #[test]
    fn standard_config_has_five_classes() {
        let config = AdmissionBudgetConfig::standard();
        assert_eq!(config.class_configs.len(), 5);
    }

    #[test]
    fn standard_config_total_slots() {
        let config = AdmissionBudgetConfig::standard();
        assert_eq!(config.total_max_slots(), 400);
    }

    #[test]
    fn standard_config_total_reserved() {
        let config = AdmissionBudgetConfig::standard();
        assert_eq!(config.total_reserved(), 90);
    }

    #[test]
    fn class_config_lookup() {
        let config = AdmissionBudgetConfig::standard();
        let live = config.class_config(WorkloadClass::Live).unwrap();
        assert_eq!(live.max_slots, 50);
        assert_eq!(live.reserved_min, 50);
        assert_eq!(live.weight, 10);
    }

    #[test]
    fn budget_snapshot_utilization_permille_zero_capacity() {
        let snapshot = BudgetSnapshot {
            total_capacity: 0,
            total_used: 0,
            total_reserved: 0,
            class_snapshots: vec![],
            degraded_mode: DegradedMode::Normal,
        };
        assert_eq!(snapshot.utilization_permille(), 0);
    }

    #[test]
    fn budget_snapshot_utilization_permille_half() {
        let snapshot = BudgetSnapshot {
            total_capacity: 100,
            total_used: 50,
            total_reserved: 50,
            class_snapshots: vec![],
            degraded_mode: DegradedMode::Normal,
        };
        assert_eq!(snapshot.utilization_permille(), 500);
    }

    #[test]
    fn budget_snapshot_utilization_permille_full() {
        let snapshot = BudgetSnapshot {
            total_capacity: 100,
            total_used: 100,
            total_reserved: 50,
            class_snapshots: vec![],
            degraded_mode: DegradedMode::Normal,
        };
        assert_eq!(snapshot.utilization_permille(), 1000);
    }

    #[test]
    fn budget_snapshot_is_above_threshold() {
        let snapshot = BudgetSnapshot {
            total_capacity: 100,
            total_used: 95,
            total_reserved: 50,
            class_snapshots: vec![],
            degraded_mode: DegradedMode::Normal,
        };
        assert!(snapshot.is_above_threshold(900));
        assert!(!snapshot.is_above_threshold(960));
    }

    #[test]
    fn class_budget_snapshot_is_exhausted() {
        let snap = ClassBudgetSnapshot {
            class: WorkloadClass::Live,
            capacity: 50,
            used: 50,
            reserved: 50,
            available: 0,
            utilization_permille: 1000,
        };
        assert!(snap.is_exhausted());
    }

    #[test]
    fn class_budget_snapshot_not_exhausted() {
        let snap = ClassBudgetSnapshot {
            class: WorkloadClass::Live,
            capacity: 50,
            used: 25,
            reserved: 50,
            available: 25,
            utilization_permille: 500,
        };
        assert!(!snap.is_exhausted());
    }

    #[test]
    fn class_budget_snapshot_above_threshold() {
        let snap = ClassBudgetSnapshot {
            class: WorkloadClass::Live,
            capacity: 100,
            used: 85,
            reserved: 50,
            available: 15,
            utilization_permille: 850,
        };
        assert!(snap.is_above_threshold(800));
        assert!(!snap.is_above_threshold(900));
    }

    #[test]
    fn invalid_config_reserved_exceeds_max() {
        let config = AdmissionBudgetConfig {
            class_configs: vec![ClassBudgetConfig::new(
                WorkloadClass::Live,
                10,
                20,
                1,
            )],
            degraded_indicator_threshold: 1,
            critical_indicator_threshold: 3,
            critical_stall_is_critical: true,
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn budget_thresholds_default_values() {
        let thresholds = BudgetThresholds::default();
        assert_eq!(thresholds.high_utilization_permille, 800);
        assert_eq!(thresholds.critical_utilization_permille, 950);
        assert_eq!(thresholds.global_throttle_permille, 900);
    }

    #[test]
    fn snapshot_class_lookup() {
        let snapshot = BudgetSnapshot {
            total_capacity: 100,
            total_used: 0,
            total_reserved: 50,
            class_snapshots: vec![ClassBudgetSnapshot {
                class: WorkloadClass::Recovery,
                capacity: 30,
                used: 0,
                reserved: 30,
                available: 30,
                utilization_permille: 0,
            }],
            degraded_mode: DegradedMode::Normal,
        };
        assert!(snapshot.class_snapshot(WorkloadClass::Recovery).is_some());
        assert!(snapshot.class_snapshot(WorkloadClass::Live).is_none());
    }

    #[test]
    fn standard_config_total_weight() {
        let config = AdmissionBudgetConfig::standard();
        assert_eq!(config.total_weight(), 26);
    }
}
