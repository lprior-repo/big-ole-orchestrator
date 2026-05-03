//! Master orchestrator for actor supervision.
//!
//! Per ADR-015: The Master Orchestrator maintains the ActiveInstances registry
//! and enforces the Single-Writer invariant.

pub(crate) mod supervision;
pub(crate) mod lifecycle;

use std::collections::{BTreeMap, HashMap};

use bytes::Bytes;
use vo_types::InstanceId;

use crate::actor_messages::{
    CompensateError, InstancePhaseView, InstanceSnapshot, OrchestratorMsg, SignalError,
    TerminateError, WorkflowParadigm,
};

/// Top-level orchestrator for actor supervision.
#[derive(Debug, Clone)]
pub struct MasterOrchestrator;

/// Configuration for the master orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub max_active_instances: u32,
    pub initial_instances: Vec<InstanceSnapshot>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_active_instances: 10_000,
            initial_instances: Vec::new(),
        }
    }
}

/// Key for looking up instances in the runtime registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct RuntimeInstanceKey {
    pub(crate) namespace: String,
    pub(crate) instance_id: InstanceId,
}

impl RuntimeInstanceKey {
    pub(crate) fn new(namespace: String, instance_id: InstanceId) -> Self {
        Self {
            namespace,
            instance_id,
        }
    }

    pub(crate) fn display(&self) -> String {
        format!("{}/{}", self.namespace, self.instance_id)
    }
}

/// Internal record for a managed instance.
#[derive(Debug, Clone)]
pub(crate) struct InstanceRecord {
    snapshot: InstanceSnapshot,
    signals_received: u64,
    compensation_requested: bool,
}

/// Pending workflow start request.
#[derive(Debug, Clone)]
pub(crate) struct PendingStartRecord {
    namespace: String,
    instance_id: InstanceId,
    workflow_type: String,
    paradigm: WorkflowParadigm,
    input: Bytes,
}

/// Pending state transition awaiting commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingTransition {
    Signal { signal_name: String },
    Compensate,
    Terminate { reason: String },
}

/// Complete state of the master orchestrator.
#[derive(Debug, Clone, Default)]
pub struct MasterState {
    config: OrchestratorConfig,
    active: HashMap<RuntimeInstanceKey, InstanceRecord>,
    pending_starts: HashMap<RuntimeInstanceKey, PendingStartRecord>,
    pending_transitions: HashMap<RuntimeInstanceKey, PendingTransition>,
}

impl Default for MasterOrchestrator {
    fn default() -> Self {
        Self
    }
}

impl MasterState {
    /// Construct state from configuration, seeding with initial instances.
    pub(crate) fn from_config(config: OrchestratorConfig) -> Self {
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

/// Increment applied to event counter when a workflow is first started.
pub(crate) fn initial_events_applied(_input: &Bytes) -> u64 {
    1
}

/// Increment applied to event counter when a signal payload is received.
pub(crate) fn signal_event_increment(_payload: &Bytes) -> u64 {
    1
}
