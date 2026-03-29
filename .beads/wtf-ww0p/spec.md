# vo-ww0p — e2e: Test happy path workflow completion

**Bead ID:** vo-ww0p
**Status:** ready
**Priority:** 1
**Type:** feature
**Effort:** 2hr

---

## 1. Objective

Create `crates/vo-api/tests/e2e_workflow_test.rs` that spins up a real NATS JetStream server (`vo-nats-test` on port 4222, or `NATS_URL`), provisions streams, boots the full vo-api axum application with a live Ractor orchestrator, uploads a simple procedural workflow definition via `POST /api/v1/definitions/procedural`, starts an instance via `POST /api/v1/workflows`, and polls `GET /api/v1/workflows/:id/journal` until the workflow reaches a terminal state — proving the entire write-read path works end-to-end with no mocks.

## 2. Context

The project has HTTP-layer tests (`crates/vo-api/tests/journal_test.rs`) that mock the `OrchestratorMsg` actor ref, so they never exercise NATS or the actor system. The worker crate has full integration tests (`crates/vo-worker/tests/worker_integration_tests.rs`) that connect to real NATS but don't touch the API layer. There is no test that exercises the complete vertical slice: HTTP request → axum handler → Ractor orchestrator → NATS JetStream → journal replay → HTTP response.

Key components and their locations:

- **Router**: `crates/vo-api/src/app.rs` — `build_app(master, kv)` creates the full axum `Router` nested under `/api/v1/`
- **Start handler**: `crates/vo-api/src/handlers/workflow.rs:18` — `start_workflow` extracts `Extension<ActorRef<OrchestratorMsg>>`, calls `OrchestratorMsg::StartWorkflow`
- **Definition handler**: `crates/vo-api/src/handlers/definitions.rs:5` — `ingest_definition` calls `vo_linter::lint_workflow_code`, returns `DefinitionResponse { valid, diagnostics }`
- **Journal handler**: `crates/vo-api/src/handlers/journal.rs:19` — `get_journal` gets the `EventStore` from the orchestrator, opens a replay stream, maps `ReplayedEvent`s to `JournalEntry`s
- **Journal storage**: `crates/vo-storage/src/journal.rs:28` — `append_event` publishes to NATS subject `wtf.log.<namespace>.<instance_id>` via msgpack
- **NATS connection**: `crates/vo-storage/src/lib.rs` — `connect(&NatsConfig)` and `provision_streams(&Context)`
- **Worker integration test pattern**: `crates/vo-worker/tests/worker_integration_tests.rs` — `NatsTestServer` struct with `global_test_lock()`, `provision()`, `reset_streams()`

Request/response types:

- `V3StartRequest`: `{ namespace, workflow_type, paradigm, input, instance_id? }` → `crates/vo-api/src/types/requests.rs:28`
- `V3StartResponse`: `{ instance_id, namespace, workflow_type }` → HTTP 201 → `crates/vo-api/src/types/responses.rs:105`
- `DefinitionRequest`: `{ source }` → `crates/vo-api/src/types/requests.rs:22`
- `DefinitionResponse`: `{ valid, diagnostics }` → HTTP 200 → `crates/vo-api/src/types/responses.rs:91`
- `JournalResponse`: `{ invocation_id, entries }` → `crates/vo-api/src/types/responses.rs:54`
- `JournalEntry`: `{ seq, entry_type, name?, input?, output?, timestamp?, duration_ms?, fire_at?, status? }` → `crates/vo-api/src/types/mod.rs:25`

## 3. Scope

