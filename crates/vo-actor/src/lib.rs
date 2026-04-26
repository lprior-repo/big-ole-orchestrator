//! Actor framework for vo-engine.
//!
//! Provides the actor model implementation using the Ractor library.
//! Actors are the fundamental units of computation in the engine.

use bytes::Bytes;
use vo_types::InstanceId;
use vo_types::{SequenceNumber, TimerId, WorkflowName};

// Module declarations
pub mod heartbeat;
pub mod async_message_router;
pub mod fairness;
pub mod instance_registry;
pub mod lifecycle;
pub mod message_router;
pub mod port;
pub mod probe;
pub mod reanimator;
pub mod semaphore;
pub mod signal_buffer;
pub mod signals;
pub mod spawn_supervisor;
pub mod timer_lifecycle;
pub mod timers;
pub mod timer_supervisor;

// Internal modules
mod domain_types;
mod error_types;
mod orchestrator_msgs;
mod instance_msgs;
mod control_msgs;
mod budget;
mod control_actor;
mod control_actor_ops;
mod control_actor_tests;
mod accept_resume_tests;
mod accept_resume_ops_tests;
mod instance_msg_construct;
mod instance_msg_clone;
mod control_msg_construct;
mod control_msg_clone;
mod msg_partial_eq;
mod msg_eq_props;
mod msg_traits;

#[cfg(test)]
pub mod signal_buffer_tests;

#[cfg(test)]
pub mod instance_registry_tests;

#[cfg(test)]
pub mod timer_supervisor_tests;

pub mod signal_messages;

// =============================================================================
// Re-exports from modules (preserving original public API)
// =============================================================================

pub use domain_types::{InstancePhaseView, NamespaceId, WorkflowParadigm};
pub use error_types::{CompensateError, SignalError, StartError, TerminateError};
pub use orchestrator_msgs::{InstanceSnapshot, OrchestratorMsg};
pub use instance_msgs::InstanceActorMessage;
pub use control_msgs::ControlActorMessage;
pub use budget::{ReservedPermitBudget, WorkloadClass};
pub use control_actor::ControlActor;

pub use signal_messages::mock_signal_storage;
pub use signal_messages::mock_signal_storage::{MockSignalStorage, MockSignalWorkQueue};
pub use signal_messages::{
    AcceptResumeError, AcceptResumeOutcome, BinaryHash, CancelError, CancelRequested,
    ContinueAsNewError, InstanceResumed, LifecycleState, NodeName, ResumeError, RolloverState,
    SecretId, SignalAccepted, SignalName, SignalPayload, SignalStorage, SignalStorageError,
    SignalWorkQueue, SignalWorkQueueError, StateLookup, TestStateLookup, TimestampMs, WaitKey,
    WorkflowCancelled, WorkflowContinued,
};
