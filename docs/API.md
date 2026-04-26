# Veloxide API Reference

> Generated documentation for all public interfaces across 14 crates.

## Crate Overview

| Crate | Description |
|-------|-------------|
| `vo-types` | Shared domain types, state machines, and workflow definitions |
| `vo-common` | Common utilities, type aliases, and telemetry stubs |
| `vo-core` | Core engine: replay, admission, circuit breaker, vault, upcaster |
| `vo-api` | HTTP REST API (axum) for workflow management and queries |
| `vo-ipc` | Inter-process communication: FD3/FD4 envelope protocol, subprocess execution |
| `vo-sdk` | Thin zero-panic SDK for task binaries: FD3 read, FD4 write, DAG builder |
| `vo-actor` | Actor framework: lifecycle, message routing, timers, spawn supervision |
| `vo-storage` | Persistence layer backed by Fjall: events, snapshots, blobs, leases |
| `vo-executor` | Step execution with timeout, retry, and scheduling |
| `vo-worker` | Distributed lock manager and connector runtime |
| `vo-cli` | Operator CLI (`vo` command) |
| `vo-frontend` | Dioxus web UI for workflow visualization |
| `vo-linter` | Static analysis: lint rules for workflow definitions |
| `vo-sdk-macros` | Procedural macro: `#[task_macro]` for task binary entrypoints |

---

## vo-types

Shared domain types, state machines, and workflow definitions.

### String Newtypes

Validated wrapper types that enforce format constraints at parse time.

| Type | Max Length | Validation |
|------|-----------|------------|
| `InstanceId` | 26 | ULID (non-nil) |
| `WorkflowName` | 128 | `[a-z][a-z0-9_]*`, no consecutive `--`/`__`/`-_` |
| `NodeName` | 128 | Same as `WorkflowName` |
| `BinaryHash` | min 8 | Lowercase hex, even length |
| `TimerId` | 256 | Non-empty |
| `IdempotencyKey` | 1024 | Non-empty |
| `SpawnId` | -- | Identifier chars, boundary rules |
| `StepId` | -- | Identifier chars, no leading `_` |
| `CredentialId` | 26 | ULID |
| `CredentialVersionId` | 26 | ULID |
| `VaultEntryId` | 26 | ULID |
| `DekId` | 26 | ULID (non-nil) |
| `DedupeKey` | 256 | Non-empty |
| `WaitKey` | 256 | Non-empty |

All implement `Display`, `TryFrom<String>`, `From<T> for String`, `Serialize`, `Deserialize`, and have `parse(&str)` / `as_str()` methods.

### Integer Newtypes

| Type | Inner | Methods |
|------|-------|---------|
| `SequenceNumber` | `NonZeroU64` | -- |
| `EventVersion` | `NonZeroU64` | -- |
| `AttemptNumber` | `NonZeroU64` | -- |
| `TimeoutMs` | `NonZeroU64` | `to_duration()` |
| `MaxAttempts` | `NonZeroU64` | `is_exhausted(attempt)` |
| `FenceToken` | `NonZeroU64` | `next()` |
| `DurationMs` | `u64` | `to_duration()` |
| `TimestampMs` | `u64` | `to_system_time()`, `now()` |
| `FireAtMs` | `u64` | `to_system_time()`, `has_elapsed(now)` |

### Constants

| Constant | Value |
|----------|-------|
| `MAX_SUPPORTED_SCHEMA_VERSION` | `1` |
| `INLINED_MAX_BYTES` | `4096` |
| `MAX_SUPPORTED_COMMAND_VERSION` | `1` |
| `MAX_HISTORY_DEPTH` | `100` |
| `MAX_UNDO_STACK_DEPTH` | `50` |
| `MAX_REDO_STACK_DEPTH` | `50` |

### Workflow Definition

```rust
pub struct WorkflowDefinition {
    pub workflow_name: WorkflowName,
    pub nodes: NonEmptyVec<DagNode>,
    pub edges: Vec<Edge>,
}

pub struct DagNode {
    pub node_name: NodeName,
    pub retry_policy: RetryPolicy,
}

pub struct Edge {
    pub source_node: NodeName,
    pub target_node: NodeName,
    pub condition: EdgeCondition,
}

pub struct RetryPolicy {
    pub max_attempts: u8,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
}
```

`WorkflowDefinition::from_deserializer(deserializer)` deserializes from any `serde::Deserializer`.

### Node Kinds

```rust
pub enum NodeKind { Pure, ManagedEffect, Wait, Signal, Unsafe }
```

### Instance Lifecycle State Machine

```rust
pub enum LifecycleState {
    Pending, RunningDecision, StepScheduled, StepExecuting,
    WaitingForTimer, Completed, Failed, Cancelled,
}
```

**Transitions**: `AssignToNode`, `Cancel`, `StepScheduled`, `Fail`, `ExecuteStep`, `WaitForTimer`, `CompleteStep`, `TimerFired`, `TimerExpired`, `InstanceResumed`.

**Functions**: `apply(state, event)`, `get_operational_status(state)`, `is_terminal(state)`, `get_valid_transitions(state)`.

**Operational status**:
```rust
pub enum OperationalStatus { Healthy, Blocked(BlockedReason), Recovering }
pub enum BlockedReason { DependenciesPending, ResourceContention, ManualHold }
```

**Lifecycle superstates**:
```rust
pub enum LifecycleSuperstate { Active, Suspended, Recovering, Compensating, Terminal }
```

### Instance Status

