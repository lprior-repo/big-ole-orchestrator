//! `wtf serve` — run the wtf-engine server.
//! Provision streams and buckets, then start the NATS JetStream context.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use ractor::actor::Actor;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use wtf_actor::master::{MasterOrchestrator, OrchestratorConfig};
use wtf_api::app::{build_app, serve as serve_api};
use wtf_storage::{connect, open_snapshot_db, provision_kv_buckets, provision_streams, NatsConfig};
use wtf_worker::timer::run_timer_loop;

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
    let timer_shutdown = shutdown_rx;

    let api_task = tokio::spawn(async move { serve_api(addr, app, api_shutdown).await });
    let timer_task = tokio::spawn(run_timer_loop(
        nats.jetstream().clone(),
        kv.timers.clone(),
        timer_shutdown,
    ));

    wait_for_shutdown_signal().await;
    drain_runtime(shutdown_tx, api_task, timer_task, || master.stop(None)).await?;

    Ok(())
}

async fn drain_runtime<EApi, ETimer, FStop>(
    shutdown_tx: watch::Sender<bool>,
    api_task: JoinHandle<Result<(), EApi>>,
    timer_task: JoinHandle<Result<(), ETimer>>,
    stop_master: FStop,
) -> anyhow::Result<()>
where
    EApi: std::error::Error + Send + Sync + 'static,
    ETimer: std::error::Error + Send + Sync + 'static,
    FStop: FnOnce(),
{
    let _ = shutdown_tx.send(true);

    let api_result: Result<(), EApi> = api_task.await.context("api task join failed")?;
    let timer_result: Result<(), ETimer> = timer_task.await.context("timer task join failed")?;

    stop_master();

    api_result.context("api server failed")?;
    timer_result.context("timer loop failed")?;

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
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::drain_runtime;
    use tokio::sync::watch;

    #[tokio::test]
    async fn drain_runtime_signals_shutdown_and_waits_for_tasks() {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let api_drained = Arc::new(AtomicBool::new(false));
        let timer_drained = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));

        let api_handle = {
            let mut rx = shutdown_rx.clone();
            let drained = Arc::clone(&api_drained);
            tokio::spawn(async move {
                let changed = rx.changed().await;
                if changed.is_ok() {
                    drained.store(true, Ordering::SeqCst);
                }
                Result::<(), std::io::Error>::Ok(())
            })
        };

        let timer_handle = {
            let mut rx = shutdown_rx;
            let drained = Arc::clone(&timer_drained);
            tokio::spawn(async move {
                let changed = rx.changed().await;
                if changed.is_ok() {
                    drained.store(true, Ordering::SeqCst);
                }
                Result::<(), std::io::Error>::Ok(())
            })
        };

        let drain_result = drain_runtime(shutdown_tx, api_handle, timer_handle, {
            let stopped = Arc::clone(&stopped);
            move || {
                stopped.store(true, Ordering::SeqCst);
            }
        })
        .await;

        assert!(drain_result.is_ok());
        assert!(api_drained.load(Ordering::SeqCst));
        assert!(timer_drained.load(Ordering::SeqCst));
        assert!(stopped.load(Ordering::SeqCst));
    }
}
