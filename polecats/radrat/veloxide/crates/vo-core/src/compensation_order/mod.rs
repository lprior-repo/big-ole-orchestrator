//! Reverse dependency ordering for saga compensation (ADR-034).
//!
//! Computes the order in which compensations must execute during rollback,
//! ensuring dependees are compensated before their dependents.

mod topology;
#[cfg(test)]
mod topology_tests;

pub use topology::CompensationPolicy;
pub use topology::{
    compute_compensation_order, detect_cycle, filter_compensatable, validate_dependencies,
    CompensationNode, CompensationOrderResult, OrderingError,
};
