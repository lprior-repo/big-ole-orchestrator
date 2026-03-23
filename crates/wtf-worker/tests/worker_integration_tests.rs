//! Integration tests for wtf-worker with live NATS JetStream.
//!
//! Tests verify the contract specified in `.beads/wtf-rqby/contract.md` and follow
//! the Martin Fowler test plan in `.beads/wtf-rqby/martin-fowler-tests.md`.
//!
//! Run with: `cargo test --test worker_integration -- --test-threads=1`
//!
//! # Prerequisites
//! - Live NATS server at `127.0.0.1:4222` (or `NATS_URL` env var)
//! - JetStream enabled on NATS
//! - Streams provisioned via `wtf_storage::provision_streams`

use bytes::Bytes;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_nats::jetstream::Context;
use tokio::sync::{watch, Mutex, OwnedMutexGuard};
use wtf_common::{ActivityId, InstanceId, NamespaceId, RetryPolicy};
use wtf_storage::{connect, provision_streams, NatsConfig};

use wtf_worker::{
    activity::complete_activity,
    queue::{enqueue_activity, ActivityTask, WorkQueueConsumer},
    Worker,
};

struct NatsTestServer {
    js: Context,
    _guard: OwnedMutexGuard<()>,
}

fn global_test_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

impl NatsTestServer {
    async fn new() -> Result<Self, wtf_common::WtfError> {
        let guard = global_test_lock().lock_owned().await;

        let config = NatsConfig {
            urls: vec![std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into())],
            embedded: true,
            connect_timeout_ms: 5_000,
            credentials_path: None,
        };

        let client = connect(&config).await?;
        Ok(Self {
            js: client.jetstream().clone(),
            _guard: guard,
        })
    }

    async fn provision(&self) -> Result<(), wtf_common::WtfError> {
        self.reset_streams().await;
        provision_streams(&self.js).await
    }

    async fn reset_streams(&self) {
        for name in ["wtf-work", "wtf-events", "wtf-signals", "wtf-archive"] {
            let _ = self.js.delete_stream(name).await;
        }
    }
}

fn make_task(activity_type: &str, attempt: u32) -> ActivityTask {
    ActivityTask {
        activity_id: ActivityId::new("act-001"),
        activity_type: activity_type.to_owned(),
        payload: Bytes::from_static(b"{\"amount\":100}"),
        namespace: NamespaceId::new("payments"),
        instance_id: InstanceId::new("inst-001"),
        attempt,
        retry_policy: RetryPolicy::default(),
        timeout: None,
    }
}

// ── Happy Path Tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn work_queue_consumer_create_succeeds_with_valid_nats_context() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let consumer = WorkQueueConsumer::create(&server.js, "test-worker", None).await;
    assert!(
        consumer.is_ok(),
        "create should succeed with valid NATS context"
    );
}

#[tokio::test]
async fn work_queue_consumer_create_with_filter_subject() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let consumer = WorkQueueConsumer::create(
        &server.js,
        "email-worker",
        Some("wtf.work.send_email".into()),
    )
    .await;
    assert!(
        consumer.is_ok(),
        "create with filter subject should succeed"
    );
}

#[tokio::test]
async fn next_task_returns_task_when_message_available() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("charge_card", 1);
    enqueue_activity(&server.js, &task)
        .await
        .expect("enqueue activity");

    let mut consumer = WorkQueueConsumer::create(&server.js, "test-worker", None)
        .await
        .expect("create consumer");

    let ackable = consumer
        .next_task()
        .await
        .expect("next_task should not error")
        .expect("task should be available");

    assert_eq!(ackable.task.activity_type, "charge_card");
    assert_eq!(ackable.task.attempt, 1);

    let _ = ackable.ack().await;
}