### In Scope
- Create `crates/vo-api/tests/e2e_workflow_test.rs` with `#[tokio::test]` tests
- Connect to real NATS JetStream (reusing `vo-storage::connect` / `provision_streams`)
- Boot the orchestrator actor system and build the axum app via `vo_api::app::build_app`
- Bind the app to a random ephemeral port (e.g. `127.0.0.1:0`) and make real HTTP calls via `reqwest`
- `POST /api/v1/definitions/procedural` with a valid clean procedural source
- `POST /api/v1/workflows` with `{ namespace: "e2e", workflow_type: "echo", paradigm: "procedural", input: {} }`
- `GET /api/v1/workflows/e2e/<instance_id>/journal` and assert entries are non-empty and seq-ordered
- `GET /api/v1/workflows/e2e/<instance_id>` and assert the response matches `V3StatusResponse`
- Add `reqwest` to `[dev-dependencies]` in `crates/vo-api/Cargo.toml` if not already present (it is in `[dependencies]`)
- Add `vo-linter` to `[dev-dependencies]` if needed for definition linting in tests

### Out of Scope
- Running a `vo-worker` to process activities (no activity handlers in scope)
- Testing DAG or FSM paradigms (procedural only)
- Testing signal handling or timer scheduling
- Performance benchmarks or load testing
- Frontend (Dioxus) integration

## 4. Contract

```rust
// crates/vo-api/tests/e2e_workflow_test.rs

/// Shared test harness: NATS connection, stream provisioning, app bootstrap, ephemeral HTTP listener.
/// Follows the NatsTestServer pattern from crates/vo-worker/tests/worker_integration_tests.rs.
struct E2eTestServer {
    http_client: reqwest::Client,
    base_url: String,
    nats_js: async_nats::jetstream::Context,
    _guard: OwnedMutexGuard<()>,
    _shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl E2eTestServer {
    /// Connect to NATS, provision streams, spawn Ractor orchestrator,
    /// build axum app, bind to random port.
    async fn new() -> Result<Self, Box<dyn std::error::Error>>;

    /// POST /api/v1/definitions/procedural with the given source.
    /// Returns the DefinitionResponse.
    async fn ingest_definition(&self, source: &str)
        -> Result<DefinitionResponse, Box<dyn std::error::Error>>;

    /// POST /api/v1/workflows with the given V3StartRequest body.
    /// Returns the V3StartResponse (HTTP 201 expected).
    async fn start_workflow(&self, req: &V3StartRequest)
        -> Result<V3StartResponse, Box<dyn std::error::Error>>;

    /// GET /api/v1/workflows/e2e/<instance_id>/journal.
    /// Polls up to `timeout` with 200ms intervals until entries are non-empty.
    /// Returns the JournalResponse.
    async fn await_journal(
        &self,
        namespace: &str,
        instance_id: &str,
        timeout: Duration,
    ) -> Result<JournalResponse, Box<dyn std::error::Error>>;

    /// GET /api/v1/workflows/e2e/<instance_id>.
    /// Returns the V3StatusResponse.
    async fn get_workflow_status(
        &self,
        namespace: &str,
        instance_id: &str,
    ) -> Result<V3StatusResponse, Box<dyn std::error::Error>>;
}
```

## 5. Invariants

1. **Real NATS only**: No mocks for NATS, JetStream, or the event store. Tests fail if NATS is unavailable
2. **Fresh streams per test**: Each test resets (`delete_stream`) and re-provisions all streams to avoid cross-test contamination
3. **Sequential test execution**: `--test-threads=1` required; tests share a `global_test_lock()` (matching worker integration test pattern)
4. **Ephemeral port**: Each test binds to `127.0.0.1:0` — no port conflicts across parallel CI runs (though we run sequentially)
5. **Journal ordering invariant**: All returned `JournalResponse.entries` must have strictly ascending `seq` values
6. **Definition linting**: `POST /api/v1/definitions/procedural` must return `{ valid: true, diagnostics: [] }` for clean source
7. **Start response shape**: `POST /api/v1/workflows` must return HTTP 201 with `{ instance_id, namespace, workflow_type }` where `instance_id` is a non-empty ULID string
8. **Journal non-empty**: After starting a workflow, the journal must contain at least one entry within the polling timeout

## 6. Affected Files

| File | Change |
|------|--------|
| `crates/vo-api/tests/e2e_workflow_test.rs` | **New file** — full e2e test suite |
| `crates/vo-api/Cargo.toml` | Add `vo-linter` to `[dev-dependencies]` if needed |

