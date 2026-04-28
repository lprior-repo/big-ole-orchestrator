//! Lineage projection system (ADR-038).
//!
//! Split into bounded contexts:
//! - [`lineage_graph`] - graph construction + traversal
//! - [`projection_state`] - state machine + transitions
//! - [`event_replay_calc`] - replay math + sequence gaps
//! - [`queries`] - query building + filtering

pub mod lineage_graph;
pub mod projection_state;
pub mod event_replay_calc;
pub mod queries;
pub mod types;

pub use lineage_graph::{route_event, is_historical_epoch};
pub use projection_state::{is_valid_state_transition, atomic_projection_swap};
pub use event_replay_calc::{
    continue_as_new_7step,
    compute_carried_state,
    validate_carried_state,
    determine_rebuild_scope,
    determine_projection_class,
    build_projection,
    build_projection_incremental,
    evaluate_rollover_trigger,
    compute_epoch_compensation_order,
    RolloverError,
    RebuildError,
    CompensationError,
};

