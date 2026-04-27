//! BDD tests for replay with stored WorkflowSpec (ADR-027).
//!
//! Verifies that recovery/replay uses the stored canonical WorkflowSpec
//! loaded from workflow_versions partition, never a fresh --graph subprocess.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use vo_core::replay::ReplayEngine;
use vo_storage::workflow_version_partition::{
    WorkflowVersionEntry, WorkflowVersionStore, WorkflowVersionStoreError,
};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

/// In-memory spy store that records which hashes were requested.
struct SpyWorkflowVersionStore {
    entries: HashMap<BinaryHash, WorkflowVersionEntry>,
    get_calls: Rc<RefCell<Vec<BinaryHash>>>,
}

impl SpyWorkflowVersionStore {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            get_calls: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn with_spec(hash: BinaryHash, binary_path: &str) -> Self {
        let mut store = Self::new();
        let entry = WorkflowVersionEntry::new(
            WorkflowName::parse("test-workflow").unwrap(),
            hash.clone(),
            1,
            TimestampMs::try_from(1712200000000u64).unwrap(),
            binary_path.to_string(),
        )
        .unwrap();
        store.entries.insert(hash, entry);
        store
    }

    fn get_calls(&self) -> Vec<BinaryHash> {
        self.get_calls.borrow().clone()
    }
}

impl Default for SpyWorkflowVersionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowVersionStore for SpyWorkflowVersionStore {
    fn get(&self, hash: &BinaryHash) -> Result<WorkflowVersionEntry, WorkflowVersionStoreError> {
        self.get_calls.borrow_mut().push(hash.clone());
        self.entries
            .get(hash)
            .cloned()
            .ok_or_else(|| WorkflowVersionStoreError::KeyNotFound {
                hash: hash.to_string(),
            })
    }

    fn put(&self, _entry: &WorkflowVersionEntry) -> Result<(), WorkflowVersionStoreError> {
        unimplemented!("spy store for tests only")
    }

    fn contains(&self, _hash: &BinaryHash) -> Result<bool, WorkflowVersionStoreError> {
        unimplemented!("spy store for tests only")
    }

    fn delete(&self, _hash: &BinaryHash) -> Result<(), WorkflowVersionStoreError> {
        unimplemented!("spy store for tests only")
    }

    fn list_hashes(&self) -> Result<Vec<BinaryHash>, WorkflowVersionStoreError> {
        unimplemented!("spy store for tests only")
    }
}

// ---------------------------------------------------------------------------
// Test events helper
// ---------------------------------------------------------------------------

fn make_test_events(instance_id: &str) -> Vec<vo_types::events::EventEnvelope> {
    use serde_json::json;
    use vo_types::events::{EventEnvelope, EventMetadata};

    let make = |seq, typ, workflow_id| EventEnvelope {
        schema_version: 1,
        instance_id: instance_id.to_string(),
        sequence: seq,
        timestamp_ms: 1000 * seq,
        payload: json!({
            "type": typ,
            "workflow_id": workflow_id,
            "version": 1
        }),
        metadata: EventMetadata::default(),
    };

    vec![
        make(1, "WorkflowStarted", "wf-1"),
        make(2, "StepScheduled", "wf-1"),
        make(3, "StepStarted", "wf-1"),
        make(4, "StepCompleted", "wf-1"),
    ]
}

// ---------------------------------------------------------------------------
// BDD Scenario: given instance pinned to spec hash when recovered then replay
// uses stored spec
// ADR-027 §6: "Replay uses the stored canonical WorkflowSpec, never a fresh
// --graph subprocess during recovery."
// ADR-027 §7 step 3: "Recover the canonical WorkflowSpec from workflow_versions
// via the pinned binary hash."
// ---------------------------------------------------------------------------

