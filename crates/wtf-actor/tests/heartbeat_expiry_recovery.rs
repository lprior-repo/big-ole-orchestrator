use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use bytes::Bytes;
use futures::TryStreamExt;
use ractor::{Actor as _, ActorRef, RpcReplyPort};
use tokio::sync::{watch, Mutex, OwnedMutexGuard};
use wtf_actor::heartbeat::run_heartbeat_watcher;
use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig, WorkflowDefinition};
use wtf_actor::{InstancePhaseView, InstanceStatusSnapshot, OrchestratorMsg, WorkflowParadigm};
use wtf_common::{EventStore, InstanceId, NamespaceId, StateStore, WorkflowEvent};
use wtf_storage::{connect, heartbeat_key, open_snapshot_db, provision_kv_buckets, provision_streams, NatsClient, NatsConfig};

const RPC_TIMEOUT: Duration = Duration::from_secs(5);
const WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

struct Harness {
    _guard: OwnedMutexGuard<()>,
    _nats: NatsClient,
    orchestrator: ActorRef<OrchestratorMsg>,
    heartbeats: async_nats::jetstream::kv::Store,
    instances: async_nats::jetstream::kv::Store,
    shutdown_tx: watch::Sender<bool>,
    watcher: tokio::task::JoinHandle<Result<(), String>>,
    _tempdir: tempfile::TempDir,
}

fn global_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

async fn setup_harness(test_name: &str) -> Result<Harness, Box<dyn std::error::Error>> {
    let guard = global_lock().lock_owned().await;
    let nats = connect(&NatsConfig::default()).await?;
    let js = nats.jetstream().clone();
    reset_nats(&js).await;
    provision_streams(&js).await?;
    let kv = provision_kv_buckets(&js).await?;

    let tempdir = tempfile::tempdir()?;
    let db = open_snapshot_db(tempdir.path())?;

    let event_store: Arc<dyn EventStore> = Arc::new(nats.clone());
    let state_store: Arc<dyn StateStore> = Arc::new(nats.clone());
    let config = OrchestratorConfig {
        max_instances: 16,
        engine_node_id: format!("node-{test_name}"),
        snapshot_db: Some(db),
        event_store: Some(event_store),
        state_store: Some(state_store),
        task_queue: None,
        definitions: vec![(
            "checkout-fsm".to_owned(),
            fsm_definition(),
        )],
        procedural_workflows: Vec::new(),
    };

    let (orchestrator, _) = MasterOrchestrator::spawn(
        Some(format!("test-orchestrator-{test_name}")),
        MasterOrchestrator,
        config,
    )
    .await?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let watcher = tokio::spawn(run_heartbeat_watcher(
        kv.heartbeats.clone(),
        orchestrator.clone(),
        shutdown_rx,
    ));

    Ok(Harness {
        _guard: guard,
        _nats: nats,
        orchestrator,
        heartbeats: kv.heartbeats,
        instances: kv.instances,
        shutdown_tx,
        watcher,
        _tempdir: tempdir,
    })
}

async fn reset_nats(js: &async_nats::jetstream::Context) {
    for name in ["wtf-events", "wtf-work", "wtf-signals", "wtf-archive"] {
        let _ = js.delete_stream(name).await;
    }
    for name in ["wtf-instances", "wtf-timers", "wtf-definitions", "wtf-heartbeats"] {
        let _ = js.delete_key_value(name).await;
    }
}

fn fsm_definition() -> WorkflowDefinition {
    WorkflowDefinition {
        paradigm: WorkflowParadigm::Fsm,
        graph_raw: r#"{
            "initial_state":"Created",
            "transitions":[
                {"from":"Created","event":"authorize","to":"Authorized"}
            ]
        }"#
            .to_owned(),
        description: Some("checkout recovery test".to_owned()),
    }
}

async fn start_workflow(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
) -> Result<(), Box<dyn std::error::Error>> {
    let call = orchestrator
        .call(
            |reply: RpcReplyPort<Result<InstanceId, wtf_actor::StartError>>| {
                OrchestratorMsg::StartWorkflow {
                    namespace: NamespaceId::new("test"),
                    instance_id: instance_id.clone(),
                    workflow_type: "checkout-fsm".to_owned(),
                    paradigm: WorkflowParadigm::Fsm,
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
                Some(snapshot) if snapshot.phase == InstancePhaseView::Live => Some(snapshot),
                _ => None,
            }
        })
    })
    .await
}

async fn wait_for_active_count(
    orchestrator: ActorRef<OrchestratorMsg>,
    expected: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let _: () = wait_for(move || {
        let orchestrator = orchestrator.clone();
        Box::pin(async move {
            match list_active(&orchestrator).await {
                Ok(list) if list.len() == expected => Some(()),
                _ => None,
            }
        })
    })
    .await?;
    Ok(())
}

async fn wait_for_heartbeat(
    heartbeats: async_nats::jetstream::kv::Store,
    instance_id: InstanceId,
) -> Result<(), Box<dyn std::error::Error>> {
    let key = heartbeat_key(&instance_id);
    let _: () = wait_for(move || {
        let heartbeats = heartbeats.clone();
        let key = key.clone();
        Box::pin(async move {
            match heartbeats.get(&key).await {
                Ok(Some(_)) => Some(()),
                _ => None,
            }
        })
    })
    .await?;
    Ok(())
}

