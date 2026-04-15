//! Event sourcing engine tests (ESE-*).
//!
//! Tests for the unified EventSourcingEngine that combines replay,
//! projection, and snapshot-based recovery.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::replay::event_sourcing_engine::{
    EventSourcingConfig, EventSourcingEngine, InMemorySnapshotStore, RecoveryResult, RecoveryType,
    Snapshot, SnapshotStore,
};
use crate::replay::projection::{Projector, RebuildThrottleConfig};
use vo_types::events::metadata::EventMetadata;
use vo_types::events::{EventEnvelope, EventPayload};
use vo_types::state::LifecycleState;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TestProjectionState {
    pub value: String,
    pub event_count: u64,
}

impl TestProjectionState {
    fn new(value: String) -> Self {
        Self {
            value,
            event_count: 0,
        }
    }
}

struct TestProjector;

impl Projector<TestProjectionState, EventEnvelope> for TestProjector {
    type Error = crate::replay::projection::ProjectionError;

    fn project(
        &self,
        state: TestProjectionState,
        event: &EventEnvelope,
    ) -> Result<TestProjectionState, Self::Error> {
        let payload = EventPayload::try_from_json(&event.payload).map_err(|e| {
            crate::replay::projection::ProjectionError::BuildFailed(format!(
                "payload decode failed: {}",
                e
            ))
        })?;

        let new_state = match payload {
            EventPayload::WorkflowStarted { workflow_id, .. } => TestProjectionState {
                value: format!("{}+started:{}", state.value, workflow_id),
                event_count: state.event_count + 1,
            },
            EventPayload::StepScheduled { step_id, .. } => TestProjectionState {
                value: format!("{}+scheduled:{}", state.value, step_id),
                event_count: state.event_count + 1,
            },
            EventPayload::StepCompleted { step_id, .. } => TestProjectionState {
                value: format!("{}+completed:{}", state.value, step_id),
                event_count: state.event_count + 1,
            },
            _ => TestProjectionState {
                value: state.value,
                event_count: state.event_count + 1,
            },
        };

        Ok(new_state)
    }

    fn initial_state() -> TestProjectionState {
        TestProjectionState::new("init".to_string())
    }

    fn schema_version(&self) -> u8 {
        1
    }
}

#[cfg(test)]
mod ese_tests {
    use super::*;
    use crate::replay::test_helpers::*;

    #[test]
    fn ese_001_replay_engine_reconstructs_state_from_events() {
        let engine = crate::replay::ReplayEngine::new();
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = engine.replay(&events).expect("replay should succeed");
        assert_eq!(result.events_applied, 2);
        assert!(result.final_state.is_some());
    }

    #[test]
    fn ese_002_event_sourcing_engine_builds_projection_from_events() {
        let engine = EventSourcingEngine::builder(1).build();
        let projector = TestProjector;

        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = engine
            .build_projection(events, &projector)
            .expect("rebuild should succeed");

        assert_eq!(result.events_applied, 2);
        assert!(result.state.value.contains("wf-1"));
    }

    #[test]
    fn ese_003_snapshot_accelerates_recovery() {
        let engine = crate::replay::ReplayEngine::new();

        let events: Vec<EventEnvelope> = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
            make_event("inst-1", 3, step_started_payload("wf-1", "step-1")),
            make_event("inst-1", 4, step_completed_payload("wf-1", "step-1")),
        ];

        let result = engine.replay(&events).expect("replay should succeed");

        assert_eq!(result.events_applied, 4);
        assert!(matches!(
            result.final_state,
            Some(LifecycleState::Completed)
        ));
    }

    #[test]
    fn ese_004_recovery_detects_sequence_gap() {
        let engine = crate::replay::ReplayEngine::new();

        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 3, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = engine.replay(&events);
        assert!(result.is_err());
    }

    #[test]
    fn ese_005_projection_state_transitions() {
        use crate::replay::projection::{ProjectionState, ProjectionStateManager};

        let mgr = ProjectionStateManager::new();

        assert!(mgr.transition_to("p1", ProjectionState::Building).is_ok());
        assert!(mgr.transition_to("p1", ProjectionState::Ready).is_ok());
        assert!(mgr.is_ready("p1"));
    }
}

#[cfg(test)]
mod event_sourcing_engine_tests {
    use super::*;
    use crate::replay::test_helpers::*;

    #[test]
    fn ese_010_event_sourcing_engine_reconstructs_state() {
        let engine = EventSourcingEngine::builder(1).build();

        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = engine
            .reconstruct_state(&events)
            .expect("reconstruct should succeed");

        assert_eq!(result.events_applied, 2);
        assert_eq!(result.recovery_type, RecoveryType::FullReplay);
        assert!(result.state.is_some());
    }

    #[test]
    fn ese_011_event_sourcing_engine_with_empty_events() {
        let engine = EventSourcingEngine::builder(1).build();

        let events: Vec<EventEnvelope> = vec![];

        let result = engine
            .reconstruct_state(&events)
            .expect("reconstruct should succeed");

        assert_eq!(result.events_applied, 0);
        assert!(result.state.is_none());
    }

