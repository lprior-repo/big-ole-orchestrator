//! WorkflowInstance actor — per-instance ractor actor with two-phase lifecycle (ADR-016).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use ractor::ActorProcessingErr;
use wtf_common::WorkflowEvent;

pub mod actor;
pub mod lifecycle;
pub mod procedural;
pub mod state;

pub use self::actor::WorkflowInstance;
pub use self::state::InstanceState;

/// Write a snapshot every 100 events (ADR-019).
pub const SNAPSHOT_INTERVAL: u32 = 100;

/// Shared helper to update event counters and trigger snapshots.
pub async fn handle_inject_event(
    state: &mut InstanceState,
    _seq: u64,
    _event: &WorkflowEvent,
) -> Result<(), ActorProcessingErr> {
    state.total_events_applied += 1;
    state.events_since_snapshot += 1;

    if state.events_since_snapshot >= SNAPSHOT_INTERVAL {
        tracing::debug!(
            instance_id = %state.args.instance_id,
            total = state.total_events_applied,
            "snapshot trigger (stub — see wtf-flbh)"
        );
        state.events_since_snapshot = 0;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};

    fn test_args(paradigm: WorkflowParadigm) -> InstanceArguments {
        InstanceArguments {
            namespace: wtf_common::NamespaceId::new("test"),
            instance_id: wtf_common::InstanceId::new("inst-01"),
            workflow_type: "order_flow".into(),
            paradigm,
            input: bytes::Bytes::from_static(b"{}"),
            engine_node_id: "node-1".into(),
            nats: None,
            procedural_workflow: None,
        }
    }

    #[test]
    fn snapshot_interval_is_100() {
        assert_eq!(SNAPSHOT_INTERVAL, 100);
    }

    #[tokio::test]
    async fn handle_inject_event_increments_counters() {
        let args = test_args(WorkflowParadigm::Fsm);
        let mut state = InstanceState {
            paradigm_state: state::initialize_paradigm_state(&args),
            args,
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: 0,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        };
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handle_inject_event(&mut state, 1, &event)
            .await
            .expect("ok");
        assert_eq!(state.total_events_applied, 1);
        assert_eq!(state.events_since_snapshot, 1);
    }

    #[tokio::test]
    async fn snapshot_resets_counter_at_interval() {
        let args = test_args(WorkflowParadigm::Fsm);
        let mut state = InstanceState {
            paradigm_state: state::initialize_paradigm_state(&args),
            args,
            phase: InstancePhase::Live,
            total_events_applied: 0,
            events_since_snapshot: SNAPSHOT_INTERVAL - 1,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            procedural_task: None,
        };
        let event = WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handle_inject_event(&mut state, 1, &event)
            .await
            .expect("ok");
        assert_eq!(state.events_since_snapshot, 0);
        assert_eq!(state.total_events_applied, 1);
    }
}
