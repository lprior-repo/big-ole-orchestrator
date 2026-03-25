//! End-to-end integration tests: HTTP → axum → Ractor orchestrator → NATS JetStream → response.
//!
//! Exercises the full vertical slice with real NATS (no mocks).
//!
//! Run: `cargo test -p wtf-api --test e2e_workflow_completion -- --test-threads=1 --nocapture`
//!
//! Prerequisites:
//! - NATS JetStream at `127.0.0.1:4222` (or `NATS_URL` env var)

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use tokio::sync::{watch, Mutex, OwnedMutexGuard};

use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig};
use wtf_storage::{connect, open_snapshot_db, provision_kv_buckets, provision_streams, NatsConfig};

// ── Response DTOs (mirror server types for deserialization) ───────────────

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
    current_state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JournalResponse {
    invocation_id: String,
    entries: Vec<JournalEntryDto>,
}

#[derive(Debug, Deserialize)]
struct JournalEntryDto {
    seq: u32,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    entry_type: String,
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DefinitionResponse {
    valid: bool,
    diagnostics: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
    #[allow(dead_code)]
    message: String,
}

// ── Test harness ──────────────────────────────────────────────────────────

struct E2eTestServer {
    http_client: reqwest::Client,
    base_url: String,
    _nats_client: wtf_storage::NatsClient,
    _guard: OwnedMutexGuard<()>,
    _shutdown_tx: watch::Sender<bool>,
}

fn global_test_lock() -> Arc<Mutex<()>> {
    static LOCK: OnceLock<Arc<Mutex<()>>> = OnceLock::new();
    LOCK.get_or_init(|| Arc::new(Mutex::new(()))).clone()
}

/// Connect to NATS, reset/provision streams, spawn orchestrator, boot axum app.
async fn boot_server() -> Result<E2eTestServer, Box<dyn std::error::Error>> {
    let guard = global_test_lock().lock_owned().await;

    let config = NatsConfig {
        urls: vec![std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into())],
        embedded: false,
        connect_timeout_ms: 5_000,
        credentials_path: None,
    };

    let nats_client = connect(&config).await?;
    let js = nats_client.jetstream().clone();

    // Reset and provision streams (isolation per test)
    reset_all_streams(&js).await;
    provision_streams(&js).await?;
    provision_kv_buckets(&js).await?;

    // Create temporary sled DB
    let tmp_dir = tempfile::tempdir()?;
    let db = open_snapshot_db(tmp_dir.path())?;

    // Wire stores as trait objects
    let event_store: Arc<dyn wtf_common::EventStore> = Arc::new(nats_client.clone());
    let state_store: Arc<dyn wtf_common::StateStore> = Arc::new(nats_client.clone());

    let orch_config = OrchestratorConfig {
        max_instances: 100,
        engine_node_id: "e2e-test".into(),
        snapshot_db: Some(db),
        event_store: Some(event_store),
        state_store: Some(state_store),
        task_queue: None,
        definitions: Vec::new(),
        procedural_workflows: Vec::new(),
    };

    let (master, _handle) = ractor::Actor::spawn(None, MasterOrchestrator, orch_config).await?;

    let kv = provision_kv_buckets(&js).await?;
    let app = wtf_api::app::build_app(master.clone(), kv);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.changed().await;
            })
            .await
            .ok();
    });

    // Allow server startup
    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok(E2eTestServer {
        http_client: reqwest::Client::new(),
        base_url,
        _nats_client: nats_client,
        _guard: guard,
        _shutdown_tx: shutdown_tx,
    })
}

async fn reset_all_streams(js: &async_nats::jetstream::Context) {
    for name in ["wtf-events", "wtf-work", "wtf-signals", "wtf-archive"] {
        let _ = js.delete_stream(name).await;
    }
    for name in [
        "wtf-instances",
        "wtf-timers",
        "wtf-definitions",
        "wtf-heartbeats",
    ] {
        let _ = js.delete_key_value(name).await;
    }
}

// ── Helper methods ────────────────────────────────────────────────────────