```rust
#[repr(u8)]
pub enum InstanceStatus {
    Pending = 0x01, Running = 0x02, Paused = 0x03,
    Completed = 0x04, Failed = 0x05, Cancelled = 0x06,
}
```

### Registration Status

```rust
pub enum RegistrationStatus { Active, Deactivated, Quarantined }
```

### Event System

**Event envelope**:
```rust
pub struct EventEnvelope {
    pub schema_version: u8,
    pub instance_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
}
```

**Event payloads**:
```rust
pub enum EventPayload {
    WorkflowStarted { workflow_id, dag_topology, binary_hash, workflow_version_hash, dedupe_key_hash },
    WorkflowCompleted { workflow_id, completion_time_ms },
    WorkflowFailed { workflow_id, failure_reason },
    WorkflowCancelled { workflow_id, cancelled_by },
    StepScheduled { workflow_id, step_id, attempt, fence, execution_id },
    StepStarted { workflow_id, step_id, started_at_ms },
    StepCompleted { workflow_id, step_id, completed_at_ms, attempt, fence, routing_projection, output_ref, output_hash, output },
    StepFailed { workflow_id, step_id, failure_reason, attempt, fence },
    EffectPrepared { workflow_id, step_id, effect_id, sink_kind, payload_hash, fence },
    EffectCommitted { workflow_id, step_id, effect_id, external_receipt, fence },
    TimerSet { workflow_id, timer_id, fire_at_ms },
    TimerFired { workflow_id, timer_id, fired_at_ms },
    CancelRequested { workflow_id, requested_by },
    InstanceResumed { workflow_id, resumed_at_ms },
    ContinuedAsNew { workflow_id, lineage_id, old_epoch, new_epoch },
}
```

**Decode**: `decode_event(input: &[u8]) -> Result<(EventEnvelope, EventPayload), Error>`

**Upcasting**:
```rust
pub trait Upcaster: Send + Sync {
    fn source_version(&self) -> u8;
    fn target_version(&self) -> u8;
    fn upcast(&self, payload: &serde_json::Value) -> Result<serde_json::Value, UpcasterError>;
}
pub struct VersionRegistry { ... }
```

### Command Envelope

```rust
pub struct CommandEnvelope {
    pub schema_version: u8,
    pub metadata: CommandMetadata,
}

pub struct CommandMetadata {
    pub command_id: IdempotencyKey,
    pub correlation_id: IdempotencyKey,
    pub causation_id: IdempotencyKey,
    pub issuer: Issuer,
    pub issued_at: TimestampMs,
}

pub enum Issuer { System, ApiClient, Operator, AiAgent, TimerLoop, RecoveryLoop }
```

`CommandEnvelope::from_bytes()` / `from_str()` / `is_supported()`.

### Command History (Undo/Redo)

```rust
pub struct CommandHistory { ... }
```

Methods: `new()`, `capacity()`, `can_undo()`, `can_redo()`, `entries()`, `undo()`, `redo()`, `apply_command()`.

```rust
pub enum CommandKind {
    ExtensionApply, ExtensionRevert, ExtensionRedo,
    NodeCreate, NodeDelete, EdgeCreate, EdgeDelete, ConfigUpdate,
}

pub enum HistoryEntryStatus { Committed, Undone, Redone, Failed }
```

### Signal System

```rust
pub struct SignalAddress { ... }  // lineage_wide() or epoch_local()
pub struct WaitRecord { instance_id, wait_key, buffer_policy, registered_at }
pub struct SignalDedupeKey { lineage_id, wait_key, command_id }

pub enum BufferPolicy { Reject, BufferOne, BufferMany }
pub enum LineageScope { EpochLocal, LineageWide }
pub enum SignalDelivery { Accepted, Rejected, Buffered }
pub enum SignalMatchResult { Matched, LineageMismatch, InstanceMismatch, WaitKeyMismatch, EpochMismatch, EpochNotSpecified }
```

`signal_match(signal, wait, wait_instance_lineage_id)` returns match result.

### Effects and Compensation

```rust
pub enum EffectIntent { Prepared, Committed, RolledBack }
pub enum EffectKind { HttpCall, SqlQuery, BlobWrite }
pub enum CompensationPolicy { None, Manual, Automatic }

pub struct EffectRecord { intent_id, kind, params_json, status, committed_at }
pub struct CompensationRecord { effect_id, policy, status, compensation_effect_id, started_at, completed_at }
```

`apply_effect_transition()`, `apply_compensation_transition()`.

### Connector State Machine

```rust
pub enum ConnectorState { Idle, Preparing, Prepared, Executing, Succeeded, Failed, Ambiguous }
pub enum ConnectorResult { Success, Failure, Ambiguous }
pub enum ReconcileAction { Commit, Rollback, Retry }
```

`apply_connector_transition()`, `reconcile_ambiguous()`.

### Transaction Coordinator

```rust
pub enum TransactionState {
    Init, Enrolling, Preparing, Prepared, Committing, Committed,
    RollingBack, RolledBack, Aborted, Ambiguous,
}
pub enum ParticipantStatus { Enrolled, Prepared, VotedRollback, Committed, RolledBack, Unknown }
pub enum CoordinatorDecision { Commit, Rollback }
```

`apply_coordinator_transition()`.

### Connector Trait

```rust
pub trait Connector: Send + Sync {
    fn prepare(&mut self) -> impl Future<Output = Result<ConnectorResult, ConnectorError>> + Send;
    fn commit(&mut self) -> impl Future<Output = Result<ConnectorResult, ConnectorError>> + Send;
    fn reconcile(&mut self) -> impl Future<Output = Result<ReconciliationResult, ConnectorError>> + Send;
    fn rollback(&mut self) -> impl Future<Output = Result<ConnectorResult, ConnectorError>> + Send;
}
```

