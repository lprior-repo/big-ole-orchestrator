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
struct LongRunningWorkflow {
    _cancel_flag: Arc<StdMutex<bool>>,
}

impl LongRunningWorkflow {
    fn new(cancel_flag: Arc<StdMutex<bool>>) -> Self {
        Self { _cancel_flag: cancel_flag }
    }
}

#[async_trait]
impl WorkflowFn for LongRunningWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = ctx.sleep(Duration::from_secs(60)).await;
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

async fn terminate_instance(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
    reason: String,
) -> Result<(), wtf_actor::TerminateError> {
    let call = orchestrator
        .call(
            |reply: RpcReplyPort<Result<(), wtf_actor::TerminateError>>| {
                OrchestratorMsg::Terminate {
                    instance_id: instance_id.clone(),
                    reason,
                    reply,
                }
            },
            Some(RPC_TIMEOUT),
        )
        .await;

    match call {
        Ok(ractor::rpc::CallResult::Success(result)) => result,
        Ok(ractor::rpc::CallResult::Timeout) => Err(wtf_actor::TerminateError::Timeout(instance_id.clone())),
        Ok(ractor::rpc::CallResult::SenderError) => Err(wtf_actor::TerminateError::NotFound(instance_id.clone())),
        Err(_e) => Err(wtf_actor::TerminateError::NotFound(instance_id.clone())),
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

async fn list_active(
    orchestrator: &ActorRef<OrchestratorMsg>,
) -> Result<Vec<InstanceStatusSnapshot>, Box<dyn std::error::Error>> {
    let call = orchestrator
        .call(|reply| OrchestratorMsg::ListActive { reply }, Some(RPC_TIMEOUT))
        .await?;

    match call {
        ractor::rpc::CallResult::Success(list) => Ok(list),
        other => Err(format!("unexpected ListActive result: {other:?}").into()),
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

async fn wait_for_instance_removed(
    orchestrator: ActorRef<OrchestratorMsg>,
    instance_id: InstanceId,
) -> Result<(), Box<dyn std::error::Error>> {
    wait_for(move || {
        let orchestrator = orchestrator.clone();
        let instance_id = instance_id.clone();
        Box::pin(async move {
            match get_status(&orchestrator, &instance_id).await {
                Ok(None) => Some(()),
                Ok(Some(_)) => None,
                Err(_) => Some(()),
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
async fn terminate_running_instance_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let cancel_flag: Arc<StdMutex<bool>> = Arc::new(StdMutex::new(false));
    let wf: Arc<dyn WorkflowFn> = Arc::new(LongRunningWorkflow::new(Arc::clone(&cancel_flag)));

    let harness = setup_harness_with_workflows(
        "terminate-running",
        vec![("long-running".to_owned(), wf)],
    )
    .await?;
    let instance_id = InstanceId::new("term-running-001");

    start_workflow(&harness.orchestrator, &instance_id, "long-running").await?;
    let _started = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;

    let result = terminate_instance(
        &harness.orchestrator,
        &instance_id,
        "api-terminate".to_string(),
    )
    .await;
    assert!(result.is_ok(), "terminate should succeed: {:?}", result);

    wait_for_instance_removed(harness.orchestrator.clone(), instance_id.clone()).await?;

    let status = get_status(&harness.orchestrator, &instance_id).await;
    assert!(status.unwrap().is_none(), "instance should be gone after terminate");

    teardown(harness).await
}

#[tokio::test]
async fn terminate_nonexistent_instance_returns_not_found() -> Result<(), Box<dyn std::error::Error>> {
    let harness = setup_harness_with_workflows("terminate-notfound", vec![]).await?;
    let instance_id = InstanceId::new("nonexistent-instance");

    let result = terminate_instance(
        &harness.orchestrator,
        &instance_id,
        "test-terminate".to_string(),
    )
    .await;

    assert!(result.is_err(), "terminate nonexistent should fail");
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("not found") || format!("{:?}", err).contains("NotFound"),
        "error should be NotFound: {:?}",
        err
    );

    teardown(harness).await
}

#[tokio::test]
async fn terminate_removes_instance_from_active_list() -> Result<(), Box<dyn std::error::Error>> {
    let cancel_flag: Arc<StdMutex<bool>> = Arc::new(StdMutex::new(false));
    let wf: Arc<dyn WorkflowFn> = Arc::new(LongRunningWorkflow::new(Arc::clone(&cancel_flag)));

    let harness = setup_harness_with_workflows(
        "terminate-active-list",
        vec![("long-running-2".to_owned(), wf)],
    )
    .await?;
    let instance_id = InstanceId::new("term-active-001");

    start_workflow(&harness.orchestrator, &instance_id, "long-running-2").await?;
    let _started = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;

    let active_before = list_active(&harness.orchestrator).await?;
    assert_eq!(active_before.len(), 1, "should have 1 active instance before terminate");
    assert_eq!(active_before[0].instance_id, instance_id);

    terminate_instance(
        &harness.orchestrator,
        &instance_id,
        "api-terminate".to_string(),
    )
    .await?;

    wait_for_instance_removed(harness.orchestrator.clone(), instance_id.clone()).await?;

    let active_after = list_active(&harness.orchestrator).await?;
    assert_eq!(active_after.len(), 0, "should have 0 active instances after terminate");

    teardown(harness).await
}
