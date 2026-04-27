use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};
use vo_cli::{run_serve_until_shutdown, ServeConfig};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const WORKFLOW_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INSTANCE_ID: &str = "01H5JYV4XHGSR2F8KZ9BWNRFMA";

#[tokio::test]
async fn given_serve_command_when_workflow_started_then_events_replay_from_live_server(
) -> TestResult {
    let temp = tempfile::tempdir()?;
    let storage_path = temp.path().join("fjall");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let config = ServeConfig {
        host: "127.0.0.1".to_string(),
        port: addr.port(),
        storage_path,
    };

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server_config = config.clone();
    let server = tokio::spawn(async move {
        run_serve_until_shutdown(&server_config, listener, async {
            let _ = shutdown_rx.await;
        })
        .await
    });

    let client = reqwest::Client::new();
    let base_url = format!("http://{addr}");
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

    let _ = shutdown_tx.send(());
    server.await??;
    Ok(())
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
    serde_json::json!({
        "namespace": "payments",
        "workflow_type": "checkout",
        "paradigm": "fsm",
        "instance_id": INSTANCE_ID,
        "dedupe_key": "dedupe-e2e-start-001",
        "input": {
            "workflow_binary_hash": WORKFLOW_HASH
        }
    })
}