### Credentials and Vault

```rust
pub enum CredentialKind { ApiKey, Password, Token, Certificate, SigningKey, EncryptionKey, Custom(String) }
pub enum CredentialStatus { Active, Rotating, Expired, Revoked, Superseded }
pub enum RotationPolicy { Manual, TimeBased { ... }, UsageBased { ... }, EventBased { ... } }
pub enum Principal { User(InstanceId), Actor(SpawnId), Workflow(WorkflowName), System }
```

### Encryption

```rust
pub enum CryptoAlgorithm { Aes256Gcm }  // IV_SIZE=12, TAG_SIZE=16, KEY_SIZE=32
pub struct DekId(String);       // parse, from_bytes, to_bytes
pub struct WrappedDek(pub Vec<u8>);
pub struct EncryptedBlob { pub iv, pub ciphertext, pub tag }
pub struct KeyMetadata { pub created_at_ms, pub algorithm, pub instance_id }
```

### Blob System

```rust
pub enum BlobStatus { Pending, DurablyStored, Published, Failed }
pub enum OutputPolicy { Required, Optional }
pub enum BlobFailureAction { BlockStep, CompleteWithInline }
pub enum OutputRef { Inline(Vec<u8>), BlobRef(BlobRef) }

pub struct BlobRef { pub blob_id, pub size_bytes, pub content_hash }
```

### Dual Representation (GDPR)

```rust
pub enum RedactionKind { Remove, ReplaceWith(String), ReplaceWithType, Hash }

pub struct RedactionPolicy { pub workflow_type, pub redaction_rules: Vec<RedactionRule> }
pub struct RedactionRule { pub field_path: Vec<String>, pub redaction_kind: RedactionKind }
pub struct OperatorProjection { pub workflow_id, pub workflow_type, pub projection_json, pub redacted_fields }
```

`apply_redaction(value, rules) -> (Value, Vec<Vec<String>>)`.

### Lineage and Continue-as-New

```rust
pub struct Epoch(pub u64);  // ZERO = Epoch(0)
pub struct WorkflowLineage { pub lineage_id, pub epoch, pub parent_epoch: Option<Epoch> }
```

`WorkflowLineage::new(lineage_id)`, `with_parent()`, `continue_as_new()`.

### Dependency Graph

```rust
pub struct DependencyGraphResolver;
```

Static methods: `dependencies()`, `dependents()`, `transitive_dependencies()`, `transitive_dependents()`, `ready_nodes()`, `execution_layers()`.

### Data Structures (re-exported)

Cartesian tree, Euler tour tree, link-cut tree, skew heap, octree, SPQR decomposition, junction tree, clique tree, rope, binomial heap, pairing heap, treap, and non-empty vec.

---

## vo-common

Common utilities and shared types.

### Type Aliases

```rust
pub type InstanceId = String;
pub type NamespaceId = String;
pub type TimerId = String;
pub type VoError = String;
```

### Events

```rust
pub enum WorkflowEvent {
    TimerFired { timer_id: String, timestamp_ms: u64 },
}
```

### Telemetry (Stub)

The `telemetry` module declares submodules (`metrics`, `traces`, `export`) and types (`TelemetryMetrics`, `TelemetryTracer`, `TelemetryExporter`, `OtlpEndpoint`, `TelemetryConfig`) but the submodule files do not exist. The `TelemetryState` struct is declared but non-compilable.

---

## vo-core

Core engine: replay, admission control, circuit breaker, vault, upcaster, and data structures.

### Admission Control (`admission`)

```rust
pub struct AdmissionThresholds { ... }
pub enum AdmissionResult { Admitted, Rejected { reason: RejectionReason } }
pub enum RejectionReason { ... }
pub struct DedupeToken { ... }
pub trait AdmissionCheck: Send + Sync { ... }
```

### Circuit Breaker (`circuit_breaker`)

```rust
pub struct CircuitBreakerConfig { ... }
pub enum CircuitBreakerState { Closed, Open, HalfOpen }
pub enum CircuitBreakerError { ... }
pub fn evaluate_registration(request) -> RegistrationOutcome;
```

### Config Hot Reload (`config_hot_reload`)

```rust
pub trait ConfigValidator<T>: Send + Sync { ... }
pub struct HotReloadConfig<T> { ... }
```

### Exact-Once Verification (`exact_once_verification`)

```rust
pub enum CrashPoint { 12 variants }
pub enum CrashPosition { Before, After }
pub struct CrashScenario { ... }
pub struct VerificationHarness { ... }
pub struct RecoveryContext { ... }
```

Assertion functions: `assert_fence_token_ordering()`, `assert_invariant_no_orphans()`, `assert_no_duplicate_effects()`.

### Replay Engine (`replay`)

```rust
pub struct ReplayEngine { ... }
pub enum ReplayError { ... }
pub struct ReplayResult { ... }
```

### Resource Quota (`resource_quota`)

```rust
pub enum ResourceKind { Cpu, Memory, Disk }
pub struct NamespaceQuota { ... }
pub struct QuotaEnforcer { ... }
```

### Upcaster (`upcaster`)

```rust
pub trait Upcaster { fn source_version(&self) -> u8; fn upcast(&self, payload) -> Result<...>; }
pub struct UpcasterRegistry { ... }
pub struct UpcasterRegistryBuilder { ... }
```

### Vault (`vault`)

```rust
pub struct CredentialVault { ... }
pub enum CredentialError { ... }
pub enum Permission { Read, Write, Delete, Rotate, Revoke }
```

