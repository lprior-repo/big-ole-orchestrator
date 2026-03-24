use crate::instance::init::*;
use crate::messages::{InstanceArguments, WorkflowParadigm};
use async_trait::async_trait;
use std::sync::Arc;
use wtf_common::storage::{EventStore, ReplayBatch, ReplayStream, ReplayedEvent};
use wtf_common::{NamespaceId, WtfError, WorkflowEvent};

/// Shared capture for published events, accessible across the trait object boundary.
type PublishedCapture = Arc<std::sync::Mutex<Vec<WorkflowEvent>>>;

fn new_capture() -> PublishedCapture {
    Arc::new(std::sync::Mutex::new(Vec::new()))
}

/// A mock event store that records calls and returns success.
#[derive(Debug)]
struct RecordingEventStore {
    published_events: PublishedCapture,
}

impl RecordingEventStore {
    fn new(capture: PublishedCapture) -> Self {
        Self {
            published_events: capture,
        }
    }
}

#[async_trait]
impl EventStore for RecordingEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &wtf_common::InstanceId,
        event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        self.published_events
            .lock()
            .expect("lock")
            .push(event);
        Ok(1)
    }

    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &wtf_common::InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyStream))
    }
}

#[derive(Debug)]
struct EmptyStream;

#[async_trait]
impl ReplayStream for EmptyStream {
    async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
        Ok(ReplayBatch::TailReached)
    }
    async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError> {
        std::future::pending().await
    }
}

fn fresh_args(capture: PublishedCapture) -> InstanceArguments {
    InstanceArguments {
        namespace: wtf_common::NamespaceId::new("test-ns"),
        instance_id: wtf_common::InstanceId::new("inst-abc"),
        workflow_type: "order_flow".into(),
        paradigm: WorkflowParadigm::Fsm,
        input: bytes::Bytes::from_static(b"{\"order\": 42}"),
        engine_node_id: "node-1".into(),
        event_store: Some(Arc::new(RecordingEventStore::new(capture))),
        state_store: None,
        task_queue: None,
        snapshot_db: None,
        procedural_workflow: None,
        workflow_definition: None,
    }
}

// Test 1: Fresh instance publishes InstanceStarted
#[tokio::test]
async fn fresh_instance_publishes_started_event() {
    let capture = new_capture();
    let args = fresh_args(Arc::clone(&capture));
    let event_log: Vec<WorkflowEvent> = vec![];

    let result = publish_instance_started(&args, 1, &event_log).await;

    assert!(result.is_ok(), "Expected Ok(()) but got {:?}", result.err());

    let published = capture.lock().expect("lock").clone();
    assert_eq!(published.len(), 1, "Expected exactly 1 published event");

    match &published[0] {
        WorkflowEvent::InstanceStarted {
            instance_id,
            workflow_type,
            input,
        } => {
            assert_eq!(instance_id, "inst-abc");
            assert_eq!(workflow_type, "order_flow");
            assert_eq!(input.as_ref(), b"{\"order\": 42}");
        }
        other => panic!("Expected InstanceStarted variant, got {:?}", other),
    }
}

// Test 2: Crash recovery skips InstanceStarted
#[tokio::test]
async fn crash_recovery_skips_started_event() {
    let capture = new_capture();
    let args = fresh_args(Arc::clone(&capture));
    let event_log = vec![WorkflowEvent::SnapshotTaken { seq: 1, checksum: 0 }];

    let result = publish_instance_started(&args, 1, &event_log).await;

    assert!(result.is_ok(), "Expected Ok(()) but got {:?}", result.err());

    let published = capture.lock().expect("lock").clone();
    assert!(
        published.is_empty(),
        "Expected no events published on crash recovery, got {}",
        published.len()
    );
}

// Test 3: No event_store returns error
#[tokio::test]
async fn no_event_store_returns_error() {
    let capture = new_capture();
    let mut args = fresh_args(capture);
    args.event_store = None;
    let event_log: Vec<WorkflowEvent> = vec![];

    let result = publish_instance_started(&args, 1, &event_log).await;

    assert!(result.is_err(), "Expected Err for missing event_store");
    let err_msg = format!("{:?}", result.err().expect("is err"));
    assert!(
        err_msg.contains("No event store"),
        "Error should mention 'No event store', got: {}",
        err_msg
    );
}

#[test]
fn snapshot_recovery_without_tail_skips_started_event() {
    assert!(should_skip_instance_started(2, &[]));
}

#[test]
fn fresh_instance_without_replay_publishes_started_event() {
    assert!(!should_skip_instance_started(1, &[]));
}
