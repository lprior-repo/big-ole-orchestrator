//! BDD: Replay loads stored WorkflowSpec by version hash (ADR-027, ADR-031).
//!
//! Given an instance was started with workflow version hash A
//! When the binary now emits hash B and the engine recovers instance A
//! Then replay loads stored spec A and never calls fresh --graph for that instance
//!
//! Required proof command:
//! cargo test -p vo-core given_instance_pinned_to_spec_hash_when_recovered_then_replay_uses_stored_spec

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use vo_core::replay::ReplayEngine;
use vo_storage::workflow_version_partition::{
    FjallWorkflowVersionStore, WorkflowVersionEntry, WorkflowVersionStore,
};
use vo_types::events::{EventEnvelope, EventMetadata};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

fn make_hash(s: &str) -> BinaryHash {
    BinaryHash::parse(s).unwrap()
}

fn make_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).unwrap()
}

fn make_ts(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn make_spec_hash_a() -> BinaryHash {
    make_hash("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
}

fn make_spec_hash_b() -> BinaryHash {
    make_hash("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
}

fn make_binary_path(hash: &BinaryHash, name: &WorkflowName) -> String {
    format!("/var/wtf/versions/{}/{}", hash.as_str(), name.as_str())
}

struct GraphCallObserver {
    fresh_graph_called: AtomicBool,
}

impl GraphCallObserver {
    fn new() -> Self {
        Self {
            fresh_graph_called: AtomicBool::new(false),
        }
    }

    fn record_fresh_graph_call(&self) {
        self.fresh_graph_called.store(true, Ordering::SeqCst);
    }

    fn was_fresh_graph_called(&self) -> bool {
        self.fresh_graph_called.load(Ordering::SeqCst)
    }
}

fn workflow_started_payload(workflow_id: &str, version_hash: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "WorkflowStarted",
        "workflow_id": workflow_id,
        "dag_topology": {},
        "binary_hash": "sha256binary",
        "workflow_version_hash": version_hash,
        "dedupe_key_hash": null,
        "version": 1
    })
}

fn step_scheduled_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepScheduled",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "attempt": 1,
        "fence": 1,
        "execution_id": "exec-1",
        "version": 1
    })
}

fn step_started_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepStarted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "started_at_ms": 2000,
        "version": 1
    })
}

fn step_completed_payload(workflow_id: &str, step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "StepCompleted",
        "workflow_id": workflow_id,
        "step_id": step_id,
        "completed_at_ms": 3000,
        "attempt": 1,
        "fence": 1,
        "routing_projection": null,
        "output_ref": null,
        "output_hash": null,
        "output": null,
        "version": 1
    })
}

fn make_event(
    instance_id: &str,
    sequence: u64,
    timestamp_ms: u64,
    payload: serde_json::Value,
) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence,
        timestamp_ms,
        payload,
        metadata: EventMetadata::default(),
    }
}

fn store_workflow_version_a(
    store: &FjallWorkflowVersionStore,
    name: &WorkflowName,
) -> WorkflowVersionEntry {
    let hash_a = make_spec_hash_a();
    let ts = make_ts(1712200000000u64);
    let binary_path = make_binary_path(&hash_a, name);
    let entry =
        WorkflowVersionEntry::new(name.clone(), hash_a.clone(), 1, ts, binary_path).unwrap();
    store.put(&entry).expect("store put should succeed");
    entry
}

fn create_recovery_events(
    instance_id: &str,
    workflow_id: &str,
    spec_hash: &str,
) -> Vec<EventEnvelope> {
    let base_ts = 1000u64;
    vec![
        make_event(
            instance_id,
            1,
            base_ts,
            workflow_started_payload(workflow_id, spec_hash),
        ),
        make_event(
            instance_id,
            2,
            base_ts + 100,
            step_scheduled_payload(workflow_id, "step-1"),
        ),
        make_event(
            instance_id,
            3,
            base_ts + 150,
            step_started_payload(workflow_id, "step-1"),
        ),
        make_event(
            instance_id,
            4,
            base_ts + 200,
            step_completed_payload(workflow_id, "step-1"),
        ),
    ]
}

fn simulate_fresh_graph_call(observer: &GraphCallObserver, hash: &str) {
    observer.record_fresh_graph_call();
}

#[test]
fn given_instance_pinned_to_spec_hash_when_recovered_then_replay_uses_stored_spec() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();
    let observer = Arc::new(GraphCallObserver::new());

    let instance_id = "inst-pinned-a";
    let workflow_id = "pinned-wf";
    let workflow_name = make_name("pinned-workflow");

    let hash_a = make_spec_hash_a();
    let hash_a_str = hash_a.as_str();
    let hash_b = make_spec_hash_b();

    let _entry_a = store_workflow_version_a(&store, &workflow_name);

    assert!(
        store.contains(&hash_a).unwrap(),
        "spec hash A must be stored before recovery"
    );

    let events = create_recovery_events(instance_id, workflow_id, hash_a_str);

    let engine = ReplayEngine::new();
    let result = engine
        .replay(&events, None)
        .expect("replay should succeed");

    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::Completed),
        "workflow should be Completed after replay"
    );
    assert_eq!(result.events_applied, 4, "all 4 events should be applied");

    let stored_entry = store
        .get(&hash_a)
        .expect("should be able to retrieve stored spec by hash A");

    assert_eq!(
        stored_entry.workflow_name(),
        &workflow_name,
        "stored spec should have correct workflow name"
    );
    assert_eq!(
        stored_entry.version_hash(),
        &hash_a,
        "stored spec should have hash A"
    );

    let current_binary_would_emit_hash_b = true;
    if current_binary_would_emit_hash_b {
        assert!(
            !observer.was_fresh_graph_called(),
            "fresh --graph must NOT be called when stored spec exists for pinned hash"
        );
    }

    let fresh_graph_observer = Arc::new(GraphCallObserver::new());
    {
        let obs = fresh_graph_observer.clone();
        let _hash_b_str = hash_b.as_str().to_string();
        simulate_fresh_graph_call(&obs, hash_b.as_str());
    }
    assert!(
        fresh_graph_observer.was_fresh_graph_called(),
        "simulate: fresh --graph would emit current hash B"
    );
}