### Workflow Version (`workflow_version`)

```rust
pub struct WorkflowVersion { ... }
pub enum WorkflowVersionError { ... }
```

### Workload and Write Classes

```rust
// workload_class
pub enum WorkloadClass { ExactCritical, Standard, Recovery, UnsafeBulk }
pub struct WorkloadBudget { ... }

// write_class
pub enum WriteClass { CriticalControlPlane, OperatorProjection, BulkBlob }
pub struct WriteBudget { ... }
```

### Workspace Swap (`workspace_swap`)

```rust
pub enum SwapPhase { ... }
pub enum SwapStatus { ... }
pub struct AtomicWorkspaceSwap { ... }
```

### Data Structures

`Quadtree`, `SegmentTree<T>`.

---

## vo-api

HTTP REST API (axum) for workflow management and queries.

### REST Endpoints

#### Active Endpoints

| Method | Route | Handler | Response |
|--------|-------|---------|----------|
| `GET` | `/api/v1/workflows/:id/timeline` | `get_timeline` | `TimelineResponse` |
| `GET` | `/api/v1/workflows/:id/history` | `get_history` | `HistoryResponse` |
| `GET` | `/api/v1/workflows/:id/effect-journal` | `get_effect_journal` | `EffectJournalResponse` |
| `GET` | `/api/v1/workflows/:id/version` | `get_workflow_version` | `WorkflowVersionResponse` |

All query endpoints require path format `<namespace>/<instance_id>` and return 400 if malformed.

#### Planned Endpoints (commented out)

| Method | Route | Handler | Response |
|--------|-------|---------|----------|
| `POST` | `/api/v1/workflows` | `start_workflow` | `V3StartResponse` (201) |
| `GET` | `/api/v1/workflows` | `list_workflows` | `Vec<V3StatusResponse>` (200) |
| `GET` | `/api/v1/workflows/:id` | `get_workflow` | `V3StatusResponse` (200) |
| `DELETE` | `/api/v1/workflows/:id` | `terminate_workflow` | 204 |
| `POST` | `/api/v1/workflows/:id/signals` | `send_signal` | 202 |
| `GET` | `/api/v1/workflows/:id/events` | `get_events` | 501 (stub) |
| `GET` | `/api/v1/watch/:instance_id` | `watch_workflow` | SSE stream |

### Request/Response Types

#### V3 API (Current)

```rust
pub struct V3StartRequest {
    pub namespace: String,
    pub workflow_type: String,
    pub paradigm: String,         // "fsm", "dag", or "procedural"
    pub input: serde_json::Value,
    pub instance_id: Option<String>,
    pub dedupe_key: Option<String>,
}

pub struct V3StartResponse { pub instance_id, pub namespace, pub workflow_type }
pub struct V3StatusResponse {
    pub instance_id, pub namespace, pub workflow_type, pub paradigm,
    pub phase,                    // "replay" or "live"
    pub events_applied: u64,
}
pub struct V3SignalRequest { pub signal_name: String, pub payload: serde_json::Value }
pub struct ApiError { pub error: String, pub message: String }
```

#### Query Responses

```rust
pub struct TimelineEntry { pub sequence: u64, pub timestamp_ms: u64, pub event_type: String, pub payload: serde_json::Value }
pub struct TimelineResponse { pub instance_id: String, pub entries: Vec<TimelineEntry>, pub total_replayed: usize }

pub struct HistoryEntry { pub sequence, pub timestamp_ms, pub event_type, pub step_id: Option, pub error: Option, pub output: Option }
pub struct HistoryResponse { pub instance_id: String, pub entries: Vec<HistoryEntry> }

pub struct EffectJournalEntry { pub sequence, pub timestamp_ms, pub event_type, pub semantics: EffectSemantics, pub payload }
pub enum EffectSemantics { Exact, Unsafe }
pub struct EffectJournalResponse { pub instance_id, pub entries: Vec<EffectJournalEntry> }

pub struct WorkflowVersionResponse { pub instance_id, pub schema_version: u8, pub event_count: u64, pub last_sequence: Option, pub last_timestamp_ms: Option }
```

#### V1 API (Legacy)

```rust
pub struct StartWorkflowRequest { pub workflow_name: WorkflowName, pub input: serde_json::Value }
pub struct StartWorkflowResponse { pub invocation_id: InvocationId, pub workflow_name, pub status, pub started_at }
pub struct WorkflowStatus { pub invocation_id, pub workflow_name, pub status, pub current_step: u32, pub started_at, pub updated_at }
pub struct JournalEntry { pub seq: u32, pub entry_type: JournalEntryType, pub name, pub input, pub output, pub timestamp, pub duration_ms, pub fire_at, pub status }
pub struct ErrorResponse { pub error, pub message, pub retry_after_seconds: Option<RetryAfterSeconds> }
```

### Named Type Wrappers

```rust
pub struct WorkflowName(String);     // validates ^[a-z][a-z0-9_]*$
pub struct SignalName(String);       // validates ^[a-z][a-z0-9_]+$
pub struct InvocationId(String);     // validates 26-char Crockford base32 ULID
pub struct RetryAfterSeconds(NonZeroU64);
pub struct Timestamp(String);        // validates RFC3339
```

### Error Taxonomy

```rust
pub enum ParseError { EmptyWorkflowName, InvalidWorkflowNameFormat, EmptySignalName, ... }
pub enum ValidationError { InvalidRetryAfterSeconds, InvalidStatusTransition, ... }
pub enum InvariantViolation { UpdatedBeforeStarted, EntriesNotSorted, ... }
```

