use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use ractor::{Actor as _, ActorRef, RpcReplyPort};
use tokio::sync::{watch, Mutex as TokioMutex, OwnedMutexGuard};
use std::sync::Mutex as StdMutex;
use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig};
use wtf_actor::procedural::{WorkflowContext, WorkflowFn};
use wtf_actor::{InstanceStatusSnapshot, OrchestratorMsg, WorkflowParadigm};
use wtf_common::{
    EventStore, InstanceId, NamespaceId,
};
use wtf_common::storage::{ReplayBatch, ReplayStream, ReplayedEvent};
use wtf_common::{WorkflowEvent, WtfError};

const RPC_TIMEOUT: Duration = Duration::from_secs(5);
const WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

struct Harness {
    _guard: OwnedMutexGuard<()>,
    orchestrator: ActorRef<OrchestratorMsg>,
    shutdown_tx: watch::Sender<bool>,
    _tempdir: tempfile::TempDir,
}

fn global_lock() -> Arc<TokioMutex<()>> {
    static LOCK: OnceLock<Arc<TokioMutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(TokioMutex::new(()))).clone()
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

#[derive(Debug)]
struct MockEventStore;

#[async_trait]
impl EventStore for MockEventStore {
    async fn publish(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _event: WorkflowEvent,
    ) -> Result<u64, WtfError> {
        Ok(1)
    }

    async fn open_replay_stream(
        &self,
        _ns: &NamespaceId,
        _inst: &InstanceId,
        _from_seq: u64,
    ) -> Result<Box<dyn ReplayStream>, WtfError> {
        Ok(Box::new(EmptyReplayStream))
    }
}

#[derive(Debug)]
struct SignalWorkflow {
    signal_name: String,
    received_payload: Arc<StdMutex<Option<Bytes>>>,
    completion_tx: Arc<StdMutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl SignalWorkflow {
    fn new(
        signal_name: &str,
        received_payload: Arc<StdMutex<Option<Bytes>>>,
        completion_tx: Arc<StdMutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    ) -> Self {
        Self {
            signal_name: signal_name.to_string(),
            received_payload,
            completion_tx,
        }
    }
}

#[async_trait]
impl WorkflowFn for SignalWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let payload = ctx.wait_for_signal(&self.signal_name).await?;
        *self.received_payload.lock().unwrap() = Some(payload);
        if let Some(tx) = self.completion_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}

async fn setup_harness_with_workflows(
    test_name: &str,
    procedural_workflows: Vec<(String, Arc<dyn WorkflowFn>)>,
) -> Result<Harness, Box<dyn std::error::Error>> {
    let guard = global_lock().lock_owned().await;
    let tempdir = tempfile::tempdir()?;

    let event_store: Arc<dyn EventStore> = Arc::new(MockEventStore);
    let config = OrchestratorConfig {
        max_instances: 16,
        engine_node_id: format!("node-{test_name}"),
        snapshot_db: None,
        event_store: Some(event_store),
        state_store: None,
        task_queue: None,
        definitions: Vec::new(),
        procedural_workflows,
    };

    let (orchestrator, _) = MasterOrchestrator::spawn(
        Some(format!("test-orchestrator-{test_name}")),
        MasterOrchestrator,
        config,
    )
    .await?;

    let (shutdown_tx, _shutdown_rx) = watch::channel(false);

    Ok(Harness {
        _guard: guard,
        orchestrator,
        shutdown_tx,
        _tempdir: tempdir,
    })
}

async fn start_workflow(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
    workflow_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let call = orchestrator
        .call(
            |reply: RpcReplyPort<Result<InstanceId, wtf_actor::StartError>>| {
                OrchestratorMsg::StartWorkflow {
                    namespace: NamespaceId::new("test"),
                    instance_id: instance_id.clone(),
                    workflow_type: workflow_type.to_owned(),
                    paradigm: WorkflowParadigm::Procedural,
                    input: Bytes::from_static(b"{}"),
                    reply,
                }
            },
            Some(RPC_TIMEOUT),
        )
        .await?;

    match call {
        ractor::rpc::CallResult::Success(Ok(_)) => Ok(()),
        other => Err(format!("unexpected StartWorkflow result: {other:?}").into()),
    }
}

async fn send_signal(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
    signal_name: &str,
    payload: Bytes,
) -> Result<(), WtfError> {
    let call = orchestrator
        .call(
            |reply: RpcReplyPort<Result<(), WtfError>>| OrchestratorMsg::Signal {
                instance_id: instance_id.clone(),
                signal_name: signal_name.to_owned(),
                payload,
                reply,
            },
            Some(RPC_TIMEOUT),
        )
        .await;

    match call {
        Ok(ractor::rpc::CallResult::Success(result)) => result,
        Ok(ractor::rpc::CallResult::Timeout) => Err(WtfError::instance_not_found("RPC timeout")),
        Ok(ractor::rpc::CallResult::SenderError) => Err(WtfError::instance_not_found("sender error")),
        Err(e) => Err(WtfError::instance_not_found(&format!("call failed: {e}"))),
    }
}