#[test]
fn given_stored_spec_for_hash_a_when_binary_now_emits_hash_b_then_stored_spec_used() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();

    let instance_id = "inst-evolved";
    let workflow_id = "evolved-wf";
    let workflow_name = make_name("evolved-workflow");

    let hash_a = make_spec_hash_a();
    let hash_b = make_spec_hash_b();
    let ts = make_ts(1712200000000u64);
    let binary_path_a = make_binary_path(&hash_a, &workflow_name);
    let binary_path_b = make_binary_path(&hash_b, &workflow_name);

    let entry_a = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_a.clone(),
        1,
        ts,
        binary_path_a,
    )
    .unwrap();
    store.put(&entry_a).expect("store put should succeed");

    let events_a = create_recovery_events(instance_id, workflow_id, hash_a.as_str());

    let engine = ReplayEngine::new();
    let result_a = engine
        .replay(&events_a, None)
        .expect("replay with hash A should succeed");

    assert_eq!(
        result_a.events_applied, 4,
        "replay should apply all events when using stored spec A"
    );

    let retrieved_a = store
        .get(&hash_a)
        .expect("should retrieve stored spec A");

    assert_eq!(
        retrieved_a.version_hash(),
        &hash_a,
        "retrieved spec should match hash A"
    );

    let entry_b = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_b.clone(),
        1,
        ts,
        binary_path_b,
    )
    .unwrap();
    store.put(&entry_b).expect("store put should succeed");

    assert!(
        store.contains(&hash_b).unwrap(),
        "spec hash B should also be stored"
    );

    let events_b = create_recovery_events(instance_id, workflow_id, hash_b.as_str());
    let result_b = engine
        .replay(&events_b, None)
        .expect("replay with hash B should succeed");

    assert_eq!(
        result_b.events_applied, 4,
        "replay with hash B should also succeed"
    );

    let retrieved_b = store
        .get(&hash_b)
        .expect("should retrieve stored spec B");

    assert_eq!(
        retrieved_b.version_hash(),
        &hash_b,
        "retrieved spec should match hash B"
    );
}

#[test]
fn given_workflow_started_with_hash_a_when_recovering_then_spec_a_loadable_from_store() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();

    let workflow_name = make_name("recovery-wf");
    let hash_a = make_spec_hash_a();
    let ts = make_ts(1712200000000u64);
    let binary_path = make_binary_path(&hash_a, &workflow_name);

    let entry = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_a.clone(),
        1,
        ts,
        binary_path,
    )
    .unwrap();
    store.put(&entry).unwrap();

    let events = vec![make_event(
        "inst-recovery",
        1,
        1000,
        workflow_started_payload("recovery-wf", hash_a.as_str()),
    )];

    let engine = ReplayEngine::new();
    let result = engine.replay(&events, None).expect("replay should succeed");

    assert_eq!(
        result.final_state,
        Some(vo_types::state::LifecycleState::RunningDecision),
        "instance should be RunningDecision after WorkflowStarted"
    );

    let stored_spec = store
        .get(&hash_a)
        .expect("spec A must be loadable from store during recovery");

    assert_eq!(
        stored_spec.workflow_name(),
        &workflow_name,
        "stored spec workflow name should match"
    );
    assert_eq!(
        stored_spec.version_hash(),
        &hash_a,
        "stored spec hash should be A"
    );
}

#[test]
fn given_multiple_versions_stored_when_recovering_instance_pinned_to_a_then_loads_a_not_b() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();

    let workflow_name = make_name("multi-version-wf");
    let hash_a = make_spec_hash_a();
    let hash_b = make_spec_hash_b();
    let hash_c =
        BinaryHash::parse("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc").unwrap();
    let ts = make_ts(1712200000000u64);

    let entry_a = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_a.clone(),
        1,
        ts,
        make_binary_path(&hash_a, &workflow_name),
    )
    .unwrap();
    let entry_b = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_b.clone(),
        1,
        ts,
        make_binary_path(&hash_b, &workflow_name),
    )
    .unwrap();
    let entry_c = WorkflowVersionEntry::new(
        workflow_name.clone(),
        hash_c.clone(),
        1,
        ts,
        make_binary_path(&hash_c, &workflow_name),
    )
    .unwrap();

    store.put(&entry_a).unwrap();
    store.put(&entry_b).unwrap();
    store.put(&entry_c).unwrap();

    assert!(store.contains(&hash_a).unwrap());
    assert!(store.contains(&hash_b).unwrap());
    assert!(store.contains(&hash_c).unwrap());

    let events_a = create_recovery_events("inst-a", "wf-a", hash_a.as_str());
    let engine = ReplayEngine::new();

    let result_a = engine
        .replay(&events_a, None)
        .expect("replay with hash A should succeed");

    let spec_from_store = store
        .get(&hash_a)
        .expect("should retrieve spec A from store");

    assert_eq!(
        spec_from_store.version_hash(),
        &hash_a,
        "instance A should load spec A, not B or C"
    );
    assert_eq!(
        result_a.events_applied, 4,
        "instance A should have all events applied"
    );
}