### SSE Types (commented out)

```rust
pub enum WorkflowSseEvent { StepCompleted, StepFailed, TimerFired, SignalReceived, PhaseChanged, InstanceCompleted, InstanceFailed }
pub struct SseBroadcaster { ... }  // capacity 1000, 15s keepalive
```

### Query State

```rust
pub struct QueryState { pub keyspace: Arc<fjall::Keyspace> }
```

---

## vo-ipc

Inter-process communication via FD3/FD4 envelope protocol.

### Protocol

The engine communicates with task binaries via two file descriptors:
- **FD3** (Engine -> Child): Input envelope with task data
- **FD4** (Child -> Engine): Result envelope with success/failure

### Constants

| Constant | Value |
|----------|-------|
| `MAX_PAYLOAD_SIZE` | 10,485,760 (10 MB) |
| `MAX_STDERR_BYTES` | 1,048,576 (1 MB) |
| `TRUNCATION_MARKER` | `"\n[... TRUNCATED AT 1MB ...]"` |

### Envelopes

```rust
pub struct Fd3Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub input: serde_json::Value,
    pub secrets: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
}

pub struct Fd4Envelope {
    pub version: u8,
    pub instance_id: String,
    pub node_id: String,
    pub result: TaskResult,
}

pub enum TaskResult {
    Success { output: serde_json::Value },
    Failure { error: TaskError },
}

pub struct TaskError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}
```

### Envelope Functions

```rust
pub fn write_envelope<T: Serialize>(writer, envelope) -> Result<(), IpcError>
pub fn read_envelope<T: DeserializeOwned>(reader) -> Result<T, IpcError>
pub fn engine_receive_envelope(reader, expected_instance_id, expected_node_id) -> Result<Fd4Envelope, IpcError>
pub fn validate_identity(envelope, expected_instance_id, expected_node_id) -> Result<(), IpcError>
```

### Subprocess Execution

```rust
pub struct SubprocessConfig { executable_path, timeout_ms, fd3_payload }
pub struct SubprocessOutput { pub fd4_bytes, pub stderr_bytes, pub stderr_truncated }

pub async fn run_subprocess(config: SubprocessConfig) -> Result<SubprocessOutput, IpcError>
```

### SPSC Queue

Lock-free single-producer single-consumer queue for internal message passing.

```rust
pub struct SpscQueue<T> { ... }
pub struct Sender<T> { ... }
pub struct Receiver<T> { ... }
pub enum SpscError { Full, Empty }
```

`SpscQueue::new(capacity)`, `sender() -> (Sender, Receiver)`, `send()`, `recv()`.

### Stderr Capture

```rust
pub struct StderrCapture { pub bytes: Vec<u8>, pub truncated: bool, pub observed_bytes: usize }
pub fn update_capture(current, chunk) -> StderrCapture
pub fn finalize_capture(capture) -> StderrCapture
pub async fn read_bounded_stderr<R: AsyncRead + Unpin>(reader) -> io::Result<StderrCapture>
```

### Error Types

```rust
pub enum ConfigError { TimeoutMustBePositive, ProgramMissing, ProgramNotExecutable }
pub enum IpcError {
    Config, PipeSetupFailed, SpawnFailed, WaitFailed, Fd4ReadFailed, Fd3WriteFailed,
    StderrReadFailed, SignalFailed, Timeout { elapsed_ms, stderr_bytes, stderr_truncated },
    ProcessFailed { exit_code, stderr_bytes, stderr_truncated },
    PayloadTooLarge(u32), IncompleteRead, InvalidJson, VersionMismatch,
    SchemaViolation, IdentityMismatch { ... }, IoError,
}
```

---

## vo-sdk

Thin, zero-panic library for task binaries.

### Core Functions

```rust
pub fn read_input() -> Result<TaskInput, SdkError>
pub fn write_success(output: &Value) -> Result<(), SdkError>
pub fn write_failure(kind: TaskFailureKind, message: &str) -> Result<(), SdkError>
```

- `read_input()`: Reads from FD3, parses `{idempotency_key, data}`, 10 MB limit, read-once guard.
- `write_success()`: Writes `{"status":"success","output":...}` to FD4, 10 MB limit, write-once guard.
- `write_failure()`: Writes `{"status":"failure","kind":"<User|System|Timeout>","message":"..."}`, message limit 1024 bytes.

### Task I/O Types

```rust
pub struct TaskInput { pub idempotency_key: IdempotencyKey, pub data: serde_json::Value }
pub enum TaskFailureKind { User, System, Timeout }
pub enum SdkError { InvalidInput, FdNotOpen, AlreadyWritten, WriteError }
```

### Workflow Builder (Fluent API)

```rust
let mut wf = Workflow::new("checkout_flow");
let validate = wf.pure("validate", |input: Cart| -> ValidatedCart { ... })?;
let charge = wf.effect("charge", |input: ValidatedCart| -> Receipt { ... })?;
wf.connect(&validate, &charge)?;
let spec = wf.build()?;
```

```rust
pub struct Workflow { ... }
```

| Method | Description |
|--------|-------------|
| `new(name)` | Create workflow |
| `pure(name, fn)` | Add pure (side-effect-free) node |
| `effect(name, fn)` | Add managed-effect node |
| `wait(name, fn)` | Add wait node |
| `signal(name, fn)` | Add signal node |
| `unsafe_node(name, fn)` | Add unsafe node |
| `connect(from, to)` | Connect nodes (compile-time type safety) |
| `build()` | Build `WorkflowSpec` |

