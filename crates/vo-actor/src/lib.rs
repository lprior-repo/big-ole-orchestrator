//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

pub use vo_common::NamespaceId;

pub mod heartbeat {
    pub fn run_heartbeat_watcher() {}
}

pub mod master {
    pub struct MasterOrchestrator;
    pub struct OrchestratorConfig;
}

// Extracted modules (from lib.rs split — ve-eztxc)
pub mod budget;
pub mod control_actor;
pub mod orchestrator_types;

pub mod fairness;
pub mod instance_registry;
pub mod lifecycle;
pub mod message_router;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod routing;
pub mod semaphore;
pub mod signal_buffer;
pub mod signals;
pub mod spawn_supervisor;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;
pub mod timer_lifecycle;
pub mod timer_supervisor;
pub mod timer_supervisor_tests;
pub mod timers;

// Actor message types
pub mod actor_messages;
pub mod signal_messages;

pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalPayload, SignalStorage, SignalStorageError, SignalWorkQueue,
    SignalWorkQueueError, TimestampMs, WaitKey, WorkflowCancelled, WorkflowContinued,
};
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::mock_signal_storage;

// Re-exports from extracted modules (preserve crate-level API)
pub use budget::{ReservedPermitBudget, StartError};
pub use control_actor::ControlActor;
pub use orchestrator_types::{
    CompensateError, InstancePhaseView, InstanceSnapshot, OrchestratorMsg, TerminateError,
    WorkflowParadigm,
};

pub use actor_messages::{ControlActorMessage, InstanceActorMessage};