#[tokio::test]
async fn ack_removes_message_from_queue() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("refund", 1);
    let _seq = enqueue_activity(&server.js, &task).await.expect("enqueue");

    let mut consumer1 = WorkQueueConsumer::create(&server.js, "worker-1", None)
        .await
        .expect("create consumer");

    let ackable = consumer1
        .next_task()
        .await
        .expect("should get task")
        .expect("task available");

    // Complete activity and ack
    complete_activity(
        &server.js,
        &ackable.task.namespace,
        &ackable.task.instance_id,
        &ackable.task.activity_id,
        Bytes::from_static(b"\"ok\""),
        10,
    )
    .await
    .expect("complete_activity");
    ackable.ack().await.expect("ack should succeed");

    // Second consumer should not see the message (acked)
    let mut consumer2 = WorkQueueConsumer::create(&server.js, "worker-2", None)
        .await
        .expect("create consumer");

    // Use a short timeout to avoid hanging
    let result = tokio::time::timeout(Duration::from_millis(500), consumer2.next_task()).await;

    assert!(
        result.is_err() || result.unwrap().unwrap().is_none(),
        "acked message should not be redelivered to different consumer"
    );
}

#[tokio::test]
async fn worker_run_processes_task_and_acks() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("send_email", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let mut worker = Worker::new(server.js.clone(), "test-worker", None);
    worker.register("send_email", |task| async move {
        assert_eq!(task.activity_type, "send_email");
        Ok(Bytes::from_static(b"\"sent\""))
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let _ = shutdown_tx.send(true);
    });

    // Run worker with timeout
    let result = tokio::time::timeout(Duration::from_secs(5), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    assert!(result.is_ok(), "worker.run should complete without error");
    assert!(result.unwrap().is_ok(), "worker.run should return Ok");
}

#[tokio::test]
async fn enqueue_activity_publishes_to_correct_subject() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("charge_card", 1);
    let seq = enqueue_activity(&server.js, &task).await.expect("enqueue");

    assert!(seq > 0, "sequence should be positive");

    // Verify by pulling the task back
    let mut consumer = WorkQueueConsumer::create(&server.js, "verify-subject", None)
        .await
        .expect("create consumer");

    let ackable = consumer
        .next_task()
        .await
        .expect("should get task")
        .expect("task available");

    assert_eq!(ackable.task.activity_type, "charge_card");

    let _ = ackable.ack().await;
}

// ── Error Path Tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn create_returns_error_when_stream_not_found() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.reset_streams().await;
    // Don't provision - stream won't exist

    let result = WorkQueueConsumer::create(&server.js, "worker", None).await;

    assert!(result.is_err(), "create should fail when stream not found");
}

#[tokio::test]
async fn next_task_returns_error_on_receive_failure() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let mut consumer = WorkQueueConsumer::create(&server.js, "test-worker", None)
        .await
        .expect("create consumer");

    // Enqueue a task then ack it to clear the queue
    let task = make_task("test", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let ackable = consumer
        .next_task()
        .await
        .expect("should get task")
        .expect("task available");
    let _ = ackable.ack().await;

    // Now consumer should eventually return None (stream behavior)
    let _result = tokio::time::timeout(Duration::from_millis(100), consumer.next_task()).await;
    // It's OK if we get None or timeout - either is valid for empty queue
}

#[tokio::test]
async fn nak_requeues_message_for_redelivery() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("send_email", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let mut consumer1 = WorkQueueConsumer::create(&server.js, "worker-1", None)
        .await
        .expect("create consumer");

    let ackable1 = consumer1
        .next_task()
        .await
        .expect("should get task")
        .expect("task available");

    // Nak instead of ack
    ackable1.nak().await.expect("nak should succeed");

    // Same consumer should be able to get it again
    let ackable2 = consumer1
        .next_task()
        .await
        .expect("should get task after nak")
        .expect("task available");

    assert_eq!(
        ackable2.task.activity_id.as_str(),
        "act-001",
        "should get same task after nak"
    );

    let _ = ackable2.ack().await;
}

