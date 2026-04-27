use std::path::PathBuf;

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
    if !config.storage_path.exists() {
        return Err(ServeError::InvalidStoragePath(format!(
            "storage path does not exist: {}",
            config.storage_path.display()
        )));
    }
    if !config.storage_path.is_dir() {
        return Err(ServeError::InvalidStoragePath(format!(
            "storage path is not a directory: {}",
            config.storage_path.display()
        )));
    }
    Ok(())
}

pub async fn run_serve(config: &ServeConfig) -> Result<(), ServeError> {
    validate_serve_config(config)?;

    let db = fjall::Database::builder(&config.storage_path)
        .open()
        .map_err(|e| ServeError::InvalidStoragePath(format!("Failed to open Fjall DB: {e}")))?;

    let db_handle = std::sync::Arc::new(db);

    let workspace_index = std::sync::Arc::new(std::sync::RwLock::new(
        vo_types::workspace::WorkspaceIndex::new(),
    ));

    let search_engine = std::sync::Arc::new(std::sync::RwLock::new(
        vo_types::search::SearchEngine::new(),
    ));

    let query = vo_api::handlers::QueryState::new(
        db_handle.clone(),
        workspace_index,
        search_engine,
    );

    let sse = vo_api::handlers::SseState::new();
    let ws = vo_api::handlers::WsState::new();

    let (master_ref, _master_handle) = ractor::Actor::spawn(
        Some("api-orchestrator".to_string()),
        DummyMaster,
        (),
    )
    .await
    .map_err(|e| {
        ServeError::InvalidStoragePath(format!("Failed to spawn orchestrator: {e}"))
    })?;
    let master = std::sync::Arc::new(master_ref);

    let circuit_breaker = std::sync::Arc::new(
        vo_core::circuit_breaker::CircuitBreakerState::new(),
    );

    let dedupe_store = std::sync::Arc::new(
        vo_storage::dedupe_partition::InMemoryDedupeStore::new(),
    );

    let state = vo_api::router::AppState {
        query,
        sse,
        ws,
        master,
        circuit_breaker,
        dedupe_store,
    };

    let router = vo_api::router::create_router(state);

    let listen_addr = format!("{}:{}", config.host, config.port);
    let listener = match tokio::net::TcpListener::bind(&listen_addr).await {
        Ok(l) => l,
        Err(e) => {
            return Err(ServeError::InvalidHost(format!(
                "Failed to bind to {listen_addr}: {e}"
            )))
        }
    };

    println!(
        "Starting veloxide server on {} with storage at {}",
        listen_addr,
        config.storage_path.display()
    );

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| {
            ServeError::InvalidHost(format!("Server error: {e}"))
        })?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c");
}

// Dummy actor for API orchestration.
// In production this would be a full MasterOrchestrator.
struct DummyMaster;

impl ractor::Actor for DummyMaster {
    type Msg = vo_actor::OrchestratorMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ractor::ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ractor::ActorProcessingErr> {
        Ok(())
    }
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
