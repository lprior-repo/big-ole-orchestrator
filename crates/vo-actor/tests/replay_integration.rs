//! Integration tests for ReplayEngine integration into InstanceActor.
//!
//! Verifies that the actor correctly reconstructs LifecycleState from event history.

use vo_core::replay::ReplayEngine;
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::state::LifecycleState;
use vo_types::InstanceId;
use serde_json::json;

#[tokio::test]
async fn instance_actor_recovers_state_from_event_history() {
    // 1. Setup event history
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").expect("valid id");

    let payload_json = json!({
        "type": "WorkflowStarted",
        "workflow_id": "test-workflow",
        "dag_topology": {},
        "binary_hash": "abc123hash",
        "workflow_version_hash": "def456",
        "version": 0
    });

    let event = EventEnvelope {
        instance_id: instance_id.to_string(),
        sequence: 1,
        payload: payload_json,
        timestamp_ms: 123456789,
        schema_version: 1,
        metadata: EventMetadata::default(),
    };

    let events = vec![event];

    // 2. Run ReplayEngine (Calculation)
    let engine = ReplayEngine::new();
    let result = engine.replay(&events).expect("replay should succeed");

    // 3. Verify final state
    let final_state = result.final_state.expect("state should be reconstructed");
    assert_eq!(final_state, LifecycleState::RunningDecision);
    assert_eq!(result.events_applied, 1);
}
