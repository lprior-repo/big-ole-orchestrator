# Contract Specification: wtf-ww0p — E2E Workflow Completion Test

## Context
- **Feature**: End-to-end integration test that exercises the full vertical slice: HTTP request -> axum handler -> Ractor orchestrator -> NATS JetStream -> journal replay -> HTTP response
- **Domain terms**:
  - `E2eTestServer`: Shared test harness that boots real NATS, Ractor orchestrator, and axum app
  - `MasterOrchestrator`: Ractor root supervisor actor (`wtf_actor::master::MasterOrchestrator`)
  - `OrchestratorConfig`: Configuration struct requiring `event_store`, `state_store`, `snapshot_db`, `task_queue`, and `definitions`
  - `OrchestratorMsg`: Enum of all orchestrator RPC messages (`StartWorkflow`, `GetStatus`, `GetEventStore`, `ListActive`, etc.)
  - `KvStores`: NATS JetStream KV bucket collection (`definitions`, `timers`, `heartbeats`)
  - `provision_streams`: Creates JetStream streams `wtf-work`, `wtf-events`, `wtf-signals`, `wtf-archive`
  - `provision_kv_buckets`: Creates KV buckets required by `KvStores`
  - `V3StartRequest`: `{ namespace, workflow_type, paradigm, input, instance_id? }` — POST /api/v1/workflows body
  - `V3StartResponse`: `{ instance_id, namespace, workflow_type }` — HTTP 201 body
  - `V3StatusResponse`: `{ instance_id, namespace, workflow_type, paradigm, phase, events_applied }` — GET status body
  - `JournalResponse`: `{ invocation_id, entries }` — journal replay body
  - `JournalEntry`: `{ seq, entry_type, name?, input?, output?, timestamp?, duration_ms?, status? }`
  - `DefinitionRequest`: `{ source, workflow_type }` — POST /api/v1/definitions/:type body
  - `DefinitionResponse`: `{ valid, diagnostics }` — definition lint result
  - `ApiError`: `{ error, message }` — error response shape
  - `global_test_lock()`: `Arc<Mutex<()>>` via `OnceLock` for sequential test execution
- **Assumptions**:
  - NATS JetStream is running at `127.0.0.1:4222` (or `NATS_URL` env override) with JetStream enabled
  - `wtf-nats-test` Docker container is healthy and accepting connections
  - `reqwest` is available as a dependency in `wtf-api` (already in `[dependencies]`)
  - `sled` is available as a dependency in `wtf-api` (already present)
  - Tests are run with `--test-threads=1` to prevent cross-test contamination
  - The orchestrator can be spawned with `MasterOrchestrator::spawn(None, MasterOrchestrator, config)` in test context
  - `provision_kv_buckets` must be called alongside `provision_streams` (matching `serve.rs` pattern)
  - `OrchestratorConfig` needs real `event_store`, `state_store` wired to NATS for events to be published
  - `NatsClient` implements `EventStore` and `StateStore` traits
  - The definition ingestion endpoint requires both `source` and `workflow_type` in the request body
