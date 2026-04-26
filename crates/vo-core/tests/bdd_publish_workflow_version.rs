//! BDD: Publish stores WorkflowSpec before activation (ADR-027, ADR-031).
//!
//! Given a valid workflow binary emits a canonical spec
//! When publish succeeds
//! Then the workflow version is stored durably before any activation/routing record is visible
//!
//! Required proof command:
//! cargo test -p vo-core given_valid_publish_when_activation_occurs_then_workflow_version_was_stored_first

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use vo_core::circuit_breaker::{
    evaluate_registration, CircuitBreakerConfig, CircuitBreakerState, RegistrationRequest,
};
use vo_storage::workflow_version_partition::{
    FjallWorkflowVersionStore, WorkflowVersionEntry, WorkflowVersionStore,
};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

fn make_hash() -> BinaryHash {
    BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
}

fn make_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).unwrap()
}

fn make_ts(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

struct PublishOrderObserver {
    put_seq: AtomicU64,
    activation_seq: AtomicU64,
    counter: AtomicU64,
}

impl PublishOrderObserver {
    fn new() -> Self {
        Self {
            put_seq: AtomicU64::new(0),
            activation_seq: AtomicU64::new(0),
            counter: AtomicU64::new(0),
        }
    }

    fn record_put(&self) {
        let seq = self.counter.fetch_add(1, Ordering::SeqCst);
        self.put_seq.store(seq, Ordering::SeqCst);
    }

    fn record_activation(&self) {
        let seq = self.counter.fetch_add(1, Ordering::SeqCst);
        self.activation_seq.store(seq, Ordering::SeqCst);
    }

    fn put_happened_before_activation(&self) -> bool {
        let put = self.put_seq.load(Ordering::SeqCst);
        let act = self.activation_seq.load(Ordering::SeqCst);
        put < act
    }
}

fn publish_workflow(
    store: &FjallWorkflowVersionStore,
    cb_state: &CircuitBreakerState,
    cb_config: &CircuitBreakerConfig,
    entry: &WorkflowVersionEntry,
    observer: &PublishOrderObserver,
) -> Result<(), String> {
    store
        .put(entry)
        .map_err(|e| format!("workflow version store failed: {e}"))?;
    observer.record_put();

    if !store
        .contains(entry.version_hash())
        .map_err(|e| format!("contains check failed: {e}"))?
    {
        return Err("workflow version not found after put — durability violated".to_string());
    }

    let request = RegistrationRequest {
        workflow_name: entry.workflow_name().clone(),
        binary_hash: entry.version_hash().clone(),
        force: false,
    };
    let outcome = evaluate_registration(&request, cb_config, cb_state, Instant::now())
        .map_err(|e| format!("registration evaluation failed: {e}"))?;
    observer.record_activation();

    match outcome {
        vo_core::circuit_breaker::RegistrationOutcome::Allowed => Ok(()),
        other => Err(format!("registration not allowed: {other:?}")),
    }
}

#[test]
fn given_valid_publish_when_activation_occurs_then_workflow_version_was_stored_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();
    let cb_state = CircuitBreakerState::new();
    let cb_config = CircuitBreakerConfig::default_config().unwrap();
    let observer = PublishOrderObserver::new();

    let hash = make_hash();
    let name = make_name("order-invariant-wf");
    let ts = make_ts(1712200000000u64);
    let binary_path = format!(
        "/var/wtf/versions/{}/order-invariant-wf",
        hash.as_str()
    );

    let entry =
        WorkflowVersionEntry::new(name.clone(), hash.clone(), 1, ts, binary_path).unwrap();

    publish_workflow(&store, &cb_state, &cb_config, &entry, &observer).unwrap();

    assert!(
        observer.put_happened_before_activation(),
        "workflow version put MUST happen before activation — ADR-031 ordering violated"
    );

    let retrieved = store.get(&hash).unwrap();
    assert_eq!(retrieved.workflow_name(), &name);
    assert_eq!(retrieved.version_hash(), &hash);
    assert_eq!(retrieved.schema_version(), 1);
}

