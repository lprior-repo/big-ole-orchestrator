//! QA worker loop lifecycle, distributed lock acquisition, and connector runtime tests.

use std::sync::atomic::{AtomicU32, Ordering};
use vo_worker::supervisor::{SpawnPhase, SpawnRecord, is_zombie_state, should_respawn, calculate_backoff_delay};
use vo_worker::{Connector, LockId, LockManager, LockManagerRetryWrapper, LockMode, LockPromote, LockPromoteResponse, LockQuery, LockQueryResponse, LockRelease, LockRequest, LockResponse, OwnerId, RetryConfig};

struct FailingLockMock { fail_count: u32, attempts: AtomicU32 }
impl FailingLockMock { fn new(fail_count: u32) -> Self { Self { fail_count, attempts: AtomicU32::new(0) } } }
#[async_trait::async_trait]
impl LockManager for FailingLockMock {
    async fn acquire(&self, req: LockRequest) -> LockResponse {
        let n = self.attempts.fetch_add(1, Ordering::SeqCst);
        if n < self.fail_count { LockResponse { request_id: req.request_id, lock_id: req.lock_id, owner: req.owner, granted: false, hold_token: None, expires_at: None, error: Some("contended".into()) } }
        else { LockResponse { request_id: req.request_id, lock_id: req.lock_id, owner: req.owner, granted: true, hold_token: Some("tok".into()), expires_at: None, error: None } }
    }
    async fn release(&self, _: LockRelease) -> Result<(), vo_worker::LockError> { Ok(()) }
    async fn query(&self, _: LockQuery) -> LockQueryResponse { LockQueryResponse { locks: vec![] } }
    async fn promote(&self, _: LockPromote) -> LockPromoteResponse { LockPromoteResponse { request_id: "".into(), lock_id: LockId::new(""), granted: false, new_mode: None, error: None } }
    async fn demote(&self, _: LockId, _: OwnerId, _: String) -> Result<LockMode, vo_worker::LockError> { Ok(LockMode::Shared) }
    async fn extend_ttl(&self, _: LockId, _: OwnerId, _: String, _: u64) -> Result<chrono::DateTime<chrono::Utc>, vo_worker::LockError> { Ok(chrono::Utc::now()) }
    async fn is_locked(&self, _: &LockId) -> bool { false }
    async fn get_holder(&self, _: &LockId) -> Option<(OwnerId, LockMode)> { None }
}

#[test]
fn spawn_lifecycle_spawn_to_running() {
    let instance_id = vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes());
    let r = SpawnRecord {
        spawn_id: None,
        instance_id,
        command: "vo-binary --execute-node start".into(),
        spawn_phase: SpawnPhase::Spawn,
        health_checks: 0,
        spawn_attempts: 1,
        last_error: None,
    };
    let r = r.transition_to_health_check();
    assert_eq!(r.spawn_phase, SpawnPhase::HealthCheck);
    let r = r.transition_to_running();
    assert_eq!(r.spawn_phase, SpawnPhase::Running);
    let r = r.transition_to_shutdown();
    assert_eq!(r.spawn_phase, SpawnPhase::Shutdown);
}

#[test]
fn respawn_resets_phase_and_increments_attempts() {
    let instance_id = vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes());
    let failed = vo_worker::supervisor::SpawnRecord {
        spawn_id: None,
        instance_id,
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 2,
        spawn_attempts: 3,
        last_error: None,
    };
    let respawned = failed.respawn(None);
    assert_eq!(respawned.spawn_phase, SpawnPhase::Spawn);
    assert_eq!(respawned.spawn_attempts, 4);
    assert_eq!(respawned.health_checks, 0);
}

#[test]
fn zombie_detection_rejects_high_attempt_failures() {
    let zombie = SpawnRecord {
        spawn_id: None,
        instance_id: vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes()),
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 5,
        last_error: None,
    };
    assert!(is_zombie_state(&zombie));
    let recoverable = SpawnRecord {
        spawn_id: None,
        instance_id: vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes()),
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 2,
        last_error: None,
    };
    assert!(!is_zombie_state(&recoverable));
}