## 7. Dependencies

- `vo-storage::connect` / `vo-storage::provision_streams` — NATS bootstrap (already a dependency)
- `vo-storage::kv::KvStores` — required by `build_app(master, kv)` (already a dependency)
- `vo-actor::OrchestratorMsg` — actor messages (already a dependency)
- `vo_api::app::build_app` — axum router factory
- `reqwest` — HTTP client for test assertions (already in `[dependencies]`, usable in tests)
- Live NATS server at `127.0.0.1:4222` (Docker container `vo-nats-test`) or `NATS_URL` env override

## 8. Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Orchestrator requires sled DB path that may conflict | Medium | Use `sled::Config::temporary().open()` for test-only in-memory sled |
| Orchestrator spawn fails because streams not yet provisioned | Low | Provision streams BEFORE spawning orchestrator |
| Definition handler uses `vo_linter::lint_workflow_code` which may parse-fail on malformed source | Low | Use a known-clean procedural source (no `thread::spawn`, no direct IO) |
| Journal entries empty because workflow hasn't processed yet | Medium | Poll with 200ms intervals up to 10s timeout (matching worker integration test patterns) |
| Test flakes due to NATS timing | Low | Sequential test execution via `global_test_lock()` and `--test-threads=1` |
| `KvStores` construction requires NATS context | Medium | Construct `KvStores` from the same JetStream context used for provisioning |

## 9. Given-When-Then

### Test 1: Ingest definition returns valid for clean procedural source
```gherkin
Given a running vo-api server connected to real NATS
  And a procedural workflow source with no lint violations:
    "impl WorkflowFn for EchoWorkflow {
       async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
           let _ = 42;
           Ok(())
       }
     }"
When POST /api/v1/definitions/procedural is called with body { source: "<source>" }
Then HTTP 200 is returned
  And response body matches DefinitionResponse { valid: true, diagnostics: [] }
```

### Test 2: Start workflow returns 201 with instance_id
```gherkin
Given a running vo-api server connected to real NATS
When POST /api/v1/workflows is called with body:
    {
      "namespace": "e2e",
      "workflow_type": "echo",
      "paradigm": "procedural",
      "input": {}
    }
Then HTTP 201 is returned
  And response body matches V3StartResponse {
       instance_id: "<non-empty ULID string>",
       namespace: "e2e",
       workflow_type: "echo"
     }
```

### Test 3: Journal contains entries after workflow start
```gherkin
Given a running vo-api server connected to real NATS
  And a workflow has been started via POST /api/v1/workflows with instance_id = "<id>"
When GET /api/v1/workflows/e2e/<id>/journal is polled every 200ms for up to 10s
Then HTTP 200 is eventually returned
  And response body matches JournalResponse {
       invocation_id: "e2e/<id>",
       entries: [<at least one entry>]
     }
  And entries are sorted by ascending seq
```

### Test 4: Workflow status returns V3StatusResponse
```gherkin
Given a running vo-api server connected to real NATS
  And a workflow has been started with instance_id = "<id>"
When GET /api/v1/workflows/e2e/<id> is called
Then HTTP 200 is returned
  And response body matches V3StatusResponse {
       instance_id: "<id>",
       namespace: "e2e",
       workflow_type: "echo",
       paradigm: "procedural",
       phase: "live",
       events_applied: <u64 >= 1>
     }
```

### Test 5: List workflows includes the started instance
```gherkin
Given a running vo-api server connected to real NATS
  And a workflow has been started with instance_id = "<id>"
When GET /api/v1/workflows is called
Then HTTP 200 is returned
  And response body is a JSON array containing at least one object
  And at least one object has instance_id == "<id>"
```

### Test 6: Invalid paradigm returns 400
```gherkin
Given a running vo-api server connected to real NATS
When POST /api/v1/workflows is called with body:
    {
      "namespace": "e2e",
      "workflow_type": "bad",
      "paradigm": "quantum_computing",
      "input": {}
    }
Then HTTP 400 is returned
  And response body contains { "error": "invalid_paradigm" }
```

