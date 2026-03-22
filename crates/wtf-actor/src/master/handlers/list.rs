use crate::messages::InstanceStatusSnapshot;
use crate::master::state::OrchestratorState;
use crate::master::handlers::status::handle_get_status;

pub async fn handle_list_active(
    state: &OrchestratorState,
) -> Vec<InstanceStatusSnapshot> {
    let mut snapshots = Vec::with_capacity(state.active.len());
    for id in state.active.keys() {
        if let Some(snapshot) = handle_get_status(state, id).await {
            snapshots.push(snapshot);
        }
    }
    snapshots
}

#[cfg(test)]
mod tests {
    use super::handle_list_active;
    use crate::master::state::{OrchestratorConfig, OrchestratorState};

    #[tokio::test]
    async fn list_active_returns_empty_when_no_instances() {
        let state = OrchestratorState::new(OrchestratorConfig::default());
        let snapshots = handle_list_active(&state).await;
        assert!(snapshots.is_empty());
    }
}
