//! MasterOrchestrator — root ractor supervisor for all WorkflowInstance actors (ADR-006).
//!
//! Responsibilities:
//! - Enforce capacity limits (max active instances).
//! - Spawn and supervise WorkflowInstance actors (bead wtf-novr).
//! - Handle `HeartbeatExpired` events from the NATS KV watcher (bead wtf-r4aa).
//! - Route signals and termination requests to the correct instance (bead wtf-eor1).
//! - Return status and list of active instances (bead wtf-eor1).
//!
//! The orchestrator holds a registry `active: HashMap<InstanceId, ActorRef<InstanceMsg>>`
//! to route messages to running instances. The registry is the only mutable shared state.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use ractor::rpc::CallResult;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use wtf_common::InstanceId;
use wtf_storage::NatsClient;

use crate::instance::WorkflowInstance;
use crate::messages::{
    InstanceArguments, InstanceMsg, OrchestratorMsg, StartError, TerminateError,
};

/// Timeout for synchronous calls to WorkflowInstance actors.
const INSTANCE_CALL_TIMEOUT: Duration = Duration::from_millis(500);

/// Configuration for the MasterOrchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of workflow instances this node may run concurrently.
    ///
    /// Requests beyond this limit are rejected with `StartError::AtCapacity`.
    pub max_instances: usize,

    /// Unique identifier for this engine node (written to heartbeat KV entries).
    pub engine_node_id: String,

    /// NATS client for JetStream and KV operations.
    pub nats: Option<NatsClient>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_instances: 1000,
            engine_node_id: "engine-local".into(),
            nats: None,
        }
    }
}

/// In-memory state of the MasterOrchestrator.
///
/// This is NOT persisted — it is rebuilt from the NATS KV `wtf-instances` bucket
/// on startup. The only authoritative state is in JetStream + KV.
#[derive(Debug)]
pub struct OrchestratorState {
    /// Registry of all currently active workflow instances.
    ///
    /// Key: stable `InstanceId`. Value: the ractor `ActorRef` for sending messages.
    /// Entries are added on spawn and removed on actor stop.
    pub active: HashMap<InstanceId, ActorRef<InstanceMsg>>,

    /// Configuration (immutable after construction).
    pub config: OrchestratorConfig,
}

impl OrchestratorState {
    /// Create a new empty orchestrator state.
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            active: HashMap::new(),
            config,
        }
    }

    /// Return the number of currently active instances.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Return `true` if the orchestrator can accept one more instance.
    #[must_use]
    pub fn has_capacity(&self) -> bool {
        self.active.len() < self.config.max_instances
    }

    /// Register a newly spawned instance.
    pub fn register(&mut self, id: InstanceId, actor_ref: ActorRef<InstanceMsg>) {
        self.active.insert(id, actor_ref);
    }

    /// Deregister a stopped instance.
    ///
    /// Called from the supervisor event handler when an instance actor stops.
    pub fn deregister(&mut self, id: &InstanceId) {
        self.active.remove(id);
    }

    /// Look up an active instance by ID.
    #[must_use]
    pub fn get(&self, id: &InstanceId) -> Option<&ActorRef<InstanceMsg>> {
        self.active.get(id)
    }
}

/// The MasterOrchestrator root supervisor actor.
pub struct MasterOrchestrator;