### Test 7: Invalid namespace returns 400
```gherkin
Given a running vo-api server connected to real NATS
When POST /api/v1/workflows is called with body:
    {
      "namespace": "has spaces!",
      "workflow_type": "bad",
      "paradigm": "procedural",
      "input": {}
    }
Then HTTP 400 is returned
  And response body contains { "error": "invalid_namespace" }
```

## 10. Data Flow

```
Test function
  │
  ├─ E2eTestServer::new()
  │    ├─ global_test_lock().lock_owned()
  │    ├─ NatsConfig { urls: ["nats://127.0.0.1:4222"], embedded: true }
  │    ├─ vo_storage::connect(&config) → async_nats::Client
  │    ├─ client.jetstream() → js: Context
  │    ├─ provision_streams(&js)  // creates vo-work, vo-events, vo-signals, vo-archive
  │    ├─ sled::Config::temporary().open() → db
  │    ├─ KvStores::new(js.clone(), db.clone())  // or equivalent constructor
  │    ├─ spawn Orchestrator actor (with js, db, kv)
  │    ├─ build_app(master_ref, kv) → Router
  │    ├─ tokio::net::TcpListener::bind("127.0.0.1:0") → port
  │    └─ tokio::spawn(axum::serve(listener, app))
  │
  ├─ POST /api/v1/definitions/procedural
  │    └─ vo_linter::lint_workflow_code(&source) → DefinitionResponse
  │
  ├─ POST /api/v1/workflows
  │    └─ OrchestratorMsg::StartWorkflow → orchestrator spawns WorkflowInstance actor
  │         └─ WorkflowInstance::pre_start
  │              ├─ replay_events (empty for fresh instance)
  │              ├─ transition_to_live
  │              └─ publish InstanceStarted → append_event → NATS subject "wtf.log.e2e.<id>"
  │
  ├─ GET /api/v1/workflows/e2e/<id>/journal  (polled)
  │    └─ OrchestratorMsg::GetEventStore → EventStore
  │         └─ open_replay_stream("e2e", "<id>", 1)
  │              └─ reads from NATS stream → maps ReplayedEvent → JournalEntry
  │
  └─ GET /api/v1/workflows/e2e/<id>
       └─ OrchestratorMsg::GetStatus → InstanceStatusSnapshot → V3StatusResponse
```

## 11. Acceptance Criteria

- [ ] `crates/vo-api/tests/e2e_workflow_test.rs` exists with all 7 tests
- [ ] Tests connect to real NATS (no mocks) using `vo-storage::connect`
- [ ] Tests provision NATS streams via `vo-storage::provision_streams`
- [ ] Tests boot the full axum app via `vo_api::app::build_app`
- [ ] Tests bind to ephemeral port and make HTTP calls via `reqwest`
- [ ] Definition ingestion returns `{ valid: true, diagnostics: [] }` for clean source
- [ ] Workflow start returns HTTP 201 with `V3StartResponse` containing a non-empty `instance_id`
- [ ] Journal endpoint returns non-empty, seq-sorted entries within 10s
- [ ] Status endpoint returns `V3StatusResponse` with matching `instance_id` and `paradigm: "procedural"`
- [ ] List endpoint includes the started instance
- [ ] Invalid paradigm and invalid namespace return HTTP 400
- [ ] `cargo test -p vo-api --test e2e_workflow -- --test-threads=1` passes (requires NATS)
- [ ] `cargo clippy -p vo-api -- -D warnings` passes
- [ ] No `unwrap()` or `expect()` in test harness code (use `?` with `Box<dyn Error>`)

## 12. Implementation Sketch

