//! DagActor — DAG paradigm actor state and event application (ADR-017).

pub mod apply;
pub mod state;
#[cfg(test)]
mod tests;

pub use apply::*;
pub use state::*;

/// Compute the set of nodes that are ready to dispatch.
#[must_use]
pub fn ready_nodes(state: &DagActorState) -> Vec<NodeId> {
    let mut ready: Vec<NodeId> = state
        .nodes
        .iter()
        .filter(|(id, node)| {
            !state.completed.contains_key(*id)
                && !state.in_flight.contains(*id)
                && !state.failed.contains(*id)
                && node
                    .predecessors
                    .iter()
                    .all(|pred| state.completed.contains_key(pred))
        })
        .map(|(id, _)| id.clone())
        .collect();

    ready.sort_by(|a, b| a.0.cmp(&b.0));
    ready
}

/// Check whether the DAG has reached a terminal state.
#[must_use]
pub fn is_terminal(state: &DagActorState) -> bool {
    is_succeeded(state) || is_failed(state)
}

/// Returns `true` if all nodes completed successfully.
#[must_use]
pub fn is_succeeded(state: &DagActorState) -> bool {
    state
        .nodes
        .keys()
        .all(|id| state.completed.contains_key(id))
}

/// Returns `true` if any node has permanently failed (blocking the DAG).
#[must_use]
pub fn is_failed(state: &DagActorState) -> bool {
    !state.failed.is_empty()
}
