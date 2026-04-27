use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{sleep, Duration};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const WORKFLOW_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INSTANCE_ID: &str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";

#[tokio::test]
async fn given_serve_command_when_workflow_started_then_events_replay_from_live_server(
) -> TestResult {
    let temp = tempfile::tempdir()?;
    let storage_path = temp.path().join("fjall");
    let mut server = spawn_server(storage_path.clone()).await?;
    let base_url = server.base_url.clone();

    let client = reqwest::Client::new();
    wait_for_ui(&client, &base_url).await?;

    let start_response = client
        .post(format!("{base_url}/api/v1/workflows"))
        .json(&workflow_start_body())
        .send()
        .await?;
    assert_eq!(start_response.status(), reqwest::StatusCode::CREATED);

    let events_response = client
        .get(format!(
            "{base_url}/api/v1/workflows/payments/{INSTANCE_ID}/events"
        ))
        .send()
        .await?;
    assert_eq!(events_response.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = events_response.json().await?;

    assert_eq!(body["total_replayed"], 1);
    assert_eq!(body["events"][0]["payload"]["type"], "WorkflowStarted");
    assert_eq!(body["events"][0]["payload"]["workflow_type"], "checkout");
    assert_eq!(body["events"][0]["payload"]["binary_hash"], WORKFLOW_HASH);

    assert_query_projection_routes(&client, &base_url, 1).await?;
    assert_live_orchestrator_routes(&client, &base_url).await?;
    assert_lifecycle_events_visible(&client, &base_url, 3).await?;
    assert_same_instance_id_in_two_namespaces_isolated(&client, &base_url).await?;

    server.kill().await?;

    let mut server = spawn_server(storage_path.clone()).await?;
    let base_url = server.base_url.clone();
    wait_for_ui(&client, &base_url).await?;

    let restart_events = client
        .get(format!(
            "{base_url}/api/v1/workflows/payments/{INSTANCE_ID}/events"
        ))
        .send()
        .await?;
    assert_eq!(restart_events.status(), reqwest::StatusCode::OK);
    let restart_body: serde_json::Value = restart_events.json().await?;
    assert_eq!(restart_body["total_replayed"], 4);
    assert_status_and_list_survive_restart(&client, &base_url).await?;

    let workflow_url = format!("{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}");
    let terminated = client.delete(workflow_url).send().await?;
    assert_eq!(terminated.status(), reqwest::StatusCode::NO_CONTENT);
    assert_lifecycle_events_visible(&client, &base_url, 5).await?;

    server.kill().await?;

    let mut server = spawn_server(storage_path).await?;
    let base_url = server.base_url.clone();
    wait_for_ui(&client, &base_url).await?;
    assert_terminal_status_survives_restart(&client, &base_url).await?;
    assert_billing_remains_live_after_payments_termination(&client, &base_url).await?;
    server.kill().await?;
    Ok(())
}

struct ServerProcess {
    base_url: String,
    child: Child,
}

impl ServerProcess {
    async fn kill(&mut self) -> TestResult {
        match self.child.start_kill() {
            Ok(()) => {
                let _status = self.child.wait().await?;
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
            Err(error) => Err(Box::new(error)),
        }
    }
}

#[allow(clippy::unused_async)]
async fn spawn_server(storage_path: PathBuf) -> TestResult<ServerProcess> {
    let port = free_port()?;
    let child = Command::new(vo_cli_binary())
        .args([
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--storage-path",
            &storage_path.display().to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ServerProcess {
        base_url: format!("http://127.0.0.1:{port}"),
        child,
    })
}

fn free_port() -> TestResult<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}

fn vo_cli_binary() -> PathBuf {
    option_env!("CARGO_BIN_EXE_vo-cli").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/vo-cli"),
        PathBuf::from,
    )
}

async fn assert_query_projection_routes(
    client: &reqwest::Client,
    base_url: &str,
    expected_count: u64,
) -> TestResult {
    let encoded_id = "payments%2F01H5JYV4XHGSR2F8KZ9BWNRFMA";
    let timeline = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/{encoded_id}/timeline"),
    )
    .await?;
    assert_eq!(timeline["total_replayed"], expected_count);
    assert_eq!(timeline["entries"][0]["event_type"], "WorkflowStarted");

    let history = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/{encoded_id}/history"),
    )
    .await?;
    assert_eq!(history["entries"][0]["event_type"], "WorkflowStarted");

    let journal = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/{encoded_id}/effect-journal"),
    )
    .await?;
    assert_eq!(journal["entries"][0]["semantics"], "unsafe");

    let version = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/{encoded_id}/version"),
    )
    .await?;
    assert_eq!(version["event_count"], expected_count);
    assert_eq!(version["last_sequence"], expected_count);
    Ok(())
}

async fn assert_live_orchestrator_routes(client: &reqwest::Client, base_url: &str) -> TestResult {
    let workflow_url = format!("{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}");
    let status = get_json(client, &workflow_url).await?;
    assert_eq!(status["instance_id"], INSTANCE_ID);
    assert_eq!(status["phase"], "live");

    let active = get_json(client, &format!("{base_url}/api/v1/workflows")).await?;
    let active_items = active.as_array().ok_or("list response must be an array")?;
    assert_eq!(active_items.len(), 1);

    let signal = client
        .post(format!("{workflow_url}/signals"))
        .json(&serde_json::json!({"signal_name": "approval", "payload": {"ok": true}}))
        .send()
        .await?;
    assert_eq!(signal.status(), reqwest::StatusCode::ACCEPTED);

    let compensated = client
        .post(format!("{workflow_url}/compensate"))
        .send()
        .await?;
    assert_eq!(compensated.status(), reqwest::StatusCode::ACCEPTED);

    Ok(())
}