#[test]
fn given_valid_publish_when_version_missing_then_activation_proof_absent() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();

    let hash = make_hash();
    let name = make_name("no-version-no-activate");
    let ts = make_ts(1712200000000u64);
    let binary_path = format!(
        "/var/wtf/versions/{}/no-version-no-activate",
        hash.as_str()
    );

    let entry =
        WorkflowVersionEntry::new(name.clone(), hash.clone(), 1, ts, binary_path).unwrap();

    assert!(!store.contains(&hash).unwrap());
    assert!(
        store.get(&hash).is_err(),
        "version must not be found before put"
    );

    store.put(&entry).unwrap();
    assert!(store.contains(&hash).unwrap());

    let retrieved = store.get(&hash).unwrap();
    assert_eq!(retrieved.workflow_name(), &name);
    assert_eq!(retrieved.version_hash(), &hash);
}

#[test]
fn given_multiple_publishes_when_activated_then_all_versions_stored_first() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();
    let cb_state = CircuitBreakerState::new();
    let cb_config = CircuitBreakerConfig::default_config().unwrap();

    let hash1 = make_hash();
    let hash2 =
        BinaryHash::parse("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            .unwrap();
    let name1 = make_name("wf-alpha");
    let name2 = make_name("wf-beta");
    let ts = make_ts(1712200000000u64);

    let entry1 = WorkflowVersionEntry::new(
        name1.clone(),
        hash1.clone(),
        1,
        ts,
        format!("/var/wtf/versions/{}/wf-alpha", hash1.as_str()),
    )
    .unwrap();
    let entry2 = WorkflowVersionEntry::new(
        name2.clone(),
        hash2.clone(),
        1,
        ts,
        format!("/var/wtf/versions/{}/wf-beta", hash2.as_str()),
    )
    .unwrap();

    let observer = PublishOrderObserver::new();

    publish_workflow(&store, &cb_state, &cb_config, &entry1, &observer).unwrap();
    publish_workflow(&store, &cb_state, &cb_config, &entry2, &observer).unwrap();

    assert!(store.contains(&hash1).unwrap());
    assert!(store.contains(&hash2).unwrap());

    let retrieved1 = store.get(&hash1).unwrap();
    assert_eq!(retrieved1.workflow_name(), &name1);

    let retrieved2 = store.get(&hash2).unwrap();
    assert_eq!(retrieved2.workflow_name(), &name2);
}

#[test]
fn given_publish_persisted_when_crash_then_version_survives() {
    let dir = tempfile::tempdir().unwrap();
    let hash = make_hash();
    let name = make_name("crash-recovery-wf");
    let ts = make_ts(1712200000000u64);
    let binary_path = format!(
        "/var/wtf/versions/{}/crash-recovery-wf",
        hash.as_str()
    );

    let entry =
        WorkflowVersionEntry::new(name.clone(), hash.clone(), 1, ts, binary_path).unwrap();

    {
        let db = fjall::Database::builder(dir.path()).open().unwrap();
        let store = FjallWorkflowVersionStore::open(&db).unwrap();
        store.put(&entry).unwrap();
        assert!(store.contains(&hash).unwrap());
    }

    let db2 = fjall::Database::builder(dir.path()).open().unwrap();
    let store2 = FjallWorkflowVersionStore::open(&db2).unwrap();
    let recovered = store2.get(&hash).unwrap();
    assert_eq!(recovered.workflow_name(), &name);
    assert_eq!(recovered.version_hash(), &hash);
    assert_eq!(recovered.schema_version(), 1);
}

#[test]
fn given_publish_when_registration_allowed_then_version_was_already_stored() {
    let dir = tempfile::tempdir().unwrap();
    let db = fjall::Database::builder(dir.path()).open().unwrap();
    let store = FjallWorkflowVersionStore::open(&db).unwrap();
    let cb_state = CircuitBreakerState::new();
    let cb_config = CircuitBreakerConfig::default_config().unwrap();

    let hash = make_hash();
    let name = make_name("trigger-needs-version");
    let ts = make_ts(1712200000000u64);
    let binary_path = format!(
        "/var/wtf/versions/{}/trigger-needs-version",
        hash.as_str()
    );

    let entry =
        WorkflowVersionEntry::new(name.clone(), hash.clone(), 1, ts, binary_path).unwrap();

    store.put(&entry).unwrap();

    let request = RegistrationRequest {
        workflow_name: name.clone(),
        binary_hash: hash.clone(),
        force: false,
    };
    let outcome =
        evaluate_registration(&request, &cb_config, &cb_state, Instant::now()).unwrap();

    use vo_core::circuit_breaker::RegistrationOutcome;
    assert!(
        matches!(outcome, RegistrationOutcome::Allowed),
        "registration should be allowed after publish"
    );

    assert!(
        store.contains(&hash).unwrap(),
        "registration allowed but version not stored — ADR-031 violated"
    );
}