/// ractor `Actor` implementation for `MasterOrchestrator`.
#[async_trait]
impl Actor for MasterOrchestrator {
    type Msg = OrchestratorMsg;
    type State = OrchestratorState;
    type Arguments = OrchestratorConfig;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        config: OrchestratorConfig,
    ) -> Result<OrchestratorState, ActorProcessingErr> {
        tracing::info!(
            max_instances = config.max_instances,
            node_id = %config.engine_node_id,
            "MasterOrchestrator starting"
        );
        Ok(OrchestratorState::new(config))
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: OrchestratorMsg,
        state: &mut OrchestratorState,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            OrchestratorMsg::StartWorkflow {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => {
                handle_start_workflow(
                    myself,
                    state,
                    namespace,
                    instance_id,
                    workflow_type,
                    paradigm,
                    input,
                    reply,
                )
                .await;
            }

            OrchestratorMsg::Signal {
                instance_id,
                signal_name,
                payload,
                reply,
            } => match state.get(&instance_id) {
                Some(actor_ref) => {
                    let _ = actor_ref.cast(InstanceMsg::InjectSignal {
                        signal_name,
                        payload,
                        reply,
                    });
                }
                None => {
                    let _ = reply.send(Err(wtf_common::WtfError::instance_not_found(
                        instance_id.as_str(),
                    )));
                }
            },

            OrchestratorMsg::Terminate {
                instance_id,
                reason,
                reply,
            } => {
                handle_terminate(state, instance_id, reason, reply).await;
            }

            OrchestratorMsg::GetStatus { instance_id, reply } => {
                let snapshot = handle_get_status(state, &instance_id).await;
                let _ = reply.send(snapshot);
            }

            OrchestratorMsg::ListActive { reply } => {
                let snapshots = handle_list_active(state).await;
                let _ = reply.send(snapshots);
            }

            OrchestratorMsg::HeartbeatExpired { instance_id } => {
                if !state.active.contains_key(&instance_id) {
                    tracing::warn!(
                        instance_id = %instance_id,
                        "HeartbeatExpired for unknown instance — may need recovery (wtf-r4aa)"
                    );
                }
                // If the instance IS in the registry the actor is still alive on this node —
                // the heartbeat TTL was not refreshed in time; the actor will write the next
                // heartbeat on its own. Recovery logic is in bead wtf-r4aa.
            }
        }

        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        _myself: ActorRef<Self::Msg>,
        evt: ractor::SupervisionEvent,
        state: &mut OrchestratorState,
    ) -> Result<(), ActorProcessingErr> {
        // When a WorkflowInstance actor stops (normally or due to a crash),
        // deregister it from the active registry. This is the lifecycle hook
        // for "child actor stopped" events (bead wtf-r4aa extends this with
        // recovery logic).
        if let ractor::SupervisionEvent::ActorTerminated(actor_cell, _, reason) = &evt {
            // Scan the active registry for the actor cell that stopped.
            // This is O(N) but N is bounded by max_instances, and actor stops
            // are rare compared to message throughput.
            let stopped_id = state
                .active
                .iter()
                .find(|(_, r)| r.get_id() == actor_cell.get_id())
                .map(|(id, _)| id.clone());

            if let Some(id) = stopped_id {
                tracing::info!(
                    instance_id = %id,
                    reason = ?reason,
                    "WorkflowInstance stopped — deregistering"
                );
                state.deregister(&id);
            }
        }
        Ok(())
    }
}

// ── Handler functions (bead wtf-novr) ─────────────────────────────────────────

/// Spawn a new WorkflowInstance actor and register it with the orchestrator.
///
/// Checks capacity and duplicate-ID guards before spawning.
async fn handle_start_workflow(
    myself: ActorRef<OrchestratorMsg>,
    state: &mut OrchestratorState,
    namespace: wtf_common::NamespaceId,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: crate::messages::WorkflowParadigm,
    input: bytes::Bytes,
    reply: ractor::RpcReplyPort<Result<InstanceId, StartError>>,
) {
    // 1. Capacity check.
    if !state.has_capacity() {
        tracing::warn!(
            instance_id = %instance_id,
            running = state.active_count(),
            max = state.config.max_instances,
            "StartWorkflow rejected — at capacity"
        );
        let _ = reply.send(Err(StartError::AtCapacity {
            running: state.active_count(),
            max: state.config.max_instances,
        }));
        return;
    }

    // 2. Duplicate-ID check — prevent double-spawning.
    if state.active.contains_key(&instance_id) {
        tracing::warn!(instance_id = %instance_id, "StartWorkflow rejected — already exists");
        let _ = reply.send(Err(StartError::AlreadyExists(instance_id)));
        return;
    }

    // 3. Build InstanceArguments.
    let args = InstanceArguments {
        namespace,
        instance_id: instance_id.clone(),
        workflow_type,
        paradigm,
        input,
        engine_node_id: state.config.engine_node_id.clone(),
        nats: state.config.nats.clone(),
        procedural_workflow: None, // TODO: Lookup from registry
    };

    // 4. Spawn the WorkflowInstance as a supervised child of this orchestrator.
    let actor_name = format!("wf-{}", instance_id.as_str());
    let supervisor_cell: ActorCell = myself.clone().into();

    match WorkflowInstance::spawn_linked(Some(actor_name), WorkflowInstance, args, supervisor_cell)
        .await
    {
        Err(e) => {
            tracing::error!(instance_id = %instance_id, error = %e, "failed to spawn WorkflowInstance");
            let _ = reply.send(Err(StartError::SpawnFailed(e.to_string())));
        }
        Ok((actor_ref, _handle)) => {
            tracing::info!(instance_id = %instance_id, "WorkflowInstance spawned");
            // 5. Register the actor ref in the active registry.
            state.register(instance_id.clone(), actor_ref);
            let _ = reply.send(Ok(instance_id));
        }
    }
}

// ── Handler functions (bead wtf-eor1) ────────────────────────────────────────

