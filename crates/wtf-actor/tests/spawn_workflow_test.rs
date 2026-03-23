//! Integration test: spawn_and_register via MasterOrchestrator RPC.
//!
//! Validates the full spawn path:
//! 1. Spawn MasterOrchestrator with test config (mock event store, no external deps).
//! 2. Send StartWorkflow via RPC — expect Ok(InstanceId).
//! 3. Send StartWorkflow again with same instance_id — expect Err(AlreadyExists).
//! 4. Send GetStatus for the spawned instance — expect Some(snapshot).
//!
//! Run with: cargo test -p wtf-actor --test spawn_workflow_test -- --test-threads=1

use async_trait::async_trait;
use bytes::Bytes;
use ractor::RpcReplyPort;
use ractor::{rpc::CallResult, Actor as _, ActorRef};
use std::sync::Arc;
use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig};
use wtf_actor::{InstanceStatusSnapshot, OrchestratorMsg, StartError, WorkflowParadigm};
use wtf_common::storage::{EventStore, ReplayBatch, ReplayStream, ReplayedEvent};
use wtf_common::{InstanceId, NamespaceId, WorkflowEvent, WtfError};

// ---------------------------------------------------------------------------
// Mock stores (no external dependencies required)
// ---------------------------------------------------------------------------

/// ReplayStream that immediately returns TailReached (no events to replay)
/// and hangs forever on next_live_event (live subscription not needed for tests).
#[derive(Debug)]
struct EmptyReplayStream;

#[async_trait]
impl ReplayStream for EmptyReplayStream {
    async fn next_event(&mut self) -> Result<ReplayBatch, WtfError> {
        Ok(ReplayBatch::TailReached)
    }

    async fn next_live_event(&mut self) -> Result<ReplayedEvent, WtfError> {
        // Hang forever — live subscription will be aborted when actor stops.
        std::future::pending().await
    }
}

/// EventStore that publishes successfully and returns empty replay streams.
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        max_instances: 10,
        engine_node_id: "test-node".into(),
        event_store: Some(Arc::new(MockEventStore)),
        ..OrchestratorConfig::default()
    }
}

const RPC_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

async fn start_workflow_rpc(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &str,
) -> Result<InstanceId, StartError> {
    let id = InstanceId::new(instance_id);
    let ns = NamespaceId::new("test-ns");
    let wt = "test-workflow".to_string();
    let paradigm = WorkflowParadigm::Procedural;
    let input = Bytes::from_static(b"{}");

    let result = orchestrator
        .call(
            |reply: RpcReplyPort<Result<InstanceId, StartError>>| OrchestratorMsg::StartWorkflow {
                namespace: ns,
                instance_id: id,
                workflow_type: wt,
                paradigm,
                input,
                reply,
            },
            Some(RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(CallResult::Success(Ok(id))) => Ok(id),
        Ok(CallResult::Success(Err(e))) => Err(e),
        Ok(CallResult::Timeout) => Err(StartError::SpawnFailed("RPC timeout".into())),
        Ok(CallResult::SenderError) => Err(StartError::SpawnFailed("sender error".into())),
        Err(_) => Err(StartError::SpawnFailed("RPC call failed".into())),
    }
}

async fn get_status_rpc(
    orchestrator: &ActorRef<OrchestratorMsg>,
    instance_id: &str,
) -> Option<InstanceStatusSnapshot> {
    let id = InstanceId::new(instance_id);
    let result = orchestrator
        .call(
            |reply| OrchestratorMsg::GetStatus {
                instance_id: id,
                reply,
            },
            Some(RPC_TIMEOUT),
        )
        .await;

    match result {
        Ok(CallResult::Success(Ok(Some(snapshot)))) => Some(snapshot),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn start_workflow_returns_instance_id() {
    let (orchestrator, _handle) = MasterOrchestrator::spawn(
        Some("test-orchestrator".into()),
        MasterOrchestrator,
        test_config(),
    )
    .await
    .expect("MasterOrchestrator should spawn");

    let result = start_workflow_rpc(&orchestrator, "inst-spawn-01").await;
    assert!(
        result.is_ok(),
        "StartWorkflow should succeed: {:?}",
        result.err()
    );
    let id = result.expect("ok");
    assert_eq!(id.as_str(), "inst-spawn-01");

    orchestrator.stop(Some("test complete".into()));
}

#[tokio::test]
async fn duplicate_instance_id_returns_already_exists() {
    let (orchestrator, _handle) = MasterOrchestrator::spawn(
        Some("test-orchestrator-dup".into()),
        MasterOrchestrator,
        test_config(),
    )
    .await
    .expect("MasterOrchestrator should spawn");

    let id_str = "inst-dup-01";

    // First start should succeed.
    let first = start_workflow_rpc(&orchestrator, id_str).await;
    assert!(
        first.is_ok(),
        "first start should succeed: {:?}",
        first.err()
    );

    // Second start with same ID should fail.
    let second = start_workflow_rpc(&orchestrator, id_str).await;
    assert!(
        matches!(second, Err(StartError::AlreadyExists(_))),
        "duplicate start should return AlreadyExists, got: {:?}",
        second
    );

    orchestrator.stop(Some("test complete".into()));
}

#[tokio::test]
async fn get_status_returns_snapshot_after_spawn() {
    let (orchestrator, _handle) = MasterOrchestrator::spawn(
        Some("test-orchestrator-status".into()),
        MasterOrchestrator,
        test_config(),
    )
    .await
    .expect("MasterOrchestrator should spawn");

    let id_str = "inst-status-01";

    // Spawn the instance.
    let start_result = start_workflow_rpc(&orchestrator, id_str).await;
    assert!(
        start_result.is_ok(),
        "start should succeed: {:?}",
        start_result.err()
    );

    // Query status — should return Some(snapshot).
    let snapshot = get_status_rpc(&orchestrator, id_str).await;
    assert!(
        snapshot.is_some(),
        "GetStatus should return Some(snapshot) after spawn"
    );
    let snap = snapshot.expect("snapshot");
    assert_eq!(snap.instance_id.as_str(), id_str);
    assert_eq!(snap.namespace.as_str(), "test-ns");
    assert_eq!(snap.workflow_type, "test-workflow");

    orchestrator.stop(Some("test complete".into()));
}

#[tokio::test]
async fn get_status_returns_none_for_unknown_instance() {
    let (orchestrator, _handle) = MasterOrchestrator::spawn(
        Some("test-orchestrator-unknown".into()),
        MasterOrchestrator,
        test_config(),
    )
    .await
    .expect("MasterOrchestrator should spawn");

    let snapshot = get_status_rpc(&orchestrator, "nonexistent-inst").await;
    assert!(
        snapshot.is_none(),
        "GetStatus should return None for unknown instance"
    );

    orchestrator.stop(Some("test complete".into()));
}
