mod binomial_heap;
#[cfg(test)]
mod blackhat_encryption_credentials_tests;
mod blob;
#[cfg(test)]
mod blob_tests;
mod btree;
pub mod cartesian_tree;
mod clique_tree;
mod command_envelope;
pub mod command_history;
pub mod command_metadata;
mod compensation;
pub mod connection_pool;
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
pub mod effects;
#[cfg(test)]
mod effects_receipt_tests;
mod encryption;
#[cfg(test)]
mod encryption_tests;
mod errors;
pub mod events;
mod identity;
mod instance_status;
pub mod integer_types;
#[cfg(test)]
mod integer_types_tests;
mod lifecycle_superstate;
mod lineage;
mod link_cut_tree;
mod macros;
pub mod next_step_selection;
mod node_kind;
mod non_empty_vec;
mod octree;
mod pairing_heap;
mod payload_parser;
mod plugin;
#[cfg(feature = "proptest")]
mod proptest_targets;
pub mod proptest_verifier;
mod recovery_contract;
mod registration_status;
mod rope;
pub mod search;
pub mod signal;
pub mod skew_heap;
mod spqr_tree;
pub mod state;
pub mod string_types;
#[cfg(test)]
mod string_types_tests;
mod task_io;
mod topology;
mod tx_coordinator;
mod types;
#[cfg(test)]
mod types_tests;
mod workflow;
pub mod workspace;

pub use binomial_heap::BinomialHeap;
pub use blob::{
    BlobFailureAction, BlobRef, BlobStatus, OutputPolicy, OutputRef, INLINED_MAX_BYTES,
};
pub use btree::{BTree, BTreeError, BTreeNode};
pub use cartesian_tree::{CartesianNode, CartesianTree, CartesianTreeError};
pub use clique_tree::{Clique, CliqueTree, CliqueTreeError};
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
    ConnectorError, ConnectorResult, ConnectorState, ConnectorTransition,
    ConnectorTransitionError, ReconciliationResult, ReconcileAction,
};
pub use credentials::{
    AccessPolicy, Credential, CredentialId, CredentialKind, CredentialStatus, CredentialVersion,
    CredentialVersionId, Principal, RotationPolicy, RotationState, RotationStatus, SecretValue,
    VaultEntry, VaultEntryId,
};
pub use dedupe::{DedupeKey, DedupePartitionKey};
pub use dependency_graph_resolver::DependencyGraphResolver;
pub use discovery::{
    enforce_pin, validate_discovery_path, DiscoveryPath, DiscoveryPathError, PinEnforcementError,
    VersionConstraint, VersionPin, VERSION_BASE_PATH,
};
pub use dual_representation::{
    apply_redaction, OperatorProjection, RedactionKind, RedactionPolicy, RedactionRule,
};
pub use effects::{
    apply_effect_transition, CompensationPolicy, EffectIntent, EffectKind, EffectRecord,
    EffectTransitionError, EffectTransitionEvent,
};
pub use encryption::{CryptoAlgorithm, DekId, EncryptedBlob, KeyMetadata, WrappedDek};
pub use errors::ParseError;
pub use events::{Error as EventError, EventEnvelope, EventMetadata};
pub use identity::{CausationId, CommandId, CorrelationId};
pub use instance_status::InstanceStatus;
pub use lifecycle_superstate::LifecycleSuperstate;
pub use lineage::{Epoch, LineageError, LineageState, LineageStatus, WorkflowLineage};
pub use link_cut_tree::{LctAggregate, LctError, LinkCutTree, Monoid};
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
pub use search::{
    Bm25Scorer, InvertedIndex, Posting, PostingList, Query, QueryParser, Scorer, SearchEngine,
    SearchError, SearchResult, TfIdfScorer,
};
pub use signal::{
    signal_match, BufferPolicy, LineageScope, SignalAddress, SignalDedupeKey, SignalDelivery,
    SignalMatchResult, WaitKey, WaitRecord,
};
pub use skew_heap::{SkewHeap, SkewHeapError, SkewNode};
pub use spqr_tree::{
    Block, Component, CutNode, SPQRDecomposition, SPQREdge, SPQRNode, SPQRNodeType, SpqrError,
    StaticGraph,
};
pub use topology::{LeaseKey, NodeId};
pub use task_io::{TaskFailureKind, TaskInputEnvelope};
pub use tx_coordinator::{
    apply_coordinator_transition, CoordinatorDecision, CoordinatorTransition,
    CoordinatorTransitionError, ParticipantRecord, ParticipantStatus, ParticipantVote,
    TransactionRecord, TransactionState,
};
pub use types::{
    extract_schema_version, AttemptNumber, BinaryHash, DurationMs, EventVersion, FenceToken,
    FireAtMs, IdempotencyKey, InstanceId, LeaseRecord, MaxAttempts, NodeName, SequenceNumber,
    Snapshot, SpawnId, State, StepId, TimeoutMs, TimerId, TimestampMs, WorkflowName, WorkflowSpec,
    MAX_SUPPORTED_SCHEMA_VERSION,
};
// Re-export NamespaceId from vo-common for type safety
pub use vo_common::NamespaceId;
pub use workflow::{
    next_nodes, DagNode, Edge, EdgeCondition, RetryPolicy, RetryPolicyError, StepOutcome,
    WorkflowDefinition, WorkflowDefinitionError,
};

#[cfg(kani)]
mod kani_proofs;

#[cfg(test)]
mod adversarial_tests;
#[cfg(test)]
mod command_envelope_red_queen_tests;
#[cfg(test)]
mod compensation_tests;
#[cfg(test)]
mod context_stack_adversarial;
#[cfg(test)]
mod cross_cutting_tests;
#[cfg(test)]
mod dependency_graph_resolver_tests;
#[cfg(test)]
mod identity_bdd_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
mod red_queen_tests;
#[cfg(test)]
mod schema_evolution_bdd_tests;
#[cfg(test)]
mod schema_version_tests;
#[cfg(test)]
mod serde_tests;
#[cfg(test)]
mod tests_bdd_dag_cycle_validation;
#[cfg(test)]
mod tests_bdd_dag_connectivity;
#[cfg(test)]
mod tests_bdd_dag_merge_point;
#[cfg(test)]
mod workflow_tests;