```rust
// crates/vo-api/tests/e2e_workflow_test.rs

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex, OwnedMutexGuard};

use vo_storage::{connect, provision_streams, NatsConfig};

// Reuse type shapes from the API crate for deserialization
#[derive(Debug, Deserialize)]
struct V3StartRequest {
    namespace: String,
    workflow_type: String,
    paradigm: String,
    input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct V3StartResponse {
    instance_id: String,
    namespace: String,
    workflow_type: String,
}

#[derive(Debug, Deserialize)]
struct V3StatusResponse {
    instance_id: String,
    namespace: String,
    workflow_type: String,
    paradigm: String,
    phase: String,
    events_applied: u64,
}

#[derive(Debug, Deserialize)]
struct JournalResponse {
    invocation_id: String,
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Deserialize)]
struct JournalEntry {
    seq: u32,
    #[serde(rename = "type")]
    entry_type: String,
    name: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DefinitionResponse {
    valid: bool,
    diagnostics: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
    message: String,
}

struct E2eTestServer {
    http_client: reqwest::Client,
    base_url: String,
    nats_js: async_nats::jetstream::Context,
    _guard: OwnedMutexGuard<()>,
    _shutdown_tx: watch::Sender<bool>,
}

fn global_test_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

impl E2eTestServer {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let guard = global_test_lock().lock_owned().await;

        let config = NatsConfig {
            urls: vec![std::env::var("NATS_URL")
                .unwrap_or_else(|_| "nats://127.0.0.1:4222".into())],
            embedded: true,
            connect_timeout_ms: 5_000,
            credentials_path: None,
        };

        let client = connect(&config).await?;
        let js = client.jetstream().clone();

        // Reset and provision streams
        for name in ["vo-work", "vo-events", "vo-signals", "vo-archive"] {
            let _ = js.delete_stream(name).await;
        }
        provision_streams(&js).await?;

        // Create sled temp DB
        let db = sled::Config::temporary().open()?;

        // Create KvStores (check actual constructor signature)
        let kv = vo_storage::kv::KvStores::new(js.clone(), db.clone())?;

        // Spawn orchestrator — needs the real Orchestrator actor.
        // Use vo_actor::Orchestrator or the public spawn function.
        let master = vo_actor::spawn_orchestrator(
            js.clone(),
            db,
            kv.clone(),
        ).await?;

        let app = vo_api::app::build_app(master, kv);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let base_url = format!("http://127.0.0.1:{port}");

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await
                .ok();
        });

        // Give the server a moment to start accepting connections
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(Self {
            http_client: reqwest::Client::new(),
            base_url,
            nats_js: js,
            _guard: guard,
            _shutdown_tx: shutdown_tx,
        })
    }

    async fn ingest_definition(
        &self, source: &str,
    ) -> Result<DefinitionResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({ "source": source });
        let resp = self.http_client
            .post(format!("{}/api/v1/definitions/procedural", self.base_url))
            .json(&body)
            .send()
            .await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(resp.json().await?)
    }

    async fn start_workflow(
        &self, req: &V3StartRequest,
    ) -> Result<V3StartResponse, Box<dyn std::error::Error>> {
        let resp = self.http_client
            .post(format!("{}/api/v1/workflows", self.base_url))
            .json(req)
            .send()
            .await?;
        assert_eq!(resp.status(), StatusCode::CREATED);
        Ok(resp.json().await?)
    }

    async fn await_journal(
        &self, namespace: &str, instance_id: &str, timeout: Duration,
    ) -> Result<JournalResponse, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/workflows/{}%2F{}/journal",
            self.base_url, namespace, instance_id
        );
        let start = tokio::time::Instant::now();
        loop {
            let resp = self.http_client.get(&url).send().await?;
            if resp.status() == StatusCode::OK {
                let journal: JournalResponse = resp.json().await?;
                if !journal.entries.is_empty() {
                    // Assert seq ordering
                    let seqs: Vec<u32> = journal.entries.iter().map(|e| e.seq).collect();
                    for w in seqs.windows(2) {
                        assert!(w[0] < w[1], "journal entries must be strictly ascending by seq");
                    }
                    return Ok(journal);
                }
            }
            if start.elapsed() > timeout {
                return Err("journal entries not available within timeout".into());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    async fn get_workflow_status(
        &self, namespace: &str, instance_id: &str,
    ) -> Result<V3StatusResponse, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/api/v1/workflows/{}%2F{}",
            self.base_url, namespace, instance_id
        );
        let resp = self.http_client.get(&url).send().await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(resp.json().await?)
    }

    async fn list_workflows(&self) -> Result<Vec<V3StatusResponse>, Box<dyn std::error::Error>> {
        let resp = self.http_client
            .get(format!("{}/api/v1/workflows", self.base_url))
            .send()
            .await?;
        assert_eq!(resp.status(), StatusCode::OK);
        Ok(resp.json().await?)
    }
}

const CLEAN_PROCEDURAL_SOURCE: &str = r#"
impl WorkflowFn for EchoWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = 42;
        Ok(())
    }
}
"#;

// ── Test 1: Definition ingestion ──────────────────────────────────────────
#[tokio::test]
async fn e2e_ingest_definition_returns_valid_for_clean_procedural_source() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let result = server.ingest_definition(CLEAN_PROCEDURAL_SOURCE).await.expect("ingest");
    assert!(result.valid, "definition should be valid, diagnostics: {:?}", result.diagnostics);
    assert!(result.diagnostics.is_empty(), "clean source should produce no diagnostics");
}

// ── Test 2: Start workflow returns 201 ────────────────────────────────────
#[tokio::test]
async fn e2e_start_workflow_returns_201_with_instance_id() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = V3StartRequest {
        namespace: "e2e".into(),
        workflow_type: "echo".into(),
        paradigm: "procedural".into(),
        input: serde_json::json!({}),
        instance_id: None,
    };
    let resp = server.start_workflow(&req).await.expect("start");
    assert!(!resp.instance_id.is_empty(), "instance_id should be non-empty");
    assert_eq!(resp.workflow_type, "echo");
}

// ── Test 3: Journal contains entries ──────────────────────────────────────
#[tokio::test]
async fn e2e_journal_contains_entries_after_workflow_start() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = V3StartRequest {
        namespace: "e2e".into(),
        workflow_type: "echo".into(),
        paradigm: "procedural".into(),
        input: serde_json::json!({}),
        instance_id: None,
    };
    let start = server.start_workflow(&req).await.expect("start");
    let journal = server.await_journal("e2e", &start.instance_id, Duration::from_secs(10))
        .await.expect("journal");
    assert!(!journal.entries.is_empty(), "journal should have entries");
    assert!(journal.invocation_id.contains(&start.instance_id));
}

// ── Test 4: Status returns V3StatusResponse ───────────────────────────────
#[tokio::test]
async fn e2e_workflow_status_returns_matching_response() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = V3StartRequest {
        namespace: "e2e".into(),
        workflow_type: "echo".into(),
        paradigm: "procedural".into(),
        input: serde_json::json!({}),
        instance_id: None,
    };
    let start = server.start_workflow(&req).await.expect("start");
    let status = server.get_workflow_status("e2e", &start.instance_id)
        .await.expect("status");
    assert_eq!(status.instance_id, start.instance_id);
    assert_eq!(status.workflow_type, "echo");
    assert_eq!(status.paradigm, "procedural");
    assert_eq!(status.phase, "live");
    assert!(status.events_applied >= 1);
}

// ── Test 5: List includes started instance ────────────────────────────────
#[tokio::test]
async fn e2e_list_workflows_includes_started_instance() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = V3StartRequest {
        namespace: "e2e".into(),
        workflow_type: "echo".into(),
        paradigm: "procedural".into(),
        input: serde_json::json!({}),
        instance_id: None,
    };
    let start = server.start_workflow(&req).await.expect("start");
    let list = server.list_workflows().await.expect("list");
    assert!(list.iter().any(|w| w.instance_id == start.instance_id),
        "list should contain the started instance");
}

// ── Test 6: Invalid paradigm returns 400 ─────────────────────────────────
#[tokio::test]
async fn e2e_invalid_paradigm_returns_400() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = serde_json::json!({
        "namespace": "e2e",
        "workflow_type": "bad",
        "paradigm": "quantum_computing",
        "input": {}
    });
    let resp = server.http_client
        .post(format!("{}/api/v1/workflows", server.base_url))
        .json(&req)
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await.expect("body");
    assert_eq!(body.error, "invalid_paradigm");
}

// ── Test 7: Invalid namespace returns 400 ────────────────────────────────
#[tokio::test]
async fn e2e_invalid_namespace_returns_400() {
    let server = E2eTestServer::new().await.expect("e2e server");
    let req = serde_json::json!({
        "namespace": "has spaces!",
        "workflow_type": "bad",
        "paradigm": "procedural",
        "input": {}
    });
    let resp = server.http_client
        .post(format!("{}/api/v1/workflows", server.base_url))
        .json(&req)
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await.expect("body");
    assert_eq!(body.error, "invalid_namespace");
}
```

