#[cfg(test)]
mod attempt_number_tests;
#[cfg(test)]
mod binary_hash_tests;
mod binomial_heap;
#[cfg(test)]
mod blackhat_encryption_credentials_tests;
mod blob;
#[cfg(test)]
mod blob_tests;
pub mod cartesian_tree;
mod clique_tree;
mod causation_chain;
mod command_envelope;
pub mod command_history;
pub mod command_metadata;
mod compensation;
mod connector;
pub mod credentials;
#[cfg(test)]
mod credentials_tests;
mod dedupe;
#[cfg(test)]
mod dedupe_tests;
mod dependency_graph_resolver;
pub mod discovery;
mod dual_representation;
#[cfg(test)]
mod dual_representation_tests;
#[cfg(test)]
mod duration_ms_tests;
mod edge_tracking;
pub mod effects;
#[cfg(test)]
mod effects_receipt_tests;
mod encryption;
#[cfg(test)]
mod encryption_tests;
mod errors;
#[cfg(test)]
mod euler_tour_tree;
#[cfg(test)]
mod event_version_tests;
pub mod events;
#[cfg(test)]
mod fence_token_tests;
#[cfg(test)]
mod fire_at_ms_tests;
#[cfg(test)]
mod idempotency_key_tests;
mod identity;
#[cfg(test)]
mod instance_id_tests;
mod instance_status;
pub mod integer_types;
#[cfg(test)]
mod integer_types_kani_proofs;
#[cfg(test)]
mod integer_types_proptests;
#[cfg(test)]
mod integer_types_serde_tests;
#[cfg(test)]
mod integer_types_try_from_tests;
mod lifecycle_superstate;
mod lineage;
mod link_cut_tree;
mod macros;
#[cfg(test)]
mod max_attempts_tests;
pub mod merge_conflict;
mod monoid;
pub mod next_step_selection;
mod node_kind;
#[cfg(test)]
mod node_name_tests;
mod non_empty_vec;
mod octree;
mod pairing_heap;
mod payload_parser;
mod plugin;
#[cfg(feature = "proptest")]
mod proptest_domain_roundtrips;
#[cfg(all(test, feature = "proptest"))]
mod proptest_domain_types;
#[cfg(feature = "proptest")]
mod proptest_generators;
#[cfg(feature = "proptest")]
mod proptest_targets;
pub mod proptest_verifier;
mod recovery_contract;
mod registration_status;
mod rope;
pub mod saga_coordinator;
pub mod search;
#[cfg(test)]
mod sequence_number_tests;
pub mod signal;
pub mod state;
#[cfg(test)]
mod step_id_tests;
#[cfg(test)]
mod step_result;
mod string_types;
#[cfg(test)]
mod string_types_contract_tests;
#[cfg(test)]
mod string_types_proptests;
#[cfg(test)]
mod string_types_serde_tests;
mod task_failure_kind;
mod task_input;
#[cfg(test)]
mod timeout_ms_tests;
#[cfg(test)]
mod timer_id_tests;
#[cfg(test)]
mod timestamp_ms_tests;
mod topology;
mod tx_coordinator;
mod types;
#[cfg(test)]
mod types_tests;
mod workflow;
#[cfg(test)]
mod workflow_name_tests;
mod workload_class;
pub mod workspace;

