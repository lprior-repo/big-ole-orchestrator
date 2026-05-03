//! Supervision strategy — reserve/commit lifecycle for the ActiveInstances registry.
//!
//! Per ADR-015: The Master Orchestrator enforces the Single-Writer invariant
//! by requiring a reservation phase before any commit of workflow operations.

use bytes::Bytes;
use vo_types::InstanceId;

use crate::actor_messages::{
    CompensateError, InstancePhaseView, InstanceSnapshot, SignalError, TerminateError,
    WorkflowParadigm,
};
use crate::start_budget::StartError;

use super::{
    InstanceRecord, MasterState, PendingStartRecord, PendingTransition, RuntimeInstanceKey,
    initial_events_applied, signal_event_increment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReservationError {
    NotFound(String),
    AlreadyReserved(String),
}

impl MasterState {
    pub(crate) fn reserve_workflow_start(
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

    pub(crate) fn commit_workflow_start(
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

    pub(crate) fn abort_workflow_start(&mut self, namespace: String, instance_id: InstanceId) {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.pending_starts.remove(&key);
    }

    pub(crate) fn reserve_transition(
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

    pub(crate) fn abort_workflow_transition(&mut self, namespace: String, instance_id: InstanceId) {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.pending_transitions.remove(&key);
    }

    pub(crate) fn reserve_terminate(
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

    pub(crate) fn commit_terminate(
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

    pub(crate) fn reserve_signal(
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

    pub(crate) fn commit_signal(
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

    pub(crate) fn reserve_compensate(
        &mut self,
        namespace: String,
        instance_id: InstanceId,
    ) -> Result<(), CompensateError> {
        let key = RuntimeInstanceKey::new(namespace, instance_id);
        self.reserve_transition(key, PendingTransition::Compensate)
            .map_err(compensate_reservation_error)
    }

    pub(crate) fn commit_compensate(
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
