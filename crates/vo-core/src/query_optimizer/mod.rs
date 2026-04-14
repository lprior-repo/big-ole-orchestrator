//! Cost-based query optimizer for workflow engine query APIs (ADR-007, ADR-037).
//!
//! Transforms high-level query descriptors into optimized execution plans by:
//! 1. Parsing the query into an initial logical plan
//! 2. Applying logical optimization rules (predicate pushdown, projection pruning, scan fusion)
//! 3. Enumerating physical alternatives
//! 4. Selecting the lowest-cost plan via a cost model
//!
//! ## Architecture
//!
//! - [`QueryDescriptor`] — input: what the caller wants (filter, sort, limit, source)
//! - [`LogicalPlan`] / [`PlanNode`] — logical IR after rule application
//! - [`PhysicalPlan`] / [`PhysicalNode`] — executable IR with access strategy
//! - [`CostModel`] — estimates row count, I/O, CPU for each physical node
//! - [`QueryPlanner`] — orchestrates the full pipeline
//! - [`TableStatistics`] — cached cardinality/selectivity metadata

mod cost;
mod error;
mod logical;
mod optimizer;
mod physical;
mod planner;
mod statistics;

pub use cost::{Cost, CostModel};
pub use error::{OptimizationError, OptimizationResult};
pub use logical::{LogicalPlan, PlanNode, Predicate, SortDirection, SortKey};
pub use optimizer::Optimizer;
pub use physical::{AccessStrategy, PhysicalNode, PhysicalPlan};
pub use planner::{QueryDescriptor, QueryPlanner};
pub use statistics::{ColumnStats, TableStatistics};

#[cfg(test)]
mod tests;