pub use binomial_heap::BinomialHeap;
pub use blob::{
    BlobFailureAction, BlobGCPolicy, BlobRef, BlobStatus, OutputPolicy, OutputRef,
    INLINED_MAX_BYTES,
};
pub use cartesian_tree::{CartesianNode, CartesianTree, CartesianTreeError};
pub use clique_tree::{Clique, CliqueTree, CliqueTreeError};
pub use causation_chain::{
    advance_chain, validate_chain_depths, CausationArchival, CausationChainError, CausationDepth,
    ChainAdvanceResult, CollapsedLink, DEFAULT_MAX_CAUSATION_DEPTH,
    is_broken_chain_reference, extract_broken_chain_original, validate_causation_reference,
    BrokenChainLink,
};
pub use command_envelope::{CommandEnvelope, CommandEnvelopeError, MAX_SUPPORTED_COMMAND_VERSION};
pub use command_history::{
    BatchId, CommandHistory, CommandHistoryError, CommandKind, ExtensionApplyMode,
    ExtensionBatchMetadata, HistoryEntry, HistoryEntryStatus, SnapshotId, WorkflowSnapshot,
    MAX_HISTORY_DEPTH, MAX_REDO_STACK_DEPTH, MAX_UNDO_STACK_DEPTH,
};
pub use command_metadata::{CommandMetadata, Issuer};
pub use compensation::{
    apply_compensation_transition, CompensationRecord, CompensationStatus,
    CompensationTransitionError, CompensationTransitionEvent,
};
pub use connector::{
    apply_connector_transition, execute_with_reconciliation, reconcile_ambiguous, Connector,
    ConnectorError, ConnectorResult, ConnectorState, ConnectorTransition, ConnectorTransitionError,
    ReconcileAction, ReconciliationResult,
};
pub use credentials::{
    AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus, CredentialVersion,
    CredentialVersionId, Principal, RotationPolicy, RotationState, RotationStatus, SecretValue,
    VaultEntry, VaultEntryId,
};
pub use dedupe::{DedupeKey, DedupePartitionKey, DedupeScope};
pub use dependency_graph_resolver::DependencyGraphResolver;
pub use discovery::{
    enforce_pin, validate_discovery_path, DiscoveryPath, DiscoveryPathError, PinEnforcementError,
    VersionConstraint, VersionPin, VERSION_BASE_PATH,
};
pub use dual_representation::{
    apply_redaction, OperatorProjection, RedactionKind, RedactionPolicy, RedactionRule,
};
pub use edge_tracking::{select_fan_in_source, EdgeTraversalLog, RouterDecision, TraversedEdge};
pub use effects::{
    apply_effect_transition, CompensationPolicy, EffectIntent, EffectKind, EffectRecord,
    EffectTransitionError, EffectTransitionEvent, ExternalReceipt,
};
pub use encryption::{CryptoAlgorithm, DekId, EncryptedBlob, KeyMetadata, WrappedDek};
pub use errors::ParseError;
pub use events::{Error as EventError, EventEnvelope};
pub use identity::{CausationId, CommandId, CorrelationId};
pub use instance_status::InstanceStatus;
pub use lifecycle_superstate::LifecycleSuperstate;
pub use lineage::{Epoch, LineageError, LineageState, LineageStatus, WorkflowLineage};
pub use link_cut_tree::{LctAggregate, LctError, LinkCutTree};
pub use merge_conflict::{
    ConflictClass, ConflictType, ConflictWinner, ErrorCategory, ErrorDetail, FenceConflict,
    LeaseConflict, MergeConflictError, ResolutionResult, ResolutionStrategy, SequenceConflict,
    StateTransitionConflict, UnresolvableReason,
};
pub use monoid::Monoid;
pub use node_kind::NodeKind;
pub use non_empty_vec::NonEmptyVec;
pub use octree::{BoundingBox, Octree, OctreeConfig, OctreeEntry, OctreeError, OctreeNode, Point3};
pub use pairing_heap::{PairingHeap, PairingHeapError};
pub use plugin::{
    apply_plugin_transition, ArtifactRef, CapabilityId, HotLoadEvent, InstanceKey,
    IsolationBreachType, IsolationLevel, PluginArtifact, PluginDescriptor, PluginErrorCategory,
    PluginErrorContext, PluginErrorDetail, PluginFailureContext, PluginHotLoadError, PluginId,
    PluginInstance, PluginName, PluginState, PluginTransition, PluginVersion,
    PluginVersionConstraint, ResourceBudget, SchemaVersion, VersionRange,
};
pub use recovery_contract::{
    classify_expected_outcome, generate_scenario_matrix, violation_to_invariant, AssertionResult,
    CrashTiming, ExpectedRecoveryOutcome, FailoverScenario, FailoverSeverity, RecoveryAssertion,
    RecoveryInvariant, RecoveryPhase, RecoveryViolation,
};
pub use registration_status::RegistrationStatus;
pub use rope::{Measurable, Rope, RopeBuilder, RopeError, RopeSlice};
pub use vo_ds::btree::{BTree, BTreeError};
pub use vo_ds::node::BTreeNode;

pub use saga_coordinator::{
    apply_saga_transition, SagaRecord, SagaState, SagaStep, SagaStepStatus, SagaTransition,
    SagaTransitionError,
};
pub use signal::{
    signal_match, BufferPolicy, FailureScope, LineageScope, SignalAddress, SignalDedupeKey,
    SignalDelivery, SignalMatchResult, WaitKey, WaitRecord,
};
pub use task_failure_kind::TaskFailureKind;
pub use task_input::{TaskInput, TaskInputEnvelope};
pub use topology::{LeaseKey, NodeId};
pub use tx_coordinator::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition,
    CoordinatorTransitionError, ParticipantRecord, ParticipantStatus, ParticipantVote,
    TransactionRecord, TransactionState,
};
pub use types::{
    extract_schema_version, AttemptNumber, BinaryHash, DurationMs, EventVersion, FenceToken,
    FireAtMs, IdempotencyKey, InstanceId, LeaseRecord, MaxAttempts, NodeName, SequenceNumber,
    Snapshot, SpawnId, State, StepId, TimeoutMs, TimerId, TimestampMs, WorkflowName, WorkflowSpec,
    WorkflowVersionHash, MAX_SUPPORTED_SCHEMA_VERSION,
};
pub use workflow::{
    next_nodes, DagNode, Edge, EdgeCondition, GuaranteeClass, RetryPolicy, RetryPolicyError,
    StepOutcome, VersionCompatResult, VersionError, WorkflowDefinition, WorkflowDefinitionError,
    WorkflowVersion,
};
pub use workload_class::{WorkloadClass, WorkloadClassParseError, ALL_WORKLOAD_CLASSES};

#[cfg(kani)]
mod kani_proofs;

#[cfg(test)]
mod adversarial_tests;
// #[cfg(test)]
// mod command_envelope_red_queen_tests;  // removed: directory without mod.rs
#[cfg(test)]
mod compensation_tests;
#[cfg(test)]
mod context_stack_adversarial;
#[cfg(test)]
mod cross_cutting_tests;
#[cfg(test)]
// mod dependency_graph_resolver_tests;  // removed: file not found
#[cfg(test)]
mod identity_bdd_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod red_queen_tests;

#[cfg(test)]
mod schema_version_tests;
#[cfg(test)]
mod serde_tests;
#[cfg(test)]
mod tests_bdd_dag_connectivity;
#[cfg(test)]
mod tests_bdd_dag_cycle_validation;
#[cfg(test)]
mod tests_bdd_dag_merge_point;
#[cfg(test)]
mod workflow_tests;
