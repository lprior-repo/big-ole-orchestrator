//! `wtf serve` — run the vo-engine server.
//! This module implements the run loop bead (vo-qz46).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use ractor::Actor;
use tokio::sync::watch;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use vo_api::app::{build_app, serve};
use vo_actor::heartbeat::run_heartbeat_watcher;
use vo_actor::master::{MasterOrchestrator, OrchestratorConfig};
use vo_storage::{
    connect, open_snapshot_db, provision_kv_buckets, provision_streams, NatsClient, NatsConfig,
};

const SHUTDOWN_TIMEOUT_SECS: u64 = 30;

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

pub async fn run_serve(config: ServeConfig) -> anyhow::Result<NatsClient> {
    let nats_config = NatsConfig::from(config.clone());

    let nats = connect(&nats_config)
        .await
        .context("failed to connect to NATS")?;

    provision_storage(&nats).await?;

    Ok(nats)
}

pub async fn run_serve_loop(
    config: ServeConfig,
    nats: NatsClient,
) -> anyhow::Result<std::process::ExitCode> {
    tracing::info!("opening sled database at {:?}", config.data_dir);
    let sled_db = open_snapshot_db(&config.data_dir)
        .map_err(|e| anyhow::anyhow!("failed to open sled DB: {e}"))?;

    let kv = provision_kv_buckets(nats.jetstream())
        .await
        .context("failed to provision KV buckets")?;

    let engine_node_id = format!(
        "engine-{}",
        Uuid::new_v4().to_string().split('-').next().unwrap_or("local")
    );

    let orch_config = OrchestratorConfig {
        max_instances: config.max_concurrent,
        engine_node_id: engine_node_id.clone(),
        snapshot_db: Some(sled_db.clone()),
        event_store: Some(Arc::new(nats.clone())),
        state_store: Some(Arc::new(nats.clone())),
        task_queue: Some(Arc::new(nats.clone())),
    };

    tracing::info!("spawning MasterOrchestrator actor");
    let (orchestrator_ref, orchestrator_handle) = MasterOrchestrator::spawn(
        Some(engine_node_id),
        MasterOrchestrator,
        orch_config,
    )
    .await
    .context("failed to spawn MasterOrchestrator")?;

    let app = build_app(orchestrator_ref.clone(), kv.clone());

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    tracing::info!("binding TCP listener on {}", addr);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let heartbeat_handle = tokio::spawn({
        let orchestrator_ref = orchestrator_ref.clone();
        let heartbeats = kv.heartbeats.clone();
        let rx = shutdown_rx.clone();
        async move {
            if let Err(e) = run_heartbeat_watcher(heartbeats, orchestrator_ref, rx).await {
                tracing::warn!("heartbeat watcher error: {e}");
            }
        }
    });

    let shutdown_tx_for_signal = shutdown_tx.clone();
    let signal_handle = tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("received shutdown signal (Ctrl+C)");
        shutdown_tx_for_signal.send(true).unwrap();
    });

    tracing::info!("starting API server on {}", addr);
    let server_result = serve(addr, app, shutdown_rx).await;
    if let Err(e) = server_result {
        tracing::error!("axum server error: {e}");
    }

    tracing::info!("initiating shutdown sequence");

    shutdown_tx.send(true).unwrap();

    let heartbeat_result = timeout(Duration::from_secs(5), heartbeat_handle).await;

    match heartbeat_result {
        Err(_) => tracing::warn!("heartbeat watcher did not stop within 5s, force-killing"),
        Ok(Err(e)) => tracing::warn!("heartbeat watcher error on join: {e}"),
        Ok(Ok(())) => {}
    }

    drop(signal_handle);

    tracing::info!("flushing sled snapshots");
    if let Err(e) = sled_db.flush() {
        tracing::warn!("sled flush warning: {e}");
    }

    tracing::info!("stopping MasterOrchestrator actor");
    let stop_result = timeout(Duration::from_secs(SHUTDOWN_TIMEOUT_SECS), orchestrator_handle).await;

    match stop_result {
        Ok(Ok(())) => tracing::info!("MasterOrchestrator stopped cleanly"),
        Ok(Err(e)) => tracing::error!("MasterOrchestrator error on stop: {e}"),
        Err(_) => tracing::error!(
            "MasterOrchestrator did not stop within {}s timeout",
            SHUTDOWN_TIMEOUT_SECS
        ),
    }

    tracing::info!("closing NATS connection");
    drop(nats);

    tracing::info!("shutdown complete");
    Ok(std::process::ExitCode::SUCCESS)
}

async fn provision_storage(nats: &NatsClient) -> anyhow::Result<()> {
    provision_streams(nats.jetstream())
        .await
        .context("failed to provision JetStream streams")?;

    provision_kv_buckets(nats.jetstream())
        .await
        .context("failed to provision KV buckets")?;

    Ok(())
}