### DAG Builder (Low-Level)

```rust
pub struct Dag { ... }
```

| Method | Description |
|--------|-------------|
| `add_node_with_kind(name, kind, fn)` | Register node with kind |
| `connect(from, to)` | Connect with type safety |
| `build(workflow_name)` | Build `WorkflowSpec` |

### Node Handle

```rust
pub struct NodeHandle<I, O> { ... }
```

Typed handle enabling compile-time edge type checking. Methods: `new(name)`, `name()`, `node_name()`.

### Graph Args (`--graph` Protocol)

```rust
pub struct WorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
}

pub struct NodeSpec { pub name: NodeName, pub kind: NodeKind }
pub struct EdgeSpec { pub from: NodeName, pub to: NodeName }
```

```rust
pub fn parse_graph_args(args: &[String]) -> Result<GraphArgs, GraphArgsError>
pub fn emit_graph_if_requested(args: &[String], spec: &WorkflowSpec) -> Result<(), ()>
```

### Error Types

```rust
pub enum DagError { InvalidNodeName, NodeNotFound, EmptyWorkflow, CycleDetected }
pub enum GraphArgsError { UnrecognizedArgument, NoGraphFlag }
```

---

## vo-actor

Actor framework built on ractor.

### Actor Messages

```rust
pub enum InstanceActorMessage {
    StartWorkflow { instance_id, workflow_name, node_name },
    StepCompleted { instance_id, node_name, sequence },
    StepFailed { instance_id, node_name, sequence, error },
    TimerFired { instance_id, timer_id },
    CancelRequested { instance_id },
    GetStatus { instance_id },
}

pub enum ControlActorMessage {
    Cancel { instance_id },
    Resume { instance_id },
    AcceptAndResume { instance_id, wait_key, signal_id, payload },
}
```

### Control Actor

```rust
pub struct ControlActor { ... }
```

| Method | Returns |
|--------|---------|
| `new()` | Self |
| `with_storage_and_queue(storage, queue)` | Self |
| `handle_cancel(instance_id)` | `Result<(CancelRequested, WorkflowCancelled), CancelError>` |
| `handle_resume(instance_id)` | `Result<InstanceResumed, ResumeError>` |
| `accept_and_resume(instance_id, wait_key, signal_id, payload)` | `Result<AcceptResumeOutcome, AcceptResumeError>` |

### Signal Types

```rust
pub struct WaitKey(String);          // parse, as_str
pub struct SignalPayload(Vec<u8>);   // from_bytes, empty, as_bytes
pub struct SignalAccepted { pub instance_id, pub wait_key, pub signal_id, pub payload, pub accepted_at }
pub struct AcceptResumeOutcome { pub accepted: SignalAccepted, pub resumed: InstanceResumed }
```

### Error Taxonomy

```rust
pub enum CancelError { AlreadyTerminal, InstanceActorNotFound, LockAcquisitionFailed, StorageError }
pub enum ResumeError { InvalidLifecycleState, MissingSecrets, NodeNotFound, NoPathToTerminal, ... }
pub enum AcceptResumeError { InvalidLifecycleState, WaitKeyMismatch, InstanceActorNotFound, PayloadTooLarge, ... }
```

`is_precondition()` and `is_transient()` methods on error types.

### Signal Storage Traits

```rust
pub trait SignalStorage: Send + Sync {
    fn persist_signal_accepted(&self, accepted: &SignalAccepted) -> Result<(), SignalStorageError>;
    fn remove_signal_accepted(&self, instance_id, signal_id) -> Result<(), SignalStorageError>;
}

pub trait SignalWorkQueue: Send + Sync {
    fn enqueue_resume(&self, instance_id: InstanceId) -> Result<(), SignalWorkQueueError>;
}
```

### Hierarchical Lifecycle (ADR-039)

```rust
pub enum ActorLifecycleState { Pending, Running, Stopping, Stopped, Failed }
pub enum LifecycleTransition { Start, Stop, ChildStopped, AllChildrenStopped, Fail }

pub struct ParentChildRegistry { ... }
pub struct ShutdownPropagator { graceful_timeout, force_kill_timeout }
pub enum ShutdownResult { Success, ChildrenRunning, Timeout }
```

### Health Check Probes

```rust
pub enum ProbeType { Http, Tcp, Exec }
pub enum ProbeStatus { Healthy, Unhealthy, Unknown }

pub trait Probe: Send + Sync {
    async fn check(&self) -> Result<ProbeResult, ProbeError>;
    fn probe_id(&self) -> ProbeId;
}

pub struct HttpProbe { ... }   // implements Probe
pub struct TcpProbe { ... }    // implements Probe
pub struct ExecProbe { ... }   // implements Probe

pub struct ProbeRegistry { ... }       // register, unregister, get, list
pub struct AggregatedStatus { ... }    // update, is_healthy
pub struct ProbeDefinition { id, name, config, interval, backoff, failure_threshold, success_threshold }
pub struct BackoffConfig { initial_interval, max_interval, multiplier, max_failures }
pub enum ProbeConfig { Http { url, ... }, Tcp { address, ... }, Exec { command, ... } }
```

### Execution Semaphore

```rust
pub struct ExecutionSemaphore { ... }
pub struct SemaphoreConfig { max_concurrent_binaries, max_waiters_for_shed, max_per_workflow, acquire_timeout }
pub enum BackpressureStatus { Healthy, Moderate, Heavy, ShedLoad }
pub enum AdmissionDecision { Admitted, Queued { position, estimated_wait_ms }, Rejected { reason, retry_after_secs } }
```

