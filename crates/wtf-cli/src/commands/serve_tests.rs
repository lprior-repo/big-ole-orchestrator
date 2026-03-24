//! Tests for `wtf serve` command (extracted from serve.rs for <300 line limit).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::watch;

use super::drain_runtime;

fn make_drained_task(
    mut rx: watch::Receiver<bool>,
    drained: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move {
        let changed = rx.changed().await;
        if changed.is_ok() {
            drained.store(true, Ordering::SeqCst);
        }
        Result::<(), std::io::Error>::Ok(())
    })
}

#[tokio::test]
async fn drain_runtime_signals_shutdown_and_waits_for_four_tasks() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let api_drained = Arc::new(AtomicBool::new(false));
    let timer_drained = Arc::new(AtomicBool::new(false));
    let heartbeat_drained = Arc::new(AtomicBool::new(false));
    let worker_drained = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));

    let api_handle = make_drained_task(shutdown_rx.clone(), Arc::clone(&api_drained));
    let timer_handle = make_drained_task(shutdown_rx.clone(), Arc::clone(&timer_drained));

    let heartbeat_handle = {
        let mut rx = shutdown_rx.clone();
        let drained = Arc::clone(&heartbeat_drained);
        tokio::spawn(async move {
            let changed = rx.changed().await;
            if changed.is_ok() {
                drained.store(true, Ordering::SeqCst);
            }
            Result::<(), String>::Ok(())
        })
    };

    let worker_handle = {
        let mut rx = shutdown_rx;
        let drained = Arc::clone(&worker_drained);
        tokio::spawn(async move {
            let changed = rx.changed().await;
            if changed.is_ok() {
                drained.store(true, Ordering::SeqCst);
            }
            Result::<(), std::io::Error>::Ok(())
        })
    };

    let drain_result = drain_runtime(
        shutdown_tx,
        api_handle,
        timer_handle,
        heartbeat_handle,
        worker_handle,
        {
            let stopped = Arc::clone(&stopped);
            move || {
                stopped.store(true, Ordering::SeqCst);
            }
        },
    )
    .await;

    assert!(drain_result.is_ok());
    assert!(api_drained.load(Ordering::SeqCst));
    assert!(timer_drained.load(Ordering::SeqCst));
    assert!(heartbeat_drained.load(Ordering::SeqCst));
    assert!(worker_drained.load(Ordering::SeqCst));
    assert!(stopped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn drain_runtime_propagates_worker_error() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut rx1 = shutdown_rx.clone();
    let mut rx2 = shutdown_rx.clone();
    let mut rx3 = shutdown_rx.clone();
    let mut rx4 = shutdown_rx;

    let api = tokio::spawn(async move {
        let _ = rx1.changed().await;
        Ok::<(), std::io::Error>(())
    });
    let timer = tokio::spawn(async move {
        let _ = rx2.changed().await;
        Ok::<(), std::io::Error>(())
    });
    let heartbeat = tokio::spawn(async move {
        let _ = rx3.changed().await;
        Ok::<(), String>(())
    });
    let worker = tokio::spawn(async move {
        let _ = rx4.changed().await;
        Err::<(), std::io::Error>(std::io::Error::other(
            "worker boom",
        ))
    });

    let result = drain_runtime(shutdown_tx, api, timer, heartbeat, worker, || {}).await;
    assert!(result.is_err());
    let err = result.expect_err("already asserted is_err");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("builtin worker failed"),
        "expected 'builtin worker failed', got: {err_msg}"
    );
    // The underlying io::Error is in the anyhow chain
    let chain_contains_worker_boom = err
        .chain()
        .skip(1)
        .any(|e| e.to_string().contains("worker boom"));
    assert!(
        chain_contains_worker_boom,
        "expected 'worker boom' in error chain, got: {err_msg}"
    );
}
