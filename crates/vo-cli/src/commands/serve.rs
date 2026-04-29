use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ServeError {
    #[error("invalid host: {0}")]
    InvalidHost(String),
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("invalid storage path: {0}")]
    InvalidStoragePath(String),
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub host: String,
    pub port: u16,
    pub storage_path: PathBuf,
}

pub fn validate_serve_config(config: &ServeConfig) -> Result<(), ServeError> {
    if config.host.is_empty() {
        return Err(ServeError::InvalidHost(
            "host must not be empty".to_string(),
        ));
    }
    if config.port == 0 {
        return Err(ServeError::InvalidPort(
            "port must be greater than 0".to_string(),
        ));
    }
    if config.storage_path.exists() && !config.storage_path.is_dir() {
        return Err(ServeError::InvalidStoragePath(format!(
            "storage path is not a directory: {}",
            config.storage_path.display()
        )));
    }
    Ok(())
}

pub async fn run_serve(config: &ServeConfig) -> Result<(), ServeError> {
    let listen_addr = format!("{}:{}", config.host, config.port);
    let listener = match tokio::net::TcpListener::bind(&listen_addr).await {
        Ok(l) => l,
        Err(e) => {
            return Err(ServeError::InvalidHost(format!(
                "Failed to bind to {listen_addr}: {e}"
            )))
        }
    };

    run_serve_until_shutdown(config, listener, shutdown_signal()).await
}

pub async fn run_serve_until_shutdown<S>(
    config: &ServeConfig,
    listener: tokio::net::TcpListener,
    shutdown: S,
) -> Result<(), ServeError>
where
    S: Future<Output = ()> + Send + 'static,
{
    validate_serve_config(config)?;

    std::fs::create_dir_all(&config.storage_path).map_err(|e| {
        ServeError::InvalidStoragePath(format!(
            "failed to create storage directory {}: {e}",
            config.storage_path.display()
        ))
    })?;

    let db = fjall::Database::builder(&config.storage_path)
        .open()
        .map_err(|e| ServeError::InvalidStoragePath(format!("Failed to open Fjall DB: {e}")))?;

    let dedupe_store = Arc::new(
        vo_storage::dedupe_partition::FjallDedupeStore::open(&db).map_err(|e| {
            ServeError::InvalidStoragePath(format!("failed to open Fjall dedupe store: {e}"))
        })?,
    );

    let initial_instances = rehydrate_instances(&db)?;
    let db_handle = Arc::new(db);

    let workspace_index = Arc::new(std::sync::RwLock::new(
        vo_types::workspace::WorkspaceIndex::new(),
    ));

    let search_engine =
        std::sync::Arc::new(std::sync::RwLock::new(vo_types::search::SearchEngine::new()));

    let query =
        vo_api::handlers::QueryState::new(db_handle.clone(), workspace_index, search_engine);

    let sse = vo_api::handlers::SseState::new();
    let ws = vo_api::handlers::WsState::new();

    let listen_addr = listener.local_addr().map_or_else(
        |_| format!("{}:{}", config.host, config.port),
        |addr| addr.to_string(),
    );

    let (master_ref, _master_handle) = ractor::Actor::spawn(
        Some(format!("api-orchestrator-{listen_addr}")),
        vo_actor::MasterOrchestrator,
        vo_actor::OrchestratorConfig {
            initial_instances,
            ..vo_actor::OrchestratorConfig::default()
        },
    )
    .await
    .map_err(|e| ServeError::InvalidStoragePath(format!("Failed to spawn orchestrator: {e}")))?;
    let master = Arc::new(master_ref);

    let circuit_breaker = Arc::new(vo_core::circuit_breaker::CircuitBreakerState::new());
    let writer_pressure = Arc::new(vo_core::admission::WatchdogPressureGuard::permissive());

    let state = vo_api::router::AppState {
        query,
        sse,
        ws,
        master,
        circuit_breaker,
        dedupe_store,
        writer_pressure,
    };

    let router = vo_api::router::create_router(state);

    println!(
        "Starting veloxide server on {} with storage at {}",
        listen_addr,
        config.storage_path.display()
    );

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| ServeError::InvalidHost(format!("Server error: {e}")))?;

    Ok(())
}