- **Open questions**:
  - Whether `NamespaceId::try_new` rejects "has spaces!" — confirmed: yes, namespace validation will reject it (the spec's Test 7 expects 400 `invalid_namespace`, which aligns)
  - Whether the orchestrator can function without `task_queue` for procedural workflows — needs verification; safe to include `None` for task_queue since no activities are dispatched
  - Exact `KvStores` constructor signature — check `wtf_storage::kv::KvStores` source

## Preconditions
- NATS JetStream server is reachable at the configured URL
- `connect(&NatsConfig)` succeeds (NATS connection established)
- `provision_streams(&js)` succeeds (all 4 JetStream streams created)
- `provision_kv_buckets(&js)` succeeds (KV buckets for definitions, timers, heartbeats created)
- `MasterOrchestrator::spawn()` succeeds (actor system healthy)
- `build_app(master, kv)` returns a valid `Router` (all extensions injected)
- TCP listener binds to `127.0.0.1:0` (ephemeral port available)
- axum server accepts connections within 500ms of spawn (startup latency budget)
- `DefinitionRequest.workflow_type` is non-empty (validated by handler; empty returns 400 `invalid_request`)
- `DefinitionRequest.source` is valid Rust syntax (parseable by `wtf_linter::lint_workflow_code`)
- `V3StartRequest.namespace` passes `NamespaceId::try_new` validation (no spaces, no special chars)
- `V3StartRequest.paradigm` is one of `"fsm"`, `"dag"`, `"procedural"` (validated by `parse_paradigm`)
- `V3StartRequest.input` is valid JSON (serializable to `serde_json::Value`)

## Postconditions
- Test 1 (definition ingestion): HTTP 200 with `{ valid: true, diagnostics: [] }` for clean procedural source
- Test 2 (start workflow): HTTP 201 with `V3StartResponse` containing non-empty `instance_id` ULID string
- Test 3 (journal): HTTP 200 with `JournalResponse` containing at least one entry with strictly ascending `seq`
- Test 4 (status): HTTP 200 with `V3StatusResponse` matching `instance_id`, `paradigm: "procedural"`, `phase: "live"`, `events_applied >= 1`
- Test 5 (list): HTTP 200 with JSON array containing an entry matching the started `instance_id`
- Test 6 (invalid paradigm): HTTP 400 with `{ error: "invalid_paradigm" }`
- Test 7 (invalid namespace): HTTP 400 with `{ error: "invalid_namespace" }`
- NATS streams contain at least one event for the started workflow instance (verifiable via journal replay)
- Each test resets all streams before execution (no cross-test state leakage)
- Orchestrator actor is stopped via `master.stop()` when test completes (clean shutdown)
- axum HTTP server is shut down gracefully when test completes

## Invariants
- **I-1: Real NATS only** — No mocks for NATS, JetStream, or the event store. If NATS is unreachable, the test must fail with a clear connection error.
- **I-2: Fresh streams per test** — Each `E2eTestServer::new()` call deletes and re-provisions all JetStream streams (`wtf-work`, `wtf-events`, `wtf-signals`, `wtf-archive`) to guarantee isolation.
- **I-3: Sequential execution** — All tests hold `global_test_lock()` via `OwnedMutexGuard` for the test's lifetime. Running with `--test-threads=1` is required.
- **I-4: Ephemeral port** — Each test binds to `127.0.0.1:0`. No hardcoded ports. No port conflict between tests.
- **I-5: Journal seq ordering** — All `JournalResponse.entries` have strictly ascending `seq` values. The server-side `sort_entries_by_seq` must enforce this; the test also verifies it client-side.
- **I-6: Definition response shape** — `POST /api/v1/definitions/procedural` returns `DefinitionResponse { valid, diagnostics }`. For clean source, `valid == true` and `diagnostics` is empty.
- **I-7: Start response shape** — `POST /api/v1/workflows` returns HTTP 201 with `V3StartResponse { instance_id, namespace, workflow_type }` where `instance_id` is a non-empty string (ULID format).
- **I-8: Journal non-empty** — After starting a procedural workflow, the journal must contain at least one entry (the `InstanceStarted` event) within the polling timeout.
- **I-9: Status reflects live phase** — After starting a workflow, `GET /api/v1/workflows/:id` returns `phase: "live"` and `events_applied >= 1`.
- **I-10: List consistency** — `GET /api/v1/workflows` returns a superset of all started instances.
- **I-11: Error response shape** — All error responses use `ApiError { error, message }` format.
- **I-12: Railway error handling** — `E2eTestServer::new()` returns `Result<Self, Box<dyn Error>`. All helper methods return `Result<..., Box<dyn Error>`. No `unwrap()` or `expect()` in harness code.

## Error Taxonomy

### E2E Infrastructure Errors
- `E2eError::NatsConnectionFailed` — `wtf_storage::connect` returns an error (NATS unreachable, auth failure, timeout)
- `E2eError::StreamProvisionFailed` — `provision_streams` or `provision_kv_buckets` fails (NATS JetStream not enabled, permission denied)
- `E2eError::StreamResetFailed` — `delete_stream` for a non-existent stream returns an error (non-fatal; logged and skipped)
- `E2eError::OrchestratorSpawnFailed` — `MasterOrchestrator::spawn` fails (actor system panic, invalid config)
- `E2eError::AppBuildFailed` — `build_app` panics or returns invalid router (missing extensions)
- `E2eError::PortBindFailed` — `TcpListener::bind("127.0.0.1:0")` fails (file descriptor exhaustion)
- `E2eError::ServerStartupTimeout` — axum server does not accept connections within 500ms of spawn

### E2E Assertion Errors (test failures, not infrastructure)
- `E2eError::UnexpectedStatus { expected, actual, url }` — HTTP response status code does not match expected value
- `E2eError::ResponseBodyParseFailed { url, reason }` — Response body cannot be deserialized into expected type
- `E2eError::JournalTimeout { elapsed, timeout }` — Journal entries did not appear within polling timeout
- `E2eError::JournalSeqOrderViolated { seq_before, seq_after }` — Journal entries are not strictly ascending by seq
- `E2eError::InstanceIdEmpty` — `V3StartResponse.instance_id` is an empty string
- `E2eError::InstanceNotInList { instance_id }` — Started instance not found in list response
- `E2eError::PhaseNotLive { actual_phase }` — Workflow status phase is not "live" after start
- `E2eError::EventsAppliedZero` — `events_applied` is 0 after workflow start

### Contract Violations Detected During Validation
- `E2eError::MockDetected` — Test harness attempted to use a mock instead of real NATS connection
- `E2eError::UnwrapInHarness` — Static analysis detected `unwrap()` or `expect()` in harness code (enforced via clippy)

## Contract Signatures

### Test Harness Struct
```rust
struct E2eTestServer {
    http_client: reqwest::Client,
    base_url: String,
    nats_js: async_nats::jetstream::Context,
    _guard: OwnedMutexGuard<()>,
    _shutdown_tx: tokio::sync::watch::Sender<bool>,
}
```

### Harness Constructor
```rust
impl E2eTestServer {
    /// Boot full vertical slice: NATS -> provision -> orchestrator -> axum -> HTTP.
    /// Returns Err if any infrastructure component fails to start.
    async fn new() -> Result<Self, Box<dyn std::error::Error>>;
}
```

### HTTP Helper Methods
```rust
impl E2eTestServer {
    /// POST /api/v1/definitions/:type — lint and store a workflow definition.
    /// NOTE: Must include both `source` AND `workflow_type` in the request body.
    async fn ingest_definition(
        &self,
        definition_type: &str,
        source: &str,
        workflow_type: &str,
    ) -> Result<DefinitionResponse, Box<dyn std::error::Error>>;

    /// POST /api/v1/workflows — start a workflow instance.
    /// Asserts HTTP 201 internally; returns parsed V3StartResponse.
    async fn start_workflow(
        &self,
        req: &V3StartRequest,
    ) -> Result<V3StartResponse, Box<dyn std::error::Error>>;

    /// GET /api/v1/workflows/:namespace/:instance_id/journal — poll until non-empty.
    /// Polls every 200ms up to `timeout`. Asserts HTTP 200 and seq ordering internally.
    async fn await_journal(
        &self,
        namespace: &str,
        instance_id: &str,
        timeout: Duration,
    ) -> Result<JournalResponse, Box<dyn std::error::Error>>;

    /// GET /api/v1/workflows/:namespace/:instance_id — get workflow status.
    /// Asserts HTTP 200 internally; returns parsed V3StatusResponse.
    async fn get_workflow_status(
        &self,
        namespace: &str,
        instance_id: &str,
    ) -> Result<V3StatusResponse, Box<dyn std::error::Error>>;

    /// GET /api/v1/workflows — list active workflow instances.
    /// Asserts HTTP 200 internally; returns parsed Vec<V3StatusResponse>.
    async fn list_workflows(
        &self,
    ) -> Result<Vec<V3StatusResponse>, Box<dyn std::error::Error>>;
}
```

### Infrastructure Boot Sequence (inside `E2eTestServer::new()`)
```rust
// 1. Acquire global test lock (sequential execution)
let guard = global_test_lock().lock_owned().await;

// 2. Connect to real NATS
let config = NatsConfig { urls: [...], embedded: true, ... };
let client = connect(&config).await?;

// 3. Create JetStream context
let js = client.jetstream().clone();

// 4. Reset and provision streams (isolation)
for name in ["wtf-work", "wtf-events", "wtf-signals", "wtf-archive"] {
    let _ = js.delete_stream(name).await; // non-fatal
}
provision_streams(&js).await?;
provision_kv_buckets(&js).await?;

// 5. Build KvStores
let kv = KvStores::new(js.clone())?;

// 6. Create temporary sled DB
let db = sled::Config::temporary().open()?;

// 7. Wire stores as trait objects
let event_store: Arc<dyn EventStore> = Arc::new(client.clone());
let state_store: Arc<dyn StateStore> = Arc::new(client.clone());

// 8. Spawn orchestrator with full config
let config = OrchestratorConfig {
    max_instances: 100,
    engine_node_id: "e2e-test".into(),
    snapshot_db: Some(db),
    event_store: Some(event_store),
    state_store: Some(state_store),
    task_queue: None, // no worker in e2e scope
    definitions: Vec::new(),
};
let (master, _handle) = MasterOrchestrator::spawn(None, MasterOrchestrator, config).await?;

// 9. Build and serve axum app
let app = build_app(master.clone(), kv);
let listener = TcpListener::bind("127.0.0.1:0").await?;
let port = listener.local_addr()?.port();
tokio::spawn(axum::serve(listener, app).with_graceful_shutdown(...));

// 10. Wait for server readiness
tokio::time::sleep(Duration::from_millis(500)).await;
```

### Test Function Signatures
```rust
#[tokio::test]
async fn e2e_definition_ingestion_returns_valid_for_clean_procedural_source() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_start_workflow_returns_201_with_instance_id() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_journal_contains_entries_after_workflow_start() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_workflow_status_returns_matching_response() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_list_workflows_includes_started_instance() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_invalid_paradigm_returns_400() -> Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn e2e_invalid_namespace_returns_400() -> Result<(), Box<dyn std::error::Error>>;
```

## Non-goals
- Running a `wtf-worker` to process activities (no activity handlers in scope)
- Testing DAG or FSM paradigms (procedural only)
- Testing signal handling, timer scheduling, or event streaming (SSE)
- Testing crash recovery or replay-from-snapshot scenarios
- Performance benchmarks or load testing
- Frontend (Dioxus WASM) interaction testing
- CI/CD pipeline integration (separate bead)
- Testing `DELETE /api/v1/workflows/:id` (terminate)
- Testing `POST /api/v1/workflows/:id/signals` (signal dispatch)
- Testing `POST /api/v1/workflows/validate` (validation endpoint)