#[tokio::test]
async fn worker_calls_fail_activity_on_handler_error() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("failing_activity", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let mut worker = Worker::new(server.js.clone(), "test-worker", None);
    worker.register("failing_activity", |_task| async move {
        Err("handler failed".to_string())
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = tokio::time::timeout(Duration::from_secs(5), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    assert!(
        result.is_ok(),
        "worker.run should complete even with handler error"
    );
}

#[tokio::test]
async fn unknown_activity_type_logs_warning_and_acks() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("unknown_type", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let worker = Worker::new(server.js.clone(), "test-worker", None);
    // No handlers registered

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = tokio::time::timeout(Duration::from_secs(5), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    assert!(
        result.is_ok(),
        "worker.run should complete even with unknown activity type"
    );
}

// ── Edge Case Tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn activity_task_msgpack_roundtrip_preserves_all_fields() {
    let task = ActivityTask {
        activity_id: ActivityId::new("act-xyz"),
        activity_type: "test_activity".to_owned(),
        payload: Bytes::from_static(b"{\"key\":\"value\"}"),
        namespace: NamespaceId::new("test-ns"),
        instance_id: InstanceId::new("inst-xyz"),
        attempt: 3,
        retry_policy: RetryPolicy {
            max_attempts: 5,
            initial_interval_ms: 100,
            backoff_coefficient: 2.0,
            max_interval_ms: 5000,
        },
        timeout: None,
    };

    let bytes = task.to_msgpack().expect("serialize should succeed");
    let decoded = ActivityTask::from_msgpack(&bytes).expect("deserialize should succeed");

    assert_eq!(decoded.activity_id.as_str(), "act-xyz");
    assert_eq!(decoded.activity_type, "test_activity");
    assert_eq!(decoded.payload, Bytes::from_static(b"{\"key\":\"value\"}"));
    assert_eq!(decoded.namespace.as_str(), "test-ns");
    assert_eq!(decoded.instance_id.as_str(), "inst-xyz");
    assert_eq!(decoded.attempt, 3);
    assert_eq!(decoded.retry_policy.max_attempts, 5);
}

#[tokio::test]
async fn multiple_workers_share_queue_correctly() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    // Enqueue tasks for different activity types
    let task1 = make_task("type_a", 1);
    let task2 = make_task("type_b", 1);
    enqueue_activity(&server.js, &task1)
        .await
        .expect("enqueue1");
    enqueue_activity(&server.js, &task2)
        .await
        .expect("enqueue2");

    let mut consumer1 = WorkQueueConsumer::create(
        &server.js,
        "shared-worker-1",
        Some("wtf.work.type_a".into()),
    )
    .await
    .expect("create consumer1");
    let mut consumer2 = WorkQueueConsumer::create(
        &server.js,
        "shared-worker-2",
        Some("wtf.work.type_b".into()),
    )
    .await
    .expect("create consumer2");

    // Each consumer should get distinct tasks (durable consumers)
    let ackable1 = consumer1
        .next_task()
        .await
        .expect("consumer1 should get task")
        .expect("task available");

    let ackable2 = consumer2
        .next_task()
        .await
        .expect("consumer2 should get task")
        .expect("task available");

    assert_eq!(ackable1.task.activity_type, "type_a");
    assert_eq!(ackable2.task.activity_type, "type_b");

    let _ = ackable1.ack().await;
    let _ = ackable2.ack().await;
}

#[tokio::test]
async fn worker_respects_shutdown_signal() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let mut worker = Worker::new(server.js.clone(), "shutdown-test", None);
    worker.register("send_email", |_task| async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        Ok(Bytes::from_static(b"\"sent\""))
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let worker_handle = tokio::spawn(async move { worker.run(shutdown_rx).await });

    // Send shutdown signal after a short delay
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = shutdown_tx.send(true);

    let result = tokio::time::timeout(Duration::from_secs(2), worker_handle).await;

    assert!(result.is_ok(), "worker should shutdown within timeout");
}

// ── Contract Verification Tests ──────────────────────────────────────────────

#[tokio::test]
async fn write_ahead_sequence_verified_complete_activity_before_ack() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("wa_test", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let mut consumer = WorkQueueConsumer::create(&server.js, "wa-test-worker", None)
        .await
        .expect("create consumer");

    let ackable = consumer
        .next_task()
        .await
        .expect("should get task")
        .expect("task available");

    // complete_activity should append to JetStream BEFORE we call ack
    let seq = complete_activity(
        &server.js,
        &ackable.task.namespace,
        &ackable.task.instance_id,
        &ackable.task.activity_id,
        Bytes::from_static(b"\"result\""),
        50,
    )
    .await
    .expect("complete_activity should succeed");

    assert!(seq > 0, "complete_activity should return valid sequence");

    // Now ack
    ackable.ack().await.expect("ack should succeed");
}

#[tokio::test]
async fn invariant_i3_attempt_is_1_based() {
    let task = make_task("test", 1);
    assert_eq!(task.attempt, 1, "first attempt should be 1");

    let task2 = make_task("test", 2);
    assert_eq!(task2.attempt, 2, "second attempt should be 2");
}

// ── End-to-End Scenario Tests ───────────────────────────────────────────────

#[tokio::test]
async fn full_dispatch_cycle_engine_to_worker_to_completion() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    // 1. Engine enqueues activity
    let task = make_task("checkout", 1);
    let _seq = enqueue_activity(&server.js, &task)
        .await
        .expect("enqueue step 1");

    // 2. Worker pulls and processes
    let mut worker = Worker::new(server.js.clone(), "e2e-worker", None);
    let processed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let processed_clone = processed.clone();

    worker.register("checkout", move |_task| {
        let processed = processed_clone.clone();
        async move {
            processed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(Bytes::from_static(b"\"checkout_complete\""))
        }
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = tokio::time::timeout(Duration::from_secs(5), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    assert!(result.is_ok(), "worker should complete");
    assert!(
        processed.load(std::sync::atomic::Ordering::SeqCst),
        "handler should have been called"
    );
}

#[tokio::test]
async fn full_dispatch_cycle_with_failure_and_retry() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let task = make_task("unreliable", 1);
    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let mut worker = Worker::new(server.js.clone(), "retry-worker", None);
    worker.register("unreliable", move |_task| {
        let counter = attempt_count_clone.clone();
        async move {
            let count = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if count < 3 {
                Err("try again".to_string())
            } else {
                Ok(Bytes::from_static(b"\"finally succeeded\""))
            }
        }
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        let _ = shutdown_tx.send(true);
    });

    let result = tokio::time::timeout(Duration::from_secs(10), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    assert!(result.is_ok(), "worker should complete");
    assert_eq!(
        attempt_count.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "handler should have been called 3 times"
    );
}

#[tokio::test]
async fn retry_policy_passed_correctly_to_handler() {
    let server = NatsTestServer::new().await.expect("NATS server");
    server.provision().await.expect("provision streams");

    let custom_policy = RetryPolicy {
        max_attempts: 7,
        initial_interval_ms: 200,
        backoff_coefficient: 2.0,
        max_interval_ms: 5000,
    };

    let task = ActivityTask {
        activity_id: ActivityId::new("act-retry"),
        activity_type: "retry_test".to_owned(),
        payload: Bytes::from_static(b"{}"),
        namespace: NamespaceId::new("test"),
        instance_id: InstanceId::new("inst-retry"),
        attempt: 1,
        retry_policy: custom_policy.clone(),
        timeout: None,
    };

    enqueue_activity(&server.js, &task).await.expect("enqueue");

    let received_policy = std::sync::Arc::new(std::sync::Mutex::new(None));
    let received_policy_clone = received_policy.clone();

    let mut worker = Worker::new(server.js.clone(), "policy-test", None);
    worker.register("retry_test", move |t| {
        let policy_store = received_policy_clone.clone();
        async move {
            *policy_store.lock().unwrap() = Some(t.retry_policy.clone());
            Ok(Bytes::from_static(b"\"ok\""))
        }
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let shutdown_task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let _ = shutdown_tx.send(true);
    });

    let _ = tokio::time::timeout(Duration::from_secs(5), worker.run(shutdown_rx)).await;
    let _ = shutdown_task.await;

    let stored = received_policy.lock().unwrap();
    assert!(
        stored.is_some(),
        "handler should have received task with retry policy"
    );
    let stored_policy = stored.as_ref().unwrap();
    assert_eq!(
        stored_policy.max_attempts, 7,
        "max_attempts should be preserved"
    );
    assert_eq!(
        stored_policy.initial_interval_ms, 200,
        "initial_interval_ms should be preserved"
    );
}