async fn get_status(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
) -> Result<Option<InstanceStatusSnapshot>, Box<dyn std::error::Error>> {
    let call = orchestrator
        .call(
            |reply| OrchestratorMsg::GetStatus {
                instance_id: instance_id.clone(),
                reply,
            },
            Some(RPC_TIMEOUT),
        )
        .await?;

    match call {
        ractor::rpc::CallResult::Success(Ok(snapshot)) => Ok(snapshot),
        ractor::rpc::CallResult::Success(Err(err)) => Err(format!("GetStatus failed: {err:?}").into()),
        other => Err(format!("unexpected GetStatus result: {other:?}").into()),
    }
}

async fn wait_for<F, T>(mut f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnMut() -> futures::future::BoxFuture<'static, Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(value) = f().await {
            return Ok(value);
        }
        if start.elapsed() > WAIT_TIMEOUT {
            return Err("timed out waiting for condition".into());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_live_status(
    orchestrator: ActorRef<OrchestratorMsg>,
    instance_id: InstanceId,
) -> Result<InstanceStatusSnapshot, Box<dyn std::error::Error>> {
    wait_for(move || {
        let orchestrator = orchestrator.clone();
        let instance_id = instance_id.clone();
        Box::pin(async move {
            match get_status(&orchestrator, &instance_id).await.ok().flatten() {
                Some(snapshot)
                    if snapshot.phase == wtf_actor::InstancePhaseView::Live =>
                {
                    Some(snapshot)
                }
                _ => None,
            }
        })
    })
    .await
}

async fn teardown(harness: Harness) -> Result<(), Box<dyn std::error::Error>> {
    harness.orchestrator.stop(Some("test complete".into()));
    Ok(())
}

#[tokio::test]
async fn signal_delivery_to_waiting_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let signal_name = "approval";
    let expected_payload = Bytes::from_static(b"approved-true");

    let received_payload: Arc<StdMutex<Option<Bytes>>> = Arc::new(StdMutex::new(None));
    let (completion_tx, mut completion_rx) = tokio::sync::oneshot::channel();
    let wf: Arc<dyn WorkflowFn> = Arc::new(SignalWorkflow::new(
        signal_name,
        Arc::clone(&received_payload),
        Arc::new(StdMutex::new(Some(completion_tx))),
    ));

    let harness = setup_harness_with_workflows(
        "signal-waiter",
        vec![("sig-waiter-wf".to_owned(), wf)],
    )
    .await?;
    let instance_id = InstanceId::new("sig-waiter-001");

    start_workflow(&harness.orchestrator, &instance_id, "sig-waiter-wf").await?;
    let _started = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;

    send_signal(
        &harness.orchestrator,
        &instance_id,
        signal_name,
        expected_payload.clone(),
    )
    .await?;

    tokio::time::timeout(Duration::from_secs(5), &mut completion_rx)
        .await
        .expect("workflow should complete after signal")?;

    let payload_guard = received_payload.lock().unwrap();
    let payload_opt = payload_guard.as_ref().expect("payload should be set");
    assert_eq!(payload_opt, &expected_payload);

    teardown(harness).await
}

#[tokio::test]
async fn signal_to_nonexistent_instance_returns_error() -> Result<(), Box<dyn std::error::Error>>
{
    let harness = setup_harness_with_workflows("signal-notfound", vec![]).await?;
    let instance_id = InstanceId::new("nonexistent-instance");
    let result = send_signal(&harness.orchestrator, &instance_id, "anysignal", Bytes::new()).await;

    assert!(result.is_err(), "signal to nonexistent instance should fail");
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("not found") || err_str.contains("instance"),
        "error should mention instance not found, got: {err_str}"
    );

    teardown(harness).await
}

#[tokio::test]
async fn signal_with_empty_payload() -> Result<(), Box<dyn std::error::Error>> {
    let signal_name = "ping";

    let received_payload: Arc<StdMutex<Option<Bytes>>> = Arc::new(StdMutex::new(None));
    let (completion_tx, mut completion_rx) = tokio::sync::oneshot::channel();
    let wf: Arc<dyn WorkflowFn> = Arc::new(SignalWorkflow::new(
        signal_name,
        Arc::clone(&received_payload),
        Arc::new(StdMutex::new(Some(completion_tx))),
    ));

    let harness = setup_harness_with_workflows(
        "signal-empty",
        vec![("sig-empty-wf".to_owned(), wf)],
    )
    .await?;
    let instance_id = InstanceId::new("sig-empty-001");

    start_workflow(&harness.orchestrator, &instance_id, "sig-empty-wf").await?;
    let _started = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;

    send_signal(&harness.orchestrator, &instance_id, signal_name, Bytes::new()).await?;

    tokio::time::timeout(Duration::from_secs(5), &mut completion_rx)
        .await
        .expect("workflow should complete after signal")?;

    let payload_guard = received_payload.lock().unwrap();
    let payload_opt = payload_guard.as_ref().expect("payload should be set");
    assert_eq!(payload_opt, &Bytes::new());

    teardown(harness).await
}
