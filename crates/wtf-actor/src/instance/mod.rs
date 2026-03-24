//! WorkflowInstance actor — per-instance ractor actor with two-phase lifecycle (ADR-016).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod actor;
pub mod handlers;
#[cfg(test)]
mod handlers_tests;
pub mod init;
#[cfg(test)]
mod init_tests;
pub mod lifecycle;
pub mod procedural;
pub mod procedural_utils;
pub mod state;

pub use self::actor::WorkflowInstance;
pub use self::handlers::SNAPSHOT_INTERVAL;
pub use self::state::InstanceState;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{InstanceArguments, InstancePhase, WorkflowParadigm};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wtf_common::storage::{EventStore, ReplayBatch, ReplayStream, ReplayedEvent};
    use wtf_common::{NamespaceId, WtfError};

    #[derive(Debug)]
    struct MockOkEventStore;

    #[async_trait]
    impl EventStore for MockOkEventStore {
        async fn publish(
            &self,
            _ns: &NamespaceId,
            _inst: &wtf_common::InstanceId,
            _event: wtf_common::WorkflowEvent,
        ) -> Result<u64, WtfError> {
            Ok(1)
        }
        async fn open_replay_stream(
            &self,
            _ns: &NamespaceId,
            _inst: &wtf_common::InstanceId,
            _from_seq: u64,
        ) -> Result<Box<dyn ReplayStream>, WtfError> {
            Ok(Box::new(EmptyReplayStream))
        }
    }

    #[derive(Debug)]
    struct EmptyReplayStream;

    #[async_trait]
    impl ReplayStream for EmptyReplayStream {
        async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
            Ok(ReplayBatch::TailReached)
        }
        async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError> {
            std::future::pending().await
        }
    }

    fn test_args(paradigm: WorkflowParadigm) -> InstanceArguments {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = wtf_storage::snapshots::open_snapshot_db(dir.path()).expect("db");
        InstanceArguments {
            namespace: wtf_common::NamespaceId::new("test"),
            instance_id: wtf_common::InstanceId::new("inst-01"),
            workflow_type: "order_flow".into(),
            paradigm,
            input: bytes::Bytes::from_static(b"{}"),
            engine_node_id: "node-1".into(),
            event_store: Some(Arc::new(MockOkEventStore)),
            state_store: None,
            task_queue: None,
            snapshot_db: Some(db),
            procedural_workflow: None,
            workflow_definition: None,
        }
    }

    #[test]
    fn snapshot_interval_is_100() {
        assert_eq!(handlers::SNAPSHOT_INTERVAL, 100);
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
            pending_signal_calls: HashMap::new(),
            procedural_task: None,
            live_subscription_task: None,
        };
        let event = wtf_common::WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handlers::inject_event(&mut state, 1, &event)
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
            events_since_snapshot: handlers::SNAPSHOT_INTERVAL - 1,
            pending_activity_calls: HashMap::new(),
            pending_timer_calls: HashMap::new(),
            pending_signal_calls: HashMap::new(),
            procedural_task: None,
            live_subscription_task: None,
        };
        let event = wtf_common::WorkflowEvent::SnapshotTaken {
            seq: 1,
            checksum: 0,
        };
        handlers::inject_event(&mut state, 1, &event)
            .await
            .expect("ok");
        assert_eq!(state.events_since_snapshot, 0);
        assert_eq!(state.total_events_applied, 1);
    }
}
