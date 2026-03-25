//! `wtf serve` — run the wtf-engine server.
//! Provision streams and buckets, then start the NATS JetStream context.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use async_nats::jetstream::kv::Store;
use futures::StreamExt;
use ractor::actor::Actor;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig, WorkflowDefinition};
use wtf_api::app::{build_app, serve as serve_api};
use wtf_storage::{connect, open_snapshot_db, provision_kv_buckets, provision_streams, NatsConfig};
use wtf_actor::heartbeat::run_heartbeat_watcher;
use wtf_worker::timer::run_timer_loop;
use wtf_worker::Worker;

/// Configuration for the `serve` command.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub port: u16,
    pub nats_url: String,
    pub embedded_nats: bool,
    pub data_dir: PathBuf,
    pub max_concurrent: usize,
}

impl From<ServeConfig> for NatsConfig {
    fn from(cfg: ServeConfig) -> Self {
        Self {
            urls: vec![cfg.nats_url],
            embedded: cfg.embedded_nats,
            ..Default::default()
        }
    }
}

/// Run the `serve` command.
///
/// Establishes NATS connection, provisions storage, starts orchestrator + API,
/// and blocks until shutdown signal.
pub async fn run_serve(config: ServeConfig) -> anyhow::Result<()> {
    let snapshot_db_path = config.data_dir.join("snapshots.db");
    let snapshot_db = open_snapshot_db(&snapshot_db_path).context("failed to open snapshot db")?;

    let nats_config = NatsConfig::from(config.clone());

    let nats = connect(&nats_config)
        .await
        .context("failed to connect to NATS")?;

    let kv = provision_storage(&nats).await?;

    let definitions = load_definitions_from_kv(&kv.definitions)
        .await
        .context("failed to load definitions from KV")?;

    let event_store = Arc::new(nats.clone());
    let state_store = Arc::new(nats.clone());
    let task_queue = Arc::new(nats.clone());

    let orchestrator_config = OrchestratorConfig {
        max_instances: config.max_concurrent,
        engine_node_id: "engine-local".to_owned(),
        snapshot_db: Some(snapshot_db),
        event_store: Some(event_store),
        state_store: Some(state_store),
        task_queue: Some(task_queue),
        definitions,
        procedural_workflows: Vec::new(),
    };

    let (master, _master_handle) = MasterOrchestrator::spawn(
        Some("master-orchestrator".to_owned()),
        MasterOrchestrator,
        orchestrator_config,
    )
    .await
    .context("failed to start MasterOrchestrator")?;

    let app = build_app(master.clone(), kv.clone());
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let api_shutdown = shutdown_rx.clone();
    let heartbeat_shutdown = shutdown_rx.clone();
    let timer_shutdown = shutdown_rx.clone();
    let worker_shutdown = shutdown_rx;

    let api_task = tokio::spawn(async move { serve_api(addr, app, api_shutdown).await });
    let timer_task = tokio::spawn(run_timer_loop(
        nats.jetstream().clone(),
        kv.timers.clone(),
        timer_shutdown,
    ));
    let heartbeat_task = tokio::spawn(run_heartbeat_watcher(
        kv.heartbeats.clone(),
        master.clone(),
        heartbeat_shutdown,
    ));
    let worker = Worker::new(nats.jetstream().clone(), "builtin-worker", None);
    let worker_task = tokio::spawn(async move {
        worker.run(worker_shutdown).await
    });

    wait_for_shutdown_signal().await;
    drain_runtime(
        shutdown_tx,
        api_task,
        timer_task,
        heartbeat_task,
        worker_task,
        || master.stop(None),
    )
    .await?;

    Ok(())
}

/// Scan all keys in the definitions KV bucket, deserialize each as [`WorkflowDefinition`],
/// and return the successfully parsed entries.
///
/// Malformed entries are logged at `warn` level and skipped. An empty bucket
/// is valid — an info message is logged and an empty vec is returned.
///
/// # Errors
/// Returns an error if the KV scan itself fails (NATS connection issue).
async fn load_definitions_from_kv(
    store: &Store,
) -> anyhow::Result<Vec<(String, WorkflowDefinition)>> {
    let mut keys = store
        .keys()
        .await
        .context("failed to scan definition keys from KV")?;

    let mut definitions = Vec::new();

    while let Some(key_result) = keys.next().await {
        let Ok(key) = key_result else {
            continue;
        };
        let Ok(Some(value)) = store.get(&key).await else {
            continue;
        };
        match serde_json::from_slice::<WorkflowDefinition>(value.as_ref()) {
            Ok(def) => definitions.push((key, def)),
            Err(e) => tracing::warn!(key = %key, error = %e, "skipping malformed definition in KV"),
        }
    }

    if definitions.is_empty() {
        tracing::info!("No workflow definitions found in KV");
    } else {
        tracing::info!(count = definitions.len(), "Loaded workflow definitions from KV");
    }

    Ok(definitions)
}

async fn drain_runtime<EApi, ETimer, EWorker, FStop>(
    shutdown_tx: watch::Sender<bool>,
    api_task: JoinHandle<Result<(), EApi>>,
    timer_task: JoinHandle<Result<(), ETimer>>,
    heartbeat_task: JoinHandle<Result<(), String>>,
    worker_task: JoinHandle<Result<(), EWorker>>,
    stop_master: FStop,
) -> anyhow::Result<()>
where
    EApi: std::error::Error + Send + Sync + 'static,
    ETimer: std::error::Error + Send + Sync + 'static,
    EWorker: std::error::Error + Send + Sync + 'static,
    FStop: FnOnce(),
{
    let _ = shutdown_tx.send(true);

    let api_result: Result<(), EApi> = api_task.await.context("api task join failed")?;
    let timer_result: Result<(), ETimer> = timer_task.await.context("timer task join failed")?;
    let heartbeat_result = heartbeat_task.await.context("heartbeat watcher task join failed")?;
    let worker_result: Result<(), EWorker> =
        worker_task.await.context("worker task join failed")?;

    stop_master();

    api_result.context("api server failed")?;
    timer_result.context("timer loop failed")?;
    heartbeat_result
        .map_err(|e| anyhow::anyhow!("heartbeat watcher failed: {e}"))?;
    worker_result.context("builtin worker failed")?;

    Ok(())
}

async fn provision_storage(
    nats: &wtf_storage::NatsClient,
) -> anyhow::Result<wtf_storage::KvStores> {
    provision_streams(nats.jetstream())
        .await
        .context("failed to provision JetStream streams")?;

    provision_kv_buckets(nats.jetstream())
        .await
        .context("failed to provision KV buckets")
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).ok();

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(sig) = sigterm.as_mut() {
                    let _ = sig.recv().await;
                }
            } => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