fn rehydrate_instances(
    db: &fjall::Database,
) -> Result<Vec<vo_actor::InstanceSnapshot>, ServeError> {
    vo_storage::event_log::replay_all_events(db)
        .map_err(|error| {
            ServeError::InvalidStoragePath(format!("failed to replay workflow events: {error}"))
        })
        .map(|events| {
            events
                .into_iter()
                .fold(HashMap::new(), |mut active, envelope| {
                    apply_event(&mut active, envelope);
                    active
                })
                .into_values()
                .collect()
        })
}

fn apply_event(
    active: &mut HashMap<(String, String), vo_actor::InstanceSnapshot>,
    envelope: vo_types::EventEnvelope,
) {
    let event_type = envelope
        .payload
        .get("type")
        .and_then(serde_json::Value::as_str);
    match event_type {
        Some("WorkflowStarted") => apply_started(active, envelope),
        Some("WorkflowTerminated") => apply_terminated(active, envelope),
        Some("SignalAccepted") | Some("WorkflowCompensationInitiated") => {
            increment_event_count(active, envelope)
        }
        _ => {}
    }
}

fn apply_started(
    active: &mut HashMap<(String, String), vo_actor::InstanceSnapshot>,
    envelope: vo_types::EventEnvelope,
) {
    let namespace = payload_namespace(&envelope);
    let instance_id = vo_types::InstanceId::parse(&envelope.instance_id);
    if let (Some(namespace), Ok(instance_id)) = (namespace, instance_id) {
        let workflow_type = envelope
            .payload
            .get("workflow_type")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| "unknown".to_string(), ToString::to_string);
        let paradigm = envelope
            .payload
            .get("paradigm")
            .and_then(serde_json::Value::as_str)
            .map_or(vo_actor::WorkflowParadigm::Procedural, parse_paradigm);
        active.insert(
            (namespace.clone(), envelope.instance_id.clone()),
            vo_actor::InstanceSnapshot {
                instance_id,
                namespace,
                workflow_type,
                paradigm,
                phase: vo_actor::InstancePhaseView::Live,
                events_applied: envelope.sequence,
            },
        );
    }
}

fn apply_terminated(
    active: &mut HashMap<(String, String), vo_actor::InstanceSnapshot>,
    envelope: vo_types::EventEnvelope,
) {
    if let Some(namespace) = payload_namespace(&envelope) {
        active.remove(&(namespace, envelope.instance_id));
    }
}

fn increment_event_count(
    active: &mut HashMap<(String, String), vo_actor::InstanceSnapshot>,
    envelope: vo_types::EventEnvelope,
) {
    if let Some(namespace) = payload_namespace(&envelope) {
        active
            .entry((namespace.clone(), envelope.instance_id.clone()))
            .and_modify(|snapshot| snapshot.events_applied = envelope.sequence);
    }
}

fn payload_namespace(envelope: &vo_types::EventEnvelope) -> Option<String> {
    envelope
        .payload
        .get("namespace")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            envelope
                .metadata
                .annotations
                .get("namespace")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

fn parse_paradigm(value: &str) -> vo_actor::WorkflowParadigm {
    match value {
        "fsm" => vo_actor::WorkflowParadigm::Fsm,
        "dag" => vo_actor::WorkflowParadigm::Dag,
        _ => vo_actor::WorkflowParadigm::Procedural,
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tokio::time::{timeout, Duration};

    fn find_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("bind")
            .local_addr()
            .unwrap()
            .port()
    }

    /// BDD: Given a configured local Fjall database path and API bind address,
    ///      When the operator runs the CLI serve command,
    ///      Then Axum starts with real AppState and the command does not exit
    ///      after only printing validation text.
    #[tokio::test]
    async fn given_valid_config_when_cli_serve_runs_then_axum_boots() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let port = find_available_port();
        let config = ServeConfig {
            host: "127.0.0.1".to_string(),
            port,
            storage_path: tmp.path().to_path_buf(),
        };

        let config_clone = config.clone();
        let handle = tokio::spawn(async move { run_serve(&config_clone).await });

        let result = timeout(Duration::from_secs(10), async {
            // Try to connect to verify the server is actually listening
            for _ in 0..20 {
                match tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await {
                    Ok(_) => return true, // Server is up
                    Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
                }
            }
            false
        })
        .await;

        // The server should have started within 3 seconds
        assert!(
            result.is_ok() && result.unwrap(),
            "Axum server did not boot in time"
        );

        // Gracefully shutdown by sending ctrl-c signal simulation
        handle.abort();
    }
}