    #[test]
    fn ese_012_snapshot_creation_and_storage() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(1)
            .snapshot_store(store.clone())
            .config(EventSourcingConfig {
                snapshot_interval_events: 10,
                ..Default::default()
            })
            .build();

        let snapshot = engine
            .create_snapshot("test-proj", &"test_state".to_string(), 10)
            .expect("snapshot creation should succeed");

        assert_eq!(snapshot.projection_id, "test-proj");
        assert_eq!(snapshot.sequence, 10);
        assert_eq!(snapshot.schema_version, 1);

        let loaded = engine
            .load_snapshot("test-proj")
            .expect("load should succeed")
            .expect("snapshot should exist");
        assert_eq!(loaded.sequence, 10);
    }

    #[test]
    fn ese_013_reconstruct_with_snapshot_returns_snapshot_accelerated() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(1)
            .snapshot_store(store.clone())
            .build();

        let snapshot = engine
            .create_snapshot("test-proj", &LifecycleState::Pending, 50)
            .expect("snapshot creation should succeed");

        let recent_events = vec![
            make_event("test-proj", 51, workflow_started_payload("wf-1")),
            make_event("test-proj", 52, step_scheduled_payload("wf-1", "step-1")),
            make_event("test-proj", 53, step_started_payload("wf-1", "step-1")),
            make_event("test-proj", 54, step_completed_payload("wf-1", "step-1")),
        ];

        let result = engine
            .reconstruct_state_with_snapshot(Some(&snapshot), &recent_events)
            .expect("reconstruct with snapshot should succeed");

        assert_eq!(result.recovery_type, RecoveryType::SnapshotAccelerated);
        assert!(result.snapshot_used);
        assert_eq!(result.events_applied, 4);
    }

    #[test]
    fn ese_014_reconstruct_with_incompatible_snapshot_fails() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(2)
            .snapshot_store(store.clone())
            .build();

        let state_json = serde_json::to_vec(&"initial_state".to_string()).unwrap();
        let incompatible_snapshot =
            Snapshot::new("test-proj".to_string(), 1, state_json, 50, 1000, 12345);
        store
            .save_snapshot(&incompatible_snapshot)
            .expect("save should succeed");

        let result = engine.reconstruct_state_with_snapshot(Some(&incompatible_snapshot), &[]);

        assert!(result.is_err());
    }

    #[test]
    fn ese_015_build_projection_with_projector() {
        let engine = EventSourcingEngine::builder(1).build();
        let projector = TestProjector;

        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            make_event("inst-1", 2, step_scheduled_payload("wf-1", "step-1")),
        ];

        let result = engine
            .build_projection(events, &projector)
            .expect("build_projection should succeed");

        assert_eq!(result.events_applied, 2);
        assert!(result.state.value.contains("wf-1"));
        assert_eq!(result.schema_version, 1);
    }

    #[test]
    fn ese_016_should_create_snapshot_at_interval() {
        let engine = EventSourcingEngine::builder(1)
            .config(EventSourcingConfig {
                snapshot_interval_events: 100,
                ..Default::default()
            })
            .build();

        assert!(!engine.should_create_snapshot(0));
        assert!(!engine.should_create_snapshot(50));
        assert!(engine.should_create_snapshot(100));
        assert!(!engine.should_create_snapshot(150));
        assert!(engine.should_create_snapshot(200));
    }

    #[test]
    fn ese_017_delete_snapshot() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(1)
            .snapshot_store(store.clone())
            .build();

        let _snapshot = engine
            .create_snapshot("test-proj", &"state".to_string(), 50)
            .expect("snapshot creation should succeed");

        let loaded = engine
            .load_snapshot("test-proj")
            .expect("load should succeed")
            .expect("snapshot should exist");
        assert_eq!(loaded.sequence, 50);

        engine
            .delete_snapshot("test-proj", 50)
            .expect("delete should succeed");

        let loaded = engine
            .load_snapshot("test-proj")
            .expect("load should succeed");
        assert!(loaded.is_none());
    }

    #[test]
    fn ese_018_recovery_result_types() {
        let result: RecoveryResult = RecoveryResult::unit(10, 1, 10);
        assert_eq!(result.events_applied, 10);
        assert_eq!(result.starting_sequence, 1);
        assert_eq!(result.ending_sequence, 10);
        assert_eq!(result.recovery_type, RecoveryType::FullReplay);
        assert!(!result.snapshot_used);
    }

    #[test]
    fn ese_019_event_sourcing_engine_default() {
        let engine = EventSourcingEngine::default();
        assert_eq!(engine.max_schema_version(), 1);
    }

    #[test]
    fn ese_020_snapshot_checksum_is_computed() {
        let store = Arc::new(InMemorySnapshotStore::new());
        let engine = EventSourcingEngine::builder(1)
            .snapshot_store(store.clone())
            .build();

        let snapshot = engine
            .create_snapshot("test", &vec![1u8, 2, 3, 4], 1)
            .expect("snapshot should be created");

        assert_ne!(snapshot.checksum, 0);
    }
}