### Signal Buffer (ADR-042)

```rust
pub struct SignalBuffer { ... }
pub struct SignalBufferConfig { pub max_buffered_per_key }
pub enum BufferResult { Buffered, Rejected, Dropped }
pub struct BufferedSignal { pub signal_id, pub payload, pub buffered_at }
```

### Spawn Supervisor

```rust
pub struct SpawnSupervisor { ... }
pub struct SpawnRecord { pub spawn_id, pub instance_id, pub command, pub spawn_phase, ... }
pub enum SpawnPhase { Spawn, HealthCheck, Running, Shutdown, Terminated, Failed }

pub trait SpawnStorage: Send + Sync { ... }
pub trait ProcessManager: Send + Sync { ... }
pub trait WorkQueue: Send + Sync { ... }
```

### Timer Supervisor

```rust
pub struct TimerSupervisor { ... }
pub struct TimerRecord { pub timer_id, pub instance_id, pub fire_at_ms, ... }

pub trait TimerStorage: Send + Sync {
    fn scan_due_timers(&self, from, to, max) -> Vec<TimerRecord>;
    fn delete_timer(&self, instance_id, fire_at_ms) -> Result<(), TimerSupervisorError>;
}
```

### Message Router

```rust
pub struct MessageRouter { ... }
pub struct ChannelId(String);
pub struct RouterConfig { max_destinations_per_channel, max_dlq_size, delivery_timeout, broadcast_enabled }

pub trait MessageRouterPort: Send + Sync {
    async fn register_channel(&self, ...) -> Result<(), RouteError>;
    async fn route<T>(&self, channel_id, message) -> Result<(), RouteError>;
    // ... 17 total methods
}

pub struct DeadLetterQueue { ... }
pub enum RouteError { ChannelNotFound, NoActiveDestinations, DeliveryTimeout, ... }
```

### Reanimator (Timer Recovery)

```rust
pub struct ReanimatorLoop { ... }
pub struct ReanimatorHandle { ... }
pub struct ReanimatorConfig { scan_interval, max_timers_per_cycle, max_concurrent_resumes, shutdown_timeout }
pub struct FairnessBudget { max_per_instance, max_per_workflow }
```

### Instance Registry

```rust
pub struct InstanceRegistry { ... }
pub trait InstanceRegistryInterface: Send + Sync {
    fn is_active(&self, instance_id: &InstanceId) -> bool;
    fn active_count(&self) -> usize;
}
```

### Workload Classification (ADR-033)

```rust
pub enum WorkloadClass { Recovery, NewInstance, Internal }
pub struct ReservedPermitBudget { ... }
pub enum StartError { BudgetExhaustion, InvalidConfig }
```

---

## vo-storage

Persistence layer backed by Fjall (LSM-tree KV store).

### Partition Constants

| Constant | Value |
|----------|-------|
| `EVENTS_PARTITION` | `"events"` |
| `INSTANCES_PARTITION` | `"instances"` |
| `TIMERS_PARTITION` | `"timers"` |
| `SNAPSHOTS_PARTITION` | `"snapshots"` |
| `WORKFLOW_VERSIONS_PARTITION` | `"workflow_versions"` |
| `PAYLOAD_BLOBS_PARTITION` | `"payload_blobs"` |
| `BLOB_RECORDS_PARTITION` | `"blob_records"` |
| `BLOB_PACK_INDEX_PARTITION` | `"blob_pack_index"` |

### Event Storage

```rust
pub fn append_event<E: Serialize>(namespace: &str, instance_id: &str, event: E) -> Result<(), String>
pub fn query_events(instance_id: &str) -> Vec<(u64, serde_json::Value)>
```

`append_event` persists events to an in-memory event log with sequence validation. `query_events` retrieves stored events by instance ID.

### Codec

```rust
pub fn encode_event_key(namespace, instance_id, sequence) -> Vec<u8>
pub fn decode_event_key(key) -> Result<(String, InstanceId, SequenceNumber), StorageError>
pub fn encode_event_value(schema_version, payload) -> Vec<u8>
pub fn decode_event_value(bytes) -> Result<(u8, serde_json::Value), StorageError>
```

### Blob Store

```rust
pub trait BlobStore: Send + Sync { ... }
pub trait BlobStoreReader: Send + Sync { ... }
pub struct ContentAddress { ... }
pub struct BlobRecord { ... }
pub enum BlobStoreError { ... }
```

### Crypto

```rust
pub fn encrypt(plaintext, dek) -> Result<EncryptedBlob, CryptoError>
pub fn decrypt(blob, dek) -> Result<Vec<u8>, CryptoError>
pub fn wrap_key(dek, kek) -> Result<WrappedDek, CryptoError>
pub fn unwrap_key(wrapped, kek) -> Result<Dek, CryptoError>
```

### Dedupe Partition

```rust
pub struct DedupePartition { ... }
pub enum DedupeError { ... }
```

### Effect Journal

```rust
pub trait EffectJournal: Send + Sync { ... }
pub enum EffectJournalError { ... }
```

### Snapshots

```rust
pub trait SnapshotStore: Send + Sync { ... }
pub enum SnapshotPolicy { EveryNEvents(u64), Disabled }
pub struct SnapshotHeader { ... }
```

### Timer Index

```rust
pub fn scan_due_timers(store, from, to, max) -> Vec<TimerEntry>
pub fn encode_timer_key(namespace, instance_id, fire_at_ms) -> Vec<u8>
```

### Lease Store

```rust
pub struct LeaseStore { ... }
pub struct LeaseEntry { ... }
```

### Write Classes and Budget Saga