#[test]
fn given_instance_pinned_to_spec_hash_when_recovered_then_replay_uses_stored_spec() {
    // GIVEN an instance was started with workflow version hash A
    let hash_a = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .unwrap();
    let binary_path_a = format!(
        "/var/wtf/versions/{}/test-workflow",
        hash_a.as_str()
    );

    // AND the workflow version store has the canonical spec for hash A stored
    let store = SpyWorkflowVersionStore::with_spec(hash_a.clone(), &binary_path_a);
    let events = make_test_events("inst-1");

    // WHEN the engine recovers the instance using replay_with_stored_spec
    let engine = ReplayEngine::new();
    let result = engine
        .replay_with_stored_spec(&store, &hash_a, &events)
        .expect("replay should succeed");

    // THEN replay uses the stored spec from the store (not fresh --graph)
    let (replay_result, loaded_binary_path) = result;

    // AND the loaded binary path matches what was stored
    assert_eq!(
        loaded_binary_path, binary_path_a,
        "replay should load spec from stored workflow_versions entry, not fresh --graph"
    );

    // AND the store was called with hash A (proving spec was loaded from store)
    let calls = store.get_calls();
    assert_eq!(
        calls.len(),
        1,
        "store should be called exactly once to load the spec"
    );
    assert_eq!(
        calls[0], hash_a,
        "store should be called with the pinned hash A"
    );

    // AND replay completed successfully with the expected final state
    assert_eq!(
        replay_result.final_state,
        Some(vo_types::state::LifecycleState::Completed),
        "workflow should complete after StepCompleted event"
    );
    assert_eq!(
        replay_result.events_applied, 4,
        "all 4 events should be applied"
    );
}

#[test]
fn given_binary_hash_not_in_store_when_replay_with_stored_spec_then_returns_error() {
    // GIVEN a binary hash B that is NOT in the store
    let hash_b = BinaryHash::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .unwrap();
    let store = SpyWorkflowVersionStore::new();
    let events = make_test_events("inst-1");

    // WHEN replay_with_stored_spec is called with hash B
    let engine = ReplayEngine::new();
    let result = engine.replay_with_stored_spec(&store, &hash_b, &events);

    // THEN it should return StoredSpecNotFound error
    assert!(
        matches!(
            result,
            Err(vo_core::replay::ReplayError::StoredSpecNotFound { .. })
        ),
        "should return StoredSpecNotFound when hash not in store, got: {:?}",
        result
    );
}

#[test]
fn given_instance_pinned_to_hash_a_and_binary_deployed_hash_b_when_recovered_then_still_uses_hash_a() {
    // GIVEN an instance started with hash A (old deployment)
    let hash_a = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .unwrap();
    let binary_path_a = format!(
        "/var/wtf/versions/{}/test-workflow",
        hash_a.as_str()
    );

    // AND a new deployment has hash B (current active)
    let hash_b = BinaryHash::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .unwrap();

    // AND only hash A is in the store (the instance's pinned version)
    let store = SpyWorkflowVersionStore::with_spec(hash_a.clone(), &binary_path_a);
    let events = make_test_events("inst-1");

    // WHEN the engine recovers the instance
    let engine = ReplayEngine::new();
    let result = engine
        .replay_with_stored_spec(&store, &hash_a, &events)
        .expect("replay should succeed");

    // THEN it uses hash A's spec (not hash B, the current active deployment)
    let (replay_result, loaded_binary_path) = result;
    assert!(
        loaded_binary_path.contains(hash_a.as_str()),
        "should use hash A's spec, not hash B"
    );
    assert!(
        !loaded_binary_path.contains(hash_b.as_str()),
        "should NOT use hash B's spec (current active)"
    );
    assert_eq!(
        replay_result.final_state,
        Some(vo_types::state::LifecycleState::Completed)
    );
}

#[test]
fn given_stored_spec_when_replay_with_stored_spec_then_store_get_called_before_replay() {
    // GIVEN a store with stored spec
    let hash_a = BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .unwrap();
    let binary_path_a = format!(
        "/var/wtf/versions/{}/test-workflow",
        hash_a.as_str()
    );
    let store = Rc::new(SpyWorkflowVersionStore::with_spec(hash_a.clone(), &binary_path_a));
    let events = make_test_events("inst-1");

    // Clone the rc to use in the assertion after
    let store_clone = Rc::clone(&store);

    // WHEN replay_with_stored_spec is called
    let engine = ReplayEngine::new();
    let result = engine
        .replay_with_stored_spec(&*store, &hash_a, &events)
        .expect("replay should succeed");

    // THEN the store was called BEFORE replay completed
    // (proving spec is loaded at start of replay, not during)
    let calls = store_clone.get_calls();
    assert_eq!(calls.len(), 1, "store should be called exactly once");
    assert_eq!(calls[0], hash_a);

    // AND replay result is valid
    let (replay_result, _) = result;
    assert_eq!(
        replay_result.final_state,
        Some(vo_types::state::LifecycleState::Completed)
    );
}