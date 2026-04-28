//! Re-export canonical WorkloadClass from vo_types.
//!
//! The unified `WorkloadClass` contains all variants across ADR-033 (dispatch
//! priority), ADR-013 (budget admission), and actor fairness scheduling.

pub use vo_types::workload_class::WorkloadClass;

/// The 4 dispatch-priority classes used by ADR-033 budget tracking, in rank order.
///
/// This is the subset of `WorkloadClass` that the `WorkloadBudget` arrays are
/// sized for. Code that indexes into `[u32; 4]` budget arrays must use
/// `adr033_class_index()` rather than `WorkloadClass::rank()` directly.
pub const ADR033_CLASSES: [WorkloadClass; 4] = [
    WorkloadClass::ExactCritical,
    WorkloadClass::Standard,
    WorkloadClass::Recovery,
    WorkloadClass::UnsafeBulk,
];

/// Returns the ADR-033 budget array index for the given class.
///
/// # Panics
/// Panics if the class is not one of the 4 ADR-033 classes.
#[must_use]
pub fn adr033_class_index(class: WorkloadClass) -> usize {
    ADR033_CLASSES
        .iter()
        .position(|&c| c == class)
        .unwrap_or_else(|| panic!("WorkloadClass::{class:?} is not an ADR-033 budget class"))
}