async fn stop_instance(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &InstanceId,
) -> Result<(), Box<dyn std::error::Error>> {
    let actor = ActorRef::<wtf_actor::InstanceMsg>::where_is(format!("wf-{}", instance_id.as_str()))
        .ok_or_else(|| "workflow actor not found".to_string())?;
    actor.stop(Some("simulated crash".into()));
    wait_for_active_count(orchestrator.clone(), 0).await
}

async fn teardown(harness: Harness) -> Result<(), Box<dyn std::error::Error>> {
    let Harness {
        orchestrator,
        shutdown_tx,
        watcher,
        ..
    } = harness;

    let _ = shutdown_tx.send(true);
    let watcher_result = watcher.await?;
    if let Err(err) = watcher_result {
        return Err(err.into());
    }
    orchestrator.stop(Some("test complete".into()));
    Ok(())
}

#[tokio::test]
async fn no_recovery_when_instance_active() -> Result<(), Box<dyn std::error::Error>> {
    let harness = setup_harness("active").await?;
    let instance_id = InstanceId::new("hb-active-001");

    start_workflow(&harness.orchestrator, &instance_id).await?;
    let before = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;
    harness
        .orchestrator
        .cast(OrchestratorMsg::HeartbeatExpired {
            instance_id: instance_id.clone(),
        })?;

    tokio::time::sleep(Duration::from_millis(300)).await;
    let active = list_active(&harness.orchestrator).await?;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].instance_id, before.instance_id);
    assert_eq!(active[0].current_state, before.current_state);

    teardown(harness).await
}

#[tokio::test]
async fn heartbeat_watcher_shutdown_clean() -> Result<(), Box<dyn std::error::Error>> {
    let harness = setup_harness("shutdown").await?;
    teardown(harness).await
}

#[tokio::test]
async fn crash_recovery_fsm_heartbeat_expiry() -> Result<(), Box<dyn std::error::Error>> {
    let harness = setup_harness("recovery").await?;
    let instance_id = InstanceId::new("hb-recovery-001");

    start_workflow(&harness.orchestrator, &instance_id).await?;
    let started = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;
    wait_for_heartbeat(harness.heartbeats.clone(), instance_id.clone()).await?;

    let event_store: Arc<dyn EventStore> = Arc::new(harness._nats.clone());
    event_store
        .publish(
            &NamespaceId::new("test"),
            &instance_id,
            WorkflowEvent::TransitionApplied {
                from_state: "Created".to_owned(),
                event_name: "authorize".to_owned(),
                to_state: "Authorized".to_owned(),
                effects: vec![],
            },
        )
        .await?;

    let orchestrator = harness.orchestrator.clone();
    let tracked_id = instance_id.clone();
    let expected_events = started.events_applied + 1;
    let pre_crash = wait_for(move || {
        let orchestrator = orchestrator.clone();
        let instance_id = tracked_id.clone();
        Box::pin(async move {
            match get_status(&orchestrator, &instance_id).await.ok().flatten() {
                Some(snapshot)
                    if snapshot.events_applied >= expected_events
                        && snapshot.current_state.as_deref() == Some("Authorized") =>
                {
                    Some(snapshot)
                }
                _ => None,
            }
        })
    })
    .await?;

    stop_instance(&harness.orchestrator, &instance_id).await?;
    harness.heartbeats.delete(heartbeat_key(&instance_id)).await?;

    let recovered = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;
    assert_eq!(recovered.phase, InstancePhaseView::Live);
    assert_eq!(recovered.events_applied, pre_crash.events_applied);
    assert_eq!(recovered.current_state.as_deref(), Some("Authorized"));

    let metadata_key = harness
        .instances
        .keys()
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    assert!(metadata_key.iter().any(|key| key == "test/hb-recovery-001"));

    teardown(harness).await
}

#[tokio::test]
async fn duplicate_heartbeat_expired_triggers_single_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let harness = setup_harness("dedupe").await?;
    let instance_id = InstanceId::new("hb-dedupe-001");

    start_workflow(&harness.orchestrator, &instance_id).await?;
    wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;
    wait_for_heartbeat(harness.heartbeats.clone(), instance_id.clone()).await?;
    stop_instance(&harness.orchestrator, &instance_id).await?;

    harness
        .orchestrator
        .cast(OrchestratorMsg::HeartbeatExpired {
            instance_id: instance_id.clone(),
        })?;
    harness
        .orchestrator
        .cast(OrchestratorMsg::HeartbeatExpired {
            instance_id: instance_id.clone(),
        })?;

    let recovered = wait_for_live_status(harness.orchestrator.clone(), instance_id.clone()).await?;
    assert_eq!(recovered.phase, InstancePhaseView::Live);

    let active = list_active(&harness.orchestrator).await?;
    let matching = active
        .iter()
        .filter(|snapshot| snapshot.instance_id == instance_id)
        .count();
    assert_eq!(matching, 1);
    assert!(ractor::ActorRef::<wtf_actor::InstanceMsg>::where_is(format!("wf-recovered-{}", instance_id.as_str())).is_some());

    teardown(harness).await
}