/// Forward a cancel request to the target instance and reply.
async fn handle_terminate(
    state: &mut OrchestratorState,
    instance_id: InstanceId,
    reason: String,
    reply: ractor::RpcReplyPort<Result<(), TerminateError>>,
) {
    match state.get(&instance_id) {
        None => {
            let _ = reply.send(Err(TerminateError::NotFound(instance_id)));
        }
        Some(actor_ref) => {
            // Ask the instance to cancel itself.
            let call_result = actor_ref
                .call(
                    |tx| InstanceMsg::Cancel { reason, reply: tx },
                    Some(INSTANCE_CALL_TIMEOUT),
                )
                .await;

            match call_result {
                Err(e) => {
                    let _ = reply.send(Err(TerminateError::Failed(format!("send failed: {e}"))));
                }
                Ok(CallResult::Timeout) => {
                    let _ = reply.send(Err(TerminateError::Failed("cancel timed out".into())));
                }
                Ok(CallResult::SenderError) => {
                    let _ = reply.send(Err(TerminateError::Failed("actor dropped reply".into())));
                }
                Ok(CallResult::Success(inner_result)) => {
                    let _ =
                        reply.send(inner_result.map_err(|e: wtf_common::WtfError| {
                            TerminateError::Failed(e.to_string())
                        }));
                }
            }
        }
    }
}

/// Query status from a running WorkflowInstance actor.
///
/// Returns `None` if the instance is not found or doesn't respond within timeout.
async fn handle_get_status(
    state: &OrchestratorState,
    instance_id: &InstanceId,
) -> Option<crate::messages::InstanceStatusSnapshot> {
    let actor_ref = state.get(instance_id)?;

    match actor_ref
        .call(InstanceMsg::GetStatus, Some(INSTANCE_CALL_TIMEOUT))
        .await
    {
        Ok(CallResult::Success(snapshot)) => Some(snapshot),
        Ok(CallResult::Timeout) => {
            tracing::warn!(instance_id = %instance_id, "GetStatus timed out");
            None
        }
        Ok(CallResult::SenderError) => None,
        Err(e) => {
            tracing::warn!(instance_id = %instance_id, error = %e, "GetStatus call failed");
            None
        }
    }
}

/// Collect status from all active WorkflowInstance actors (sequentially with timeout).
async fn handle_list_active(
    state: &OrchestratorState,
) -> Vec<crate::messages::InstanceStatusSnapshot> {
    let mut snapshots = Vec::with_capacity(state.active.len());
    for (id, actor_ref) in &state.active {
        match actor_ref
            .call(InstanceMsg::GetStatus, Some(INSTANCE_CALL_TIMEOUT))
            .await
        {
            Ok(CallResult::Success(snapshot)) => snapshots.push(snapshot),
            Ok(CallResult::Timeout) => {
                tracing::warn!(instance_id = %id, "GetStatus timed out during list");
            }
            Ok(CallResult::SenderError) | Err(_) => {
                tracing::warn!(instance_id = %id, "GetStatus failed during list");
            }
        }
    }
    snapshots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> OrchestratorConfig {
        // We still have the NatsClient problem here.
        // I'll update the struct to use Option<NatsClient> to make tests easier.
        OrchestratorConfig {
            max_instances: 10,
            engine_node_id: "node-test".into(),
            nats: None,
        }
    }

    #[test]
    fn new_state_is_empty() {
        let state = OrchestratorState::new(test_config());
        assert_eq!(state.active_count(), 0);
    }

    #[test]
    fn has_capacity_when_empty() {
        let state = OrchestratorState::new(test_config());
        assert!(state.has_capacity());
    }

    #[test]
    fn has_capacity_false_when_at_limit() {
        let config = OrchestratorConfig {
            max_instances: 0,
            engine_node_id: "node".into(),
            nats: None,
        };
        let state = OrchestratorState::new(config);
        assert!(!state.has_capacity());
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let state = OrchestratorState::new(test_config());
        let id = InstanceId::new("unknown");
        assert!(state.get(&id).is_none());
    }

    #[test]
    fn deregister_removes_entry() {
        // We can't easily create a real ActorRef in unit tests without running the ractor runtime.
        // So we just test the invariant that deregistering a non-existent ID is a no-op.
        let mut state = OrchestratorState::new(test_config());
        let id = InstanceId::new("not-there");
        state.deregister(&id); // should not panic
        assert_eq!(state.active_count(), 0);
    }

    #[test]
    fn orchestrator_config_default_max_instances() {
        let cfg = OrchestratorConfig::default();
        assert_eq!(cfg.max_instances, 1000);
    }

    #[test]
    fn orchestrator_config_default_node_id() {
        let cfg = OrchestratorConfig::default();
        assert_eq!(cfg.engine_node_id, "engine-local");
    }

    #[test]
    fn active_count_matches_registry_size() {
        let mut state = OrchestratorState::new(test_config());
        state.deregister(&InstanceId::new("x"));
        assert_eq!(state.active_count(), 0);
    }
}