impl E2eTestServer {
    async fn ingest_definition(
        &self,
        source: &str,
        workflow_type: &str,
    ) -> Result<DefinitionResponse, Box<dyn std::error::Error>> {
        let body = serde_json::json!({
            "source": source,
            "workflow_type": workflow_type,
        });
        let resp = self
            .http_client
            .post(format!("{}/api/v1/definitions/procedural", self.base_url))
            .json(&body)
            .send()
            .await?;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "definition ingestion should return 200"
        );
        Ok(resp.json().await?)
    }

    async fn start_workflow(
        &self,
        namespace: &str,
        workflow_type: &str,
        paradigm: &str,
        input: serde_json::Value,
        instance_id: Option<&str>,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        let mut body = serde_json::json!({
            "namespace": namespace,
            "workflow_type": workflow_type,
            "paradigm": paradigm,
            "input": input,
        });
        if let Some(id) = instance_id {
            body["instance_id"] = serde_json::json!(id);
        }
        let resp = self
            .http_client
            .post(format!("{}/api/v1/workflows", self.base_url))
            .json(&body)
            .send()
            .await?;
        Ok(resp)
    }

    async fn await_journal(
        &self,
        namespace: &str,
        instance_id: &str,
        timeout: Duration,
    ) -> Result<JournalResponse, Box<dyn std::error::Error>> {
        let encoded_id = format!("{namespace}%2F{instance_id}");
        let url = format!("{}/api/v1/workflows/{encoded_id}/journal", self.base_url);
        let start = tokio::time::Instant::now();

        loop {
            let resp = self.http_client.get(&url).send().await?;
            if resp.status() == StatusCode::OK {
                let journal: JournalResponse = resp.json().await?;
                if !journal.entries.is_empty() {
                    verify_seq_ascending(&journal.entries);
                    return Ok(journal);
                }
            }
            if start.elapsed() > timeout {
                return Err(format!(
                    "journal entries not available within {}ms",
                    timeout.as_millis()
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    async fn get_workflow_status(
        &self,
        namespace: &str,
        instance_id: &str,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        let encoded_id = format!("{namespace}%2F{instance_id}");
        let url = format!("{}/api/v1/workflows/{encoded_id}", self.base_url);
        let resp = self.http_client.get(&url).send().await?;
        Ok(resp)
    }

    async fn await_workflow_status_live(
        &self,
        namespace: &str,
        instance_id: &str,
        timeout: Duration,
    ) -> Result<V3StatusResponse, Box<dyn std::error::Error>> {
        let start = tokio::time::Instant::now();
        loop {
            let resp = self.get_workflow_status(namespace, instance_id).await?;
            if resp.status() == StatusCode::OK {
                let status: V3StatusResponse = resp.json().await?;
                if status.phase == "live" {
                    return Ok(status);
                }
            }
            if start.elapsed() > timeout {
                return Err(format!(
                    "workflow did not reach live phase within {}ms",
                    timeout.as_millis()
                )
                .into());
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    async fn list_workflows(&self) -> Result<Vec<V3StatusResponse>, Box<dyn std::error::Error>> {
        let resp = self
            .http_client
            .get(format!("{}/api/v1/workflows", self.base_url))
            .send()
            .await?;
        assert_eq!(resp.status(), StatusCode::OK, "list should return 200");
        Ok(resp.json().await?)
    }
}

fn verify_seq_ascending(entries: &[JournalEntryDto]) {
    entries.windows(2).for_each(|w| {
        assert!(
            w[0].seq < w[1].seq,
            "journal entries must be strictly ascending by seq: {} >= {}",
            w[0].seq,
            w[1].seq
        )
    });
}

const CLEAN_PROCEDURAL_SOURCE: &str = r#"
impl WorkflowFn for EchoWorkflow {
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()> {
        let _ = 42;
        Ok(())
    }
}
"#;

// ── Scenario 1: Definition ingestion returns valid for clean procedural source ─

#[tokio::test]
async fn e2e_definition_ingestion_returns_valid_for_clean_procedural_source(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let result = server
        .ingest_definition(CLEAN_PROCEDURAL_SOURCE, "echo")
        .await?;

    assert!(
        result.valid,
        "definition should be valid, diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.is_empty(),
        "clean source should produce no diagnostics"
    );
    Ok(())
}

// ── Scenario 2: Start workflow returns 201 with instance_id ───────────────

#[tokio::test]
async fn e2e_start_workflow_returns_201_with_instance_id() -> Result<(), Box<dyn std::error::Error>>
{
    let server = boot_server().await?;
    let resp = server
        .start_workflow("e2e", "echo", "procedural", serde_json::json!({}), None)
        .await?;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: V3StartResponse = resp.json().await?;
    assert!(
        !body.instance_id.is_empty(),
        "instance_id should be non-empty"
    );
    assert_eq!(
        body.instance_id.len(),
        26,
        "instance_id should be 26-char ULID"
    );
    assert_eq!(body.workflow_type, "echo");

    // C1: namespace is always "" in start response (workflow_mappers.rs:62)
    assert_eq!(
        body.namespace, "",
        "start response namespace is always empty"
    );
    Ok(())
}

// ── Scenario 3: Journal contains entries after workflow start ─────────────

#[tokio::test]
async fn e2e_journal_contains_entries_after_workflow_start(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let resp = server
        .start_workflow("e2e", "echo", "procedural", serde_json::json!({}), None)
        .await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let start: V3StartResponse = resp.json().await?;

    let journal = server
        .await_journal("e2e", &start.instance_id, Duration::from_secs(10))
        .await?;

    assert!(!journal.entries.is_empty(), "journal should have entries");
    assert!(
        journal.invocation_id.contains(&start.instance_id),
        "invocation_id should contain the instance_id"
    );

    // At least one entry should have a timestamp
    let has_timestamp = journal.entries.iter().any(|e| e.timestamp.is_some());
    assert!(has_timestamp, "at least one entry should have a timestamp");

    Ok(())
}

// ── Scenario 4: Workflow status returns V3StatusResponse with live phase ──

#[tokio::test]
async fn e2e_workflow_status_returns_matching_response() -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let resp = server
        .start_workflow("e2e", "echo", "procedural", serde_json::json!({}), None)
        .await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let start: V3StartResponse = resp.json().await?;

    let status = server
        .await_workflow_status_live("e2e", &start.instance_id, Duration::from_secs(10))
        .await?;

    assert_eq!(status.instance_id, start.instance_id);
    assert_eq!(status.namespace, "e2e", "status namespace from snapshot");
    assert_eq!(status.workflow_type, "echo");
    assert_eq!(status.paradigm, "procedural");
    assert_eq!(status.phase, "live");
    assert!(
        status.events_applied >= 1,
        "should have at least 1 event applied"
    );
    assert!(status.current_state.is_none());
    Ok(())
}

// ── Scenario 5: List workflows includes the started instance ──────────────

#[tokio::test]
async fn e2e_list_workflows_includes_started_instance() -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let resp = server
        .start_workflow("e2e", "echo", "procedural", serde_json::json!({}), None)
        .await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let start: V3StartResponse = resp.json().await?;

    // Brief settle for the list to reflect the new instance
    tokio::time::sleep(Duration::from_millis(500)).await;

    let list = server.list_workflows().await?;
    let found = list.iter().any(|w| w.instance_id == start.instance_id);
    assert!(found, "list should contain the started instance");
    Ok(())
}

// ── Scenario 6: Invalid paradigm returns 400 ──────────────────────────────

#[tokio::test]
async fn e2e_invalid_paradigm_returns_400() -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let resp = server
        .start_workflow(
            "e2e",
            "bad",
            "quantum_computing",
            serde_json::json!({}),
            None,
        )
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await?;
    assert_eq!(body.error, "invalid_paradigm");
    Ok(())
}

// ── Scenario 7: Empty paradigm returns 400 invalid_paradigm ───────────────

#[tokio::test]
async fn e2e_empty_paradigm_returns_400_invalid_paradigm() -> Result<(), Box<dyn std::error::Error>>
{
    let server = boot_server().await?;
    let resp = server
        .start_workflow("e2e", "echo", "", serde_json::json!({}), None)
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await?;
    assert_eq!(body.error, "invalid_paradigm");
    Ok(())
}

// ── Scenario 8: Invalid namespace returns 400 ─────────────────────────────

#[tokio::test]
async fn e2e_invalid_namespace_returns_400() -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let resp = server
        .start_workflow(
            "has spaces!",
            "bad",
            "procedural",
            serde_json::json!({}),
            None,
        )
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await?;
    assert_eq!(body.error, "invalid_namespace");
    Ok(())
}

// ── Scenario 9: Definition with empty workflow_type returns 400 ───────────

#[tokio::test]
async fn e2e_definition_with_empty_workflow_type_returns_400(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let body = serde_json::json!({
        "source": "fn valid() {}",
        "workflow_type": "",
    });
    let resp = server
        .http_client
        .post(format!("{}/api/v1/definitions/procedural", server.base_url))
        .json(&body)
        .send()
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: ApiError = resp.json().await?;
    assert_eq!(err.error, "invalid_request");
    Ok(())
}

// ── Scenario 10: Definition with malformed source returns 400 ─────────────

#[tokio::test]
async fn e2e_definition_with_malformed_source_returns_400() -> Result<(), Box<dyn std::error::Error>>
{
    let server = boot_server().await?;
    let body = serde_json::json!({
        "source": "!!!not valid rust syntax",
        "workflow_type": "bad-workflow",
    });
    let resp = server
        .http_client
        .post(format!("{}/api/v1/definitions/procedural", server.base_url))
        .json(&body)
        .send()
        .await?;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err: ApiError = resp.json().await?;
    assert_eq!(err.error, "parse_error");
    Ok(())
}

// ── Scenario 11: Definition with lint errors returns 200 { valid: false } ─

#[tokio::test]
async fn e2e_definition_with_lint_errors_returns_200_valid_false(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let source = "impl WorkflowFn for BadWorkflow { async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> { tokio::spawn(async {}); Ok(()) } }";
    let result = server.ingest_definition(source, "lint-violator").await?;

    assert!(
        !result.valid,
        "definition with lint errors should not be valid"
    );
    assert!(
        !result.diagnostics.is_empty(),
        "lint errors should produce diagnostics"
    );

    // At least one diagnostic should be WTF-L005
    let has_l005 = result.diagnostics.iter().any(|d| {
        d.get("code")
            .is_some_and(|code| code.as_str() == Some("WTF-L005"))
    });
    assert!(has_l005, "should have WTF-L005 diagnostic for tokio::spawn");
    Ok(())
}

// ── Scenario 12: Duplicate instance_id start returns 409 ──────────────────

#[tokio::test]
async fn e2e_duplicate_instance_id_start_returns_409() -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;

    // First start — get an instance_id
    let resp = server
        .start_workflow("e2e", "echo", "procedural", serde_json::json!({}), None)
        .await?;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let first: V3StartResponse = resp.json().await?;

    // Second start with the SAME instance_id
    let resp2 = server
        .start_workflow(
            "e2e",
            "echo",
            "procedural",
            serde_json::json!({}),
            Some(&first.instance_id),
        )
        .await?;

    assert_eq!(resp2.status(), StatusCode::CONFLICT);
    let err: ApiError = resp2.json().await?;
    assert_eq!(err.error, "already_exists");
    Ok(())
}

// ── Scenario 13: Journal for non-existent instance returns empty ──────────

#[tokio::test]
async fn e2e_journal_for_nonexistent_instance_returns_empty(
) -> Result<(), Box<dyn std::error::Error>> {
    let server = boot_server().await?;
    let encoded_id = "e2e%2Fnonexistent-id-12345";
    let url = format!("{}/api/v1/workflows/{encoded_id}/journal", server.base_url);
    let resp = server.http_client.get(&url).send().await?;

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "journal returns 200 even for non-existent instance"
    );
    let journal: JournalResponse = resp.json().await?;
    assert!(
        journal.entries.is_empty(),
        "non-existent instance should have empty journal"
    );
    Ok(())
}

// ── Scenario 14: Status for non-existent instance returns 404 ─────────────

#[tokio::test]
async fn e2e_status_for_nonexistent_instance_returns_404() -> Result<(), Box<dyn std::error::Error>>
{
    let server = boot_server().await?;
    let encoded_id = "e2e%2Fnonexistent-id-12345";
    let url = format!("{}/api/v1/workflows/{encoded_id}", server.base_url);
    let resp = server.http_client.get(&url).send().await?;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let err: ApiError = resp.json().await?;
    assert_eq!(err.error, "not_found");
    Ok(())
}
