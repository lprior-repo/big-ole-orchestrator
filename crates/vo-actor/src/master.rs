//! Master orchestrator for actor supervision.
//!
//! Per ADR-015: The Master Orchestrator maintains the ActiveInstances registry
//! and enforces the Single-Writer invariant.

use std::collections::{BTreeMap, HashMap};

use bytes::Bytes;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use vo_types::InstanceId;

use crate::{
    CompensateError, InstancePhaseView, InstanceSnapshot, OrchestratorMsg, SignalError, StartError,
    TerminateError, WorkflowParadigm,
};

#[derive(Debug, Clone)]
pub struct MasterOrchestrator;

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_active_instances: u32,
    pub initial_instances: Vec<InstanceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeInstanceKey {
    namespace: String,
    instance_id: InstanceId,
}

#[derive(Debug, Clone)]
struct InstanceRecord {
    snapshot: InstanceSnapshot,
    signals_received: u64,
    compensation_requested: bool,
}

#[derive(Debug, Clone)]
struct PendingStartRecord {
    namespace: String,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: WorkflowParadigm,
    input: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingTransition {
    Signal { signal_name: String },
    Compensate,
    Terminate { reason: String },
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct MasterState {
    config: OrchestratorConfig,
    active: HashMap<RuntimeInstanceKey, InstanceRecord>,
    pending_starts: HashMap<RuntimeInstanceKey, PendingStartRecord>,
    pending_transitions: HashMap<RuntimeInstanceKey, PendingTransition>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_active_instances: 10_000,
            initial_instances: Vec::new(),
        }
    }
}

impl RuntimeInstanceKey {
    fn new(namespace: String, instance_id: InstanceId) -> Self {
        Self {
            namespace,
            instance_id,
        }
    }

    fn display(&self) -> String {
        format!("{}/{}", self.namespace, self.instance_id)
    }
}

impl Default for MasterOrchestrator {
    fn default() -> Self {
        Self
    }
}


impl MasterState {
    fn reserve_workflow_start(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
    ) -> Result<(), StartError> {
        let key = RuntimeInstanceKey::new(namespace.clone(), instance_id.clone());
        if self.active.contains_key(&key) {
            return Err(StartError::AlreadyExists(key.display()));
        }
        if self.pending_starts.contains_key(&key) {
            return Err(StartError::AlreadyExists(key.display()));
        }

        let running = u32::try_from(self.active.len().saturating_add(self.pending_starts.len()))
            .map_err(|_| {
                StartError::InvalidConfig("active instance count exceeds u32".to_string())
            })?;
        if running >= self.config.max_active_instances {
            return Err(StartError::AtCapacity {
                running,
                max: self.config.max_active_instances,
            });
        }

        self.pending_starts.insert(
            key,
            PendingStartRecord {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
            },
        );
        Ok(())
    }

    fn commit_workflow_start(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        workflow_type: String,
        paradigm: WorkflowParadigm,
        input: Bytes,
    ) -> Result<(), StartError> {
        let key = RuntimeInstanceKey::new(namespace.clone(), instance_id.clone());
        if self.active.contains_key(&key) {
            return Err(StartError::AlreadyExists(key.display()));
        }
        let pending = self.pending_starts.remove(&key).map_or(
            PendingStartRecord {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
            },
            |reserved| reserved,
        );

        let snapshot = InstanceSnapshot {
            instance_id: pending.instance_id.clone(),
            namespace: pending.namespace,
            workflow_type: pending.workflow_type,
            paradigm: pending.paradigm,
            phase: InstancePhaseView::Live,
            events_applied: initial_events_applied(&pending.input),
        };
        self.active.insert(
            key,
            InstanceRecord {
                snapshot,
                signals_received: 0,
                compensation_requested: false,
            },
        );
        Ok(())
    }

    fn abort_workflow_start(&mut self, namespace: String, instance_id: InstanceId) {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.pending_starts.remove(&key);
    }

    fn abort_workflow_transition(&mut self, namespace: String, instance_id: InstanceId) {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.pending_transitions.remove(&key);
    }

    fn get_status(&self, namespace: String, instance_id: InstanceId) -> Option<InstanceSnapshot> {
        self.active
            .get(&RuntimeInstanceKey::new(namespace, instance_id))
            .map(|record| record.snapshot.clone())
    }

