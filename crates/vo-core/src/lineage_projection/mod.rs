//! Lineage projection system (ADR-038).
//!
//! Split into bounded contexts:
//! - [`lineage_graph`] - graph construction + traversal
//! - [`projection_state`] - state machine + transitions
//! - [`event_replay_calc`] - replay math + sequence gaps
//! - [`queries`] - query building + filtering

pub mod event_replay_calc;
pub mod lineage_graph;
pub mod projection_state;
pub mod queries;
pub mod types;

pub use event_replay_calc::{
    build_projection, build_projection_incremental, compute_carried_state,
    compute_epoch_compensation_order, continue_as_new_7step, determine_projection_class,
    determine_rebuild_scope, evaluate_rollover_trigger, validate_carried_state, CompensationError,
    RebuildError, RolloverError,
};
pub use lineage_graph::{is_historical_epoch, route_event};
pub use projection_state::{atomic_projection_swap, is_valid_state_transition};