async fn assert_lifecycle_events_visible(
    client: &reqwest::Client,
    base_url: &str,
    expected_count: u64,
) -> TestResult {
    let body = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/payments/{INSTANCE_ID}/events"),
    )
    .await?;
    assert_eq!(body["total_replayed"], expected_count);
    let events = body["events"].as_array().ok_or("events must be an array")?;
    let event_types: Vec<_> = events
        .iter()
        .map(|event| event["payload"]["type"].as_str().unwrap_or("missing"))
        .collect();
    assert!(event_types.contains(&"WorkflowStarted"));
    assert!(event_types.contains(&"SignalAccepted"));
    assert!(event_types.contains(&"WorkflowCompensationInitiated"));
    if expected_count >= 5 {
        assert!(event_types.contains(&"WorkflowTerminated"));
    }
    assert_query_projection_routes(client, base_url, expected_count).await
}

async fn assert_status_and_list_survive_restart(
    client: &reqwest::Client,
    base_url: &str,
) -> TestResult {
    let workflow_url = format!("{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}");
    let status = get_json(client, &workflow_url).await?;
    assert_eq!(status["instance_id"], INSTANCE_ID);
    assert_eq!(status["namespace"], "payments");
    assert_eq!(status["events_applied"], 4);

    let active = get_json(client, &format!("{base_url}/api/v1/workflows")).await?;
    let active_items = active.as_array().ok_or("list response must be an array")?;
    assert!(active_items
        .iter()
        .any(|item| item["namespace"] == "payments" && item["instance_id"] == INSTANCE_ID));
    Ok(())
}

async fn assert_same_instance_id_in_two_namespaces_isolated(
    client: &reqwest::Client,
    base_url: &str,
) -> TestResult {
    let response = client
        .post(format!("{base_url}/api/v1/workflows"))
        .json(&workflow_start_body_for("billing", "dedupe-e2e-start-002"))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);

    let payments = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}"),
    )
    .await?;
    let billing = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/billing%2F{INSTANCE_ID}"),
    )
    .await?;
    assert_eq!(payments["namespace"], "payments");
    assert_eq!(billing["namespace"], "billing");

    let signal_body =
        serde_json::json!({"signal_name": "namespace-check", "payload": {"ok": true}});
    let payments_signal = client
        .post(format!(
            "{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}/signals"
        ))
        .json(&signal_body)
        .send()
        .await?;
    assert_eq!(payments_signal.status(), reqwest::StatusCode::ACCEPTED);
    let billing_signal = client
        .post(format!(
            "{base_url}/api/v1/workflows/billing%2F{INSTANCE_ID}/signals"
        ))
        .json(&signal_body)
        .send()
        .await?;
    assert_eq!(billing_signal.status(), reqwest::StatusCode::ACCEPTED);

    let billing_events = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/billing/{INSTANCE_ID}/events"),
    )
    .await?;
    assert!(billing_events["events"]
        .as_array()
        .ok_or("billing events array")?
        .iter()
        .any(|event| event["payload"]["type"] == "SignalAccepted"
            && event["payload"]["namespace"] == "billing"));
    Ok(())
}

async fn assert_terminal_status_survives_restart(
    client: &reqwest::Client,
    base_url: &str,
) -> TestResult {
    let workflow_url = format!("{base_url}/api/v1/workflows/payments%2F{INSTANCE_ID}");
    let detail = get_json(client, &workflow_url).await?;
    assert_eq!(detail["phase"], "terminated");
    assert_eq!(detail["events_applied"], 5);
    let status = get_json(client, &format!("{workflow_url}/status")).await?;
    assert_eq!(status["phase"], "terminated");
    assert_eq!(status["namespace"], "payments");
    assert_lifecycle_events_visible(client, base_url, 5).await
}

async fn assert_billing_remains_live_after_payments_termination(
    client: &reqwest::Client,
    base_url: &str,
) -> TestResult {
    let billing = get_json(
        client,
        &format!("{base_url}/api/v1/workflows/billing%2F{INSTANCE_ID}"),
    )
    .await?;
    assert_eq!(billing["namespace"], "billing");
    assert_eq!(billing["phase"], "live");
    Ok(())
}

async fn get_json(client: &reqwest::Client, url: &str) -> TestResult<serde_json::Value> {
    let response = client.get(url).send().await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK, "GET {url}");
    Ok(response.json().await?)
}

async fn wait_for_ui(client: &reqwest::Client, base_url: &str) -> TestResult {
    for _ in 0..40 {
        if ui_is_ready(client, base_url).await? {
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }
    Err("server did not expose /wtf/ui before timeout".into())
}

async fn ui_is_ready(client: &reqwest::Client, base_url: &str) -> TestResult<bool> {
    let response = client.get(format!("{base_url}/wtf/ui")).send().await;
    match response {
        Ok(response) => Ok(response.status() == reqwest::StatusCode::OK),
        Err(_) => Ok(false),
    }
}

fn workflow_start_body() -> serde_json::Value {
    workflow_start_body_for("payments", "dedupe-e2e-start-001")
}

fn workflow_start_body_for(namespace: &str, dedupe_key: &str) -> serde_json::Value {
    serde_json::json!({
        "namespace": namespace,
        "workflow_type": "checkout",
        "paradigm": "fsm",
        "instance_id": INSTANCE_ID,
        "dedupe_key": dedupe_key,
        "workflow_binary_hash": WORKFLOW_HASH,
        "input": {}
    })
}
