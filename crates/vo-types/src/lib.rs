mod blob;
mod command_metadata;
mod compensation;
mod connector;
mod dedupe;
#[cfg(test)]
mod dedupe_tests;
mod effects;
mod errors;
pub mod events;
mod instance_status;
mod integer_types;
#[cfg(test)]
mod integer_types_tests;
mod lifecycle_superstate;
mod lineage;
mod node_kind;
mod non_empty_vec;
mod payload_parser;
pub mod proptest_verifier;
mod registration_status;
mod signal;
pub mod state;
mod string_types;
#[cfg(test)]
mod string_types_tests;
mod types;
#[cfg(test)]
mod types_tests;
mod workflow;

pub use blob::{BlobRef, BlobStatus, OutputRef, INLINED_MAX_BYTES};
pub use command_metadata::{CommandMetadata, Issuer};
pub use compensation::{
    apply_compensation_transition, CompensationRecord, CompensationStatus,
    CompensationTransitionError, CompensationTransitionEvent,
};
pub use connector::{
    apply_connector_transition, ConnectorResult, ConnectorState, ConnectorTransition,
    ConnectorTransitionError, ReconcileAction,
};
pub use dedupe::{DedupeKey, DedupePartitionKey};
pub use effects::{
    apply_effect_transition, CompensationPolicy, EffectIntent, EffectKind, EffectRecord,
    EffectTransitionError, EffectTransitionEvent,
};
pub use errors::ParseError;
pub use events::{Error as EventError, EventEnvelope};
pub use instance_status::InstanceStatus;
pub use lifecycle_superstate::LifecycleSuperstate;
pub use lineage::{Epoch, LineageError, WorkflowLineage};
pub use node_kind::NodeKind;
pub use non_empty_vec::NonEmptyVec;
pub use registration_status::RegistrationStatus;
pub use signal::{
    BufferPolicy, SignalAddress, SignalDedupeKey, SignalDelivery, WaitKey, WaitRecord,
};
pub use types::{
    extract_schema_version, AttemptNumber, BinaryHash, DurationMs, EventVersion, FenceToken,
    FireAtMs, IdempotencyKey, InstanceId, LeaseRecord, MaxAttempts, NodeName, SequenceNumber,
    Snapshot, State, StepId, TimeoutMs, TimerId, TimestampMs, WorkflowName, WorkflowSpec,
    MAX_SUPPORTED_SCHEMA_VERSION,
};
pub use workflow::{
    next_nodes, DagNode, Edge, EdgeCondition, RetryPolicy, RetryPolicyError, StepOutcome,
    WorkflowDefinition, WorkflowDefinitionError,
};

#[cfg(test)]
mod adversarial_tests;
#[cfg(test)]
mod compensation_tests;
#[cfg(test)]
mod cross_cutting_tests;
#[cfg(test)]
mod red_queen_tests;
#[cfg(test)]
mod schema_version_tests;
#[cfg(test)]
mod serde_tests;
#[cfg(test)]
mod workflow_tests;