```rust
pub enum WriteClass { CriticalControlPlane, OperatorProjection, BulkBlob }
pub struct BudgetSaga { ... }
pub enum SagaStatus { ... }
```

### Compensation Saga

```rust
pub struct CompensationSaga { ... }
pub enum SagaCompensationStatus { ... }
```

### Key Encoding

```rust
pub fn encode_timer_key(namespace, instance_id, fire_at_ms) -> Vec<u8>
pub fn encode_lease_key(namespace, instance_id, step_id) -> Vec<u8>
pub fn encode_instance_key(namespace, instance_id) -> Vec<u8>
```

---

## vo-executor

Step execution with timeout, retry, and scheduling.

### Execution Functions

```rust
pub fn execute_step(step_id: &StepId, timeout_ms: u64) -> Result<StepResult, ExecuteNodeError>
pub fn execute_step_with_retry(step_id, timeout_ms, retry_policy) -> Result<StepResult, ExecuteNodeError>
pub fn cancel_execution(step_id: &StepId) -> Result<(), ExecuteNodeError>
pub fn get_execution_status(step_id) -> ExecutionStatus
pub fn get_last_error(step_id) -> Option<ExecuteNodeError>
```

### Runtime

```rust
pub struct Runtime { ... }
impl Runtime {
    pub fn new() -> Self
    pub fn block_on<F: Future>(&self, future: F) -> F::Output
    pub fn execute_step_sync(step_id, timeout_ms) -> Result<StepResult, ExecuteNodeError>
}
```

### Types

```rust
pub struct StepId(String);        // new, parse, as_str
pub enum StepResult { Success, Failure }
pub enum ExecutionStatus { Ready, Executing, Completed, Cancelled }
pub struct RetryPolicy { max_attempts, backoff_ms, backoff_multiplier, max_backoff_ms }
```

### Scheduler

```rust
pub struct Scheduler { ... }
pub enum JobPriority { Critical, High, Normal, Low }
pub enum Schedule { Cron(String), OneShot, Interval(Duration) }
pub struct Job { pub id, pub priority, pub payload, pub schedule }
```

### Error Types

```rust
pub enum ExecuteNodeError {
    StepNotFound, InvalidTimeout, TimeoutExceeded, InvalidTransition,
    RetryExhausted, InvalidRetryPolicy, ExecutionCancelled, TransientError,
}
```

---

## vo-worker

Distributed lock manager and connector runtime.

### Lock Manager

```rust
pub enum LockMode { Shared, Exclusive }
pub enum LockStatus { Held, Pending, Expired }
pub struct LockEntry { pub lock_id, pub owner, pub mode, pub status, pub acquired_at, pub expires_at, pub hold_token }

pub trait LockManager: Send + Sync {
    async fn acquire(&self, request: LockRequest) -> Result<LockResponse, LockError>;
    async fn release(&self, release: LockRelease) -> Result<(), LockError>;
    async fn query(&self, query: LockQuery) -> Result<LockQueryResponse, LockError>;
    // ...
}
```

### Connector Runtime

```rust
pub trait Connector: Send + Sync {
    async fn prepare(&self) -> Result<Vec<PreparedEffect>, ConnectorError>;
    async fn commit(&self, effects: Vec<PreparedEffect>) -> Result<Vec<CommitOutcome>, ConnectorError>;
    async fn reconcile(&self, effect_id: String) -> Result<ReconcileOutcome, ConnectorError>;
    async fn compensate(&self, effect_id: String) -> Result<(), ConnectorError>;
}

pub struct HttpConnector { ... }   // implements Connector
pub struct ConnectorRegistry { ... }
```

### Wait-For Graph (Deadlock Detection)

```rust
pub struct WaitForGraph { ... }
```

Methods: `add_edge()`, `detect_cycle()`, `get_waiters()`, `remove_edges_for_owner()`.

---

## vo-cli

Operator CLI (`vo` command).

### Commands

```rust
pub enum Command { Purge, Check, Gc, Init, Lock, Doctor, Rebuild }
pub struct Cli { ... }  // clap-based
```

### Middleware

```rust
pub trait Middleware: Send + Sync {
    fn name(&self) -> &str;
    fn before(&self, ctx: &mut CommandContext) -> Result<(), CliError>;
    fn after(&self, ctx: &CommandContext, result: &Result<(), CliError>) -> Result<(), CliError>;
}
```

---

## vo-frontend

Dioxus web UI for workflow visualization.

### Node Types

```rust
pub enum NodeTemplateId {
    HttpHandler, Run, Timer, SetState, GetState, SendMessage,
    Condition, Signal, Branch, ContinueAsNew, Sleep, Poll,
    SetTimer, CancelTimer,
}
```

### UI Components

`NodeCommandPalette`, `PrototypePalette`, `SketchNode`.

---

## vo-linter

Static analysis for workflow definitions.

```rust
pub enum LintCode { L002 }
pub struct Diagnostic { ... }
pub fn check_random_in_workflow(file: &syn::File) -> Vec<Diagnostic>
```

Detects non-deterministic `Uuid::new_v4()` and `rand::random()` calls in workflow definitions.

---

## vo-sdk-macros

Procedural macro for task binary entrypoints.

```rust
#[task_macro]
fn my_task() -> Result<Value, SdkError> {
    let input = vo_sdk::read_input()?;
    let result = process(input.data);
    vo_sdk::write_success(&result)?;
    Ok(result)
}
```

Generates a `fn main()` wrapper. Sync functions are called directly; async functions are wrapped in `tokio::runtime::Builder::new_current_thread()`.