    fn list_active(&self) -> Vec<InstanceSnapshot> {
        self.active
            .values()
            .map(|record| {
                (
                    format!(
                        "{}/{}",
                        record.snapshot.namespace, record.snapshot.instance_id
                    ),
                    record.snapshot.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect()
    }

    fn terminate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        reason: &str,
    ) -> Result<(), TerminateError> {
        if reason.is_empty() {
            return Err(TerminateError::Failed(
                "termination reason must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.active
            .remove(&key)
            .map(|_| ())
            .ok_or_else(|| TerminateError::NotFound(key.display()))
    }

    fn reserve_terminate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        reason: String,
    ) -> Result<(), TerminateError> {
        if reason.is_empty() {
            return Err(TerminateError::Failed(
                "termination reason must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.reserve_transition(key, PendingTransition::Terminate { reason })
            .map_err(terminate_reservation_error)
    }

    fn commit_terminate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        reason: String,
    ) -> Result<(), TerminateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        match self.pending_transitions.remove(&key) {
            Some(PendingTransition::Terminate { reason: reserved }) if reserved == reason => {
                self.active.remove(&key).map(|_| ()).ok_or_else(|| {
                    TerminateError::Failed(format!(
                        "reserved instance {} disappeared before terminate commit",
                        key.display()
                    ))
                })
            }
            Some(other) => Err(TerminateError::Failed(format!(
                "reserved transition mismatch for {}: {other:?}",
                key.display()
            ))),
            None => Err(TerminateError::Failed(format!(
                "terminate commit missing reservation for {}",
                key.display()
            ))),
        }
    }

    fn signal(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        signal_name: &str,
        payload: Bytes,
    ) -> Result<(), SignalError> {
        if signal_name.is_empty() {
            return Err(SignalError::Failed(
                "signal_name must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        let record = self
            .active
            .get_mut(&key)
            .ok_or_else(|| SignalError::NotFound(key.display()))?;
        record.signals_received = record
            .signals_received
            .checked_add(1)
            .ok_or_else(|| SignalError::Failed("signal counter overflow".to_string()))?;
        record.snapshot.events_applied = record
            .snapshot
            .events_applied
            .checked_add(signal_event_increment(&payload))
            .ok_or_else(|| SignalError::Failed("event counter overflow".to_string()))?;
        Ok(())
    }

    fn reserve_signal(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        signal_name: String,
    ) -> Result<(), SignalError> {
        if signal_name.is_empty() {
            return Err(SignalError::Failed(
                "signal_name must not be empty".to_string(),
            ));
        }
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.reserve_transition(key, PendingTransition::Signal { signal_name })
            .map_err(signal_reservation_error)
    }

    fn commit_signal(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
        signal_name: String,
        payload: Bytes,
    ) -> Result<(), SignalError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        match self.pending_transitions.remove(&key) {
            Some(PendingTransition::Signal {
                signal_name: reserved,
            }) if reserved == signal_name => {
                let record = self.active.get_mut(&key).ok_or_else(|| {
                    SignalError::Failed(format!(
                        "reserved instance {} disappeared before signal commit",
                        key.display()
                    ))
                })?;
                record.signals_received = record
                    .signals_received
                    .checked_add(1)
                    .ok_or_else(|| SignalError::Failed("signal counter overflow".to_string()))?;
                record.snapshot.events_applied = record
                    .snapshot
                    .events_applied
                    .checked_add(signal_event_increment(&payload))
                    .ok_or_else(|| SignalError::Failed("event counter overflow".to_string()))?;
                Ok(())
            }
            Some(other) => Err(SignalError::Failed(format!(
                "reserved transition mismatch for {}: {other:?}",
                key.display()
            ))),
            None => Err(SignalError::Failed(format!(
                "signal commit missing reservation for {}",
                key.display()
            ))),
        }
    }

    fn compensate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
    ) -> Result<(), CompensateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        let record = self
            .active
            .get_mut(&key)
            .ok_or_else(|| CompensateError::NotFound(key.display()))?;
        record.compensation_requested = true;
        record.snapshot.events_applied = record
            .snapshot
            .events_applied
            .checked_add(1)
            .ok_or_else(|| CompensateError::Failed("event counter overflow".to_string()))?;
        Ok(())
    }

    fn reserve_compensate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
    ) -> Result<(), CompensateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.reserve_transition(key, PendingTransition::Compensate)
            .map_err(compensate_reservation_error)
    }

    fn commit_compensate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
    ) -> Result<(), CompensateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        match self.pending_transitions.remove(&key) {
            Some(PendingTransition::Compensate) => {
                let record = self.active.get_mut(&key).ok_or_else(|| {
                    CompensateError::Failed(format!(
                        "reserved instance {} disappeared before compensation commit",
                        key.display()
                    ))
                })?;
                record.compensation_requested = true;
                record.snapshot.events_applied = record
                    .snapshot
                    .events_applied
                    .checked_add(1)
                    .ok_or_else(|| CompensateError::Failed("event counter overflow".to_string()))?;
                Ok(())
            }
            Some(other) => Err(CompensateError::Failed(format!(
                "reserved transition mismatch for {}: {other:?}",
                key.display()
            ))),
            None => Err(CompensateError::Failed(format!(
                "compensation commit missing reservation for {}",
                key.display()
            ))),
        }
    }

    fn reserve_transition(
        &mut self,
        key: RuntimeInstanceKey,
        transition: PendingTransition,
    ) -> Result<(), ReservationError> {
        if !self.active.contains_key(&key) {
            return Err(ReservationError::NotFound(key.display()));
        }
        if self.pending_transitions.contains_key(&key) {
            return Err(ReservationError::AlreadyReserved(key.display()));
        }
        self.pending_transitions.insert(key, transition);
        Ok(())
    }

    fn from_config(config: OrchestratorConfig) -> Self {
        let active = config
            .initial_instances
            .iter()
            .cloned()
            .map(|snapshot| {
                (
                    RuntimeInstanceKey::new(
                        snapshot.namespace.clone(),
                        snapshot.instance_id.clone(),
                    ),
                    InstanceRecord {
                        snapshot,
                        signals_received: 0,
                        compensation_requested: false,
                    },
                )
            })
            .collect();
        Self {
            config,
            active,
            pending_starts: HashMap::new(),
            pending_transitions: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReservationError {
    NotFound(String),
    AlreadyReserved(String),
}

fn terminate_reservation_error(error: ReservationError) -> TerminateError {
    match error {
        ReservationError::NotFound(id) => TerminateError::NotFound(id),
        ReservationError::AlreadyReserved(id) => {
            TerminateError::Failed(format!("instance {id} already has a pending transition"))
        }
    }
}

fn signal_reservation_error(error: ReservationError) -> SignalError {
    match error {
        ReservationError::NotFound(id) => SignalError::NotFound(id),
        ReservationError::AlreadyReserved(id) => {
            SignalError::Failed(format!("instance {id} already has a pending transition"))
        }
    }
}

fn compensate_reservation_error(error: ReservationError) -> CompensateError {
    match error {
        ReservationError::NotFound(id) => CompensateError::NotFound(id),
        ReservationError::AlreadyReserved(id) => {
            CompensateError::Failed(format!("instance {id} already has a pending transition"))
        }
    }
}

fn initial_events_applied(_input: &Bytes) -> u64 {
    1
}

fn signal_event_increment(_payload: &Bytes) -> u64 {
    1
}

impl Actor for MasterOrchestrator {
    type Msg = OrchestratorMsg;
    type State = MasterState;
    type Arguments = OrchestratorConfig;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(MasterState::from_config(args))
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            OrchestratorMsg::StartWorkflow {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.commit_workflow_start(namespace, instance_id, workflow_type, paradigm, input),
            ),
            OrchestratorMsg::ReserveWorkflowStart {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.reserve_workflow_start(
                    namespace,
                    instance_id,
                    workflow_type,
                    paradigm,
                    input,
                ),
            ),
            OrchestratorMsg::CommitWorkflowStart {
                namespace,
                instance_id,
                workflow_type,
                paradigm,
                input,
                reply,
            } => send_reply(
                reply,
                state.commit_workflow_start(namespace, instance_id, workflow_type, paradigm, input),
            ),
            OrchestratorMsg::AbortWorkflowStart {
                namespace,
                instance_id,
                reply,
            } => {
                let _: () = state.abort_workflow_start(namespace, instance_id);
                send_reply(reply, ())
            },
            OrchestratorMsg::GetStatus {
                namespace,
                instance_id,
                reply,
            } => {
                send_reply(reply, state.get_status(namespace, instance_id));
            }
            OrchestratorMsg::Terminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(reply, state.terminate(namespace, instance_id, &reason)),
            OrchestratorMsg::ReserveTerminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(
                reply,
                state.reserve_terminate(namespace, instance_id, reason),
            ),
            OrchestratorMsg::CommitTerminate {
                namespace,
                instance_id,
                reason,
                reply,
            } => send_reply(
                reply,
                state.commit_terminate(namespace, instance_id, reason),
            ),
            OrchestratorMsg::AbortWorkflowTransition {
                namespace,
                instance_id,
                reply,
            } => {
                let _: () = state.abort_workflow_transition(namespace, instance_id);
                send_reply(
                    reply,
                    (),
                )
            },
            OrchestratorMsg::ListActive { reply } => send_reply(reply, state.list_active()),
            OrchestratorMsg::Compensate {
                namespace,
                instance_id,
                reply,
            } => {
                send_reply(reply, state.compensate(namespace, instance_id));
            }
            OrchestratorMsg::ReserveCompensate {
                namespace,
                instance_id,
                reply,
            } => send_reply(reply, state.reserve_compensate(namespace, instance_id)),
            OrchestratorMsg::CommitCompensate {
                namespace,
                instance_id,
                reply,
            } => send_reply(reply, state.commit_compensate(namespace, instance_id)),
            OrchestratorMsg::Signal {
                namespace,
                instance_id,
                signal_name,
                payload,
                reply,
            } => send_reply(
                reply,
                state.signal(namespace, instance_id, &signal_name, payload),
            ),
            OrchestratorMsg::ReserveSignal {
                namespace,
                instance_id,
                signal_name,
                reply,
            } => send_reply(
                reply,
                state.reserve_signal(namespace, instance_id, signal_name),
            ),
            OrchestratorMsg::CommitSignal {
                namespace,
                instance_id,
                signal_name,
                payload,
                reply,
            } => send_reply(
                reply,
                state.commit_signal(namespace, instance_id, signal_name, payload),
            ),
        }
        Ok(())
    }
}

fn send_reply<T: Send + 'static>(reply: ractor::port::RpcReplyPort<T>, value: T) {
    if let Err(error) = reply.send(value) {
        tracing::warn!(?error, "orchestrator reply receiver dropped");
    }
}