## 13. Test Plan

| Test | Run Command | Assert |
|------|-------------|--------|
| Definition ingestion | `cargo test -p vo-api --test e2e_workflow e2e_ingest -- --test-threads=1 --nocapture` | HTTP 200, `valid: true`, empty diagnostics |
| Start workflow 201 | `cargo test -p vo-api --test e2e_workflow e2e_start_workflow -- --test-threads=1 --nocapture` | HTTP 201, non-empty `instance_id`, `workflow_type: "echo"` |
| Journal non-empty | `cargo test -p vo-api --test e2e_workflow e2e_journal -- --test-threads=1 --nocapture` | HTTP 200, `entries.len() > 0`, seq ascending |
| Status match | `cargo test -p vo-api --test e2e_workflow e2e_workflow_status -- --test-threads=1 --nocapture` | HTTP 200, matching `instance_id`, `paradigm: "procedural"`, `phase: "live"` |
| List includes | `cargo test -p vo-api --test e2e_workflow e2e_list_workflows -- --test-threads=1 --nocapture` | HTTP 200, array contains started instance |
| Invalid paradigm | `cargo test -p vo-api --test e2e_workflow e2e_invalid_paradigm -- --test-threads=1 --nocapture` | HTTP 400, `error: "invalid_paradigm"` |
| Invalid namespace | `cargo test -p vo-api --test e2e_workflow e2e_invalid_namespace -- --test-threads=1 --nocapture` | HTTP 400, `error: "invalid_namespace"` |
| Full suite | `cargo test -p vo-api --test e2e_workflow -- --test-threads=1` | All 7 tests pass |
| Clippy | `cargo clippy -p vo-api -- -D warnings` | No warnings |