#[test]
fn should_respawn_respects_max_attempts() {
    let within = SpawnRecord {
        spawn_id: None,
        instance_id: vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes()),
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 2,
        last_error: None,
    };
    assert!(should_respawn(&within, 5));
    let at_limit = SpawnRecord {
        spawn_id: None,
        instance_id: vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes()),
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Failed,
        health_checks: 0,
        spawn_attempts: 5,
        last_error: None,
    };
    assert!(!should_respawn(&at_limit, 5));
    let running = SpawnRecord {
        spawn_id: None,
        instance_id: vo_types::InstanceId::from_bytes(ulid::Ulid::new().to_bytes()),
        command: "vo-binary".into(),
        spawn_phase: SpawnPhase::Running,
        health_checks: 0,
        spawn_attempts: 1,
        last_error: None,
    };
    assert!(!should_respawn(&running, 5));
}

#[test]
fn backoff_grows_exponentially() {
    assert_eq!(calculate_backoff_delay(100, 2.0, 1), 100);
    assert_eq!(calculate_backoff_delay(100, 2.0, 2), 200);
    assert_eq!(calculate_backoff_delay(100, 2.0, 3), 400);
}

#[tokio::test]
async fn lock_acquire_first_try() {
    let mock = FailingLockMock::new(0);
    let wrapper = LockManagerRetryWrapper::new(&mock, RetryConfig::new(10, 2.0, 3));
    let resp = wrapper.acquire(LockRequest {
        lock_id: LockId::new("wq-1"), owner: OwnerId::new("w1".into()),
        mode: LockMode::Exclusive, ttl_ms: 5000, request_id: "r1".into(),
    }).await;
    assert!(resp.granted);
}

#[tokio::test]
async fn lock_acquire_retries_then_succeeds() {
    let mock = FailingLockMock::new(2);
    let wrapper = LockManagerRetryWrapper::new(&mock, RetryConfig::new(1, 2.0, 5));
    let resp = wrapper.acquire(LockRequest {
        lock_id: LockId::new("wq-1"), owner: OwnerId::new("w2".into()),
        mode: LockMode::Exclusive, ttl_ms: 5000, request_id: "r1".into(),
    }).await;
    assert!(resp.granted);
    assert_eq!(mock.attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn lock_acquire_exhausts_retries() {
    let mock = FailingLockMock::new(10);
    let wrapper = LockManagerRetryWrapper::new(&mock, RetryConfig::new(1, 2.0, 2));
    let resp = wrapper.acquire(LockRequest {
        lock_id: LockId::new("wq-1"), owner: OwnerId::new("w3".into()),
        mode: LockMode::Exclusive, ttl_ms: 5000, request_id: "r1".into(),
    }).await;
    assert!(!resp.granted);
    assert!(resp.error.unwrap().contains("max retry"));
}

#[test]
fn lock_entry_ttl_tracking() {
    let entry = vo_worker::LockEntry::new(
        LockId::new("job-1"), OwnerId::new("w1".into()), LockMode::Exclusive, 5000,
    );
    assert!(!entry.is_expired());
    assert!(entry.remaining_ttl().is_some());
}

#[tokio::test]
async fn connector_registry_dispatches_by_type() {
    let mut reg = vo_worker::ConnectorRegistry::new();
    reg.register("http".into(), Box::new(vo_worker::HttpConnector::new("https://api.test.io")));
    assert_eq!(reg.get("http").unwrap().connector_type(), "http");
}

#[tokio::test]
async fn http_connector_prepare_commit() {
    let c = vo_worker::HttpConnector::new("https://svc.internal");
    let pe = c.prepare(serde_json::json!({"method":"POST","path":"/exec"}), "fx-0".into(), 3).await.unwrap();
    assert_eq!(pe.effect_id, "fx-0");
    assert_eq!(pe.fence, 3);
    assert_eq!(pe.payload["idempotency_key"], "fx-0:3");
}

#[tokio::test]
async fn http_connector_reconcile_not_committed() {
    let c = vo_worker::HttpConnector::new("https://svc.internal");
    assert_eq!(c.reconcile("fx-ghost").await.unwrap(), vo_worker::ReconcileOutcome::StillAmbiguous);
}