## 14. Non-Goals

- Testing DAG or FSM workflow paradigms (procedural-only for this bead)
- Testing activity dispatch/completion cycles (no worker registered in e2e)
- Testing signal or timer workflows
- Testing crash recovery / replay scenarios
- Performance, latency, or concurrency benchmarks
- CI/CD pipeline integration (separate bead)
- Frontend (Dioxus WASM) interaction testing

## 15. Rollback Plan

Delete `crates/vo-api/tests/e2e_workflow_test.rs` and revert `Cargo.toml` dev-dependencies changes. No production code changes, no schema migrations, no state changes. The test file is purely additive.

## 16. Definition of Done

1. `crates/vo-api/tests/e2e_workflow_test.rs` created with all 7 test functions
2. `E2eTestServer` harness connects to real NATS, provisions streams, boots axum app on ephemeral port
3. All 7 tests pass with `cargo test -p vo-api --test e2e_workflow -- --test-threads=1` (NATS required)
4. `cargo clippy -p vo-api -- -D warnings` — clean
5. No `unwrap()` or `expect()` in test harness (only in test assertion helpers where failure is the expected signal)
6. Journal entries verified to have strictly ascending `seq` values
7. HTTP status codes verified: 200, 201, 400
8. Response body shapes verified against `V3StartResponse`, `V3StatusResponse`, `JournalResponse`, `DefinitionResponse`, `ApiError`
