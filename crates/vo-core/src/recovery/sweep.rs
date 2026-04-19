//! Orphan detection sweep mechanism.
//!
//! Periodically sweeps the storage layer to detect orphan processes that need
//! recovery. An orphan is a workflow instance that is in a failed state but
//! has not been processed for recovery.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

use super::{OrphanProcess, RecoveryError, RecoveryMetrics};

pub trait OrphanQuery: Send + Sync {
    fn query_orphans(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<OrphanProcess>, String>> + Send;
}

pub struct OrphanDetector<Q> {
    sweep_interval: Duration,
    query: Q,
}

impl<Q> OrphanDetector<Q>
where
    Q: OrphanQuery,
{
    pub fn new(sweep_interval: Duration, query: Q) -> Self {
        Self {
            sweep_interval,
            query,
        }
    }

    pub async fn run(self, tx: mpsc::Sender<OrphanProcess>) {
        let mut interval = interval(self.sweep_interval);
        loop {
            interval.tick().await;
            match self.query.query_orphans().await {
                Ok(orphans) => {
                    for orphan in orphans {
                        if tx.send(orphan).await.is_err() {
                            return;
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    /// Run a single sweep with a timeout.
    ///
    /// # Errors
    ///
    /// Returns `RecoveryError::SweepTimeout` if the sweep exceeds the deadline.
    pub async fn run_with_timeout(
        &self,
        tx: mpsc::Sender<OrphanProcess>,
        deadline: Duration,
    ) -> Result<(), RecoveryError> {
        use tokio::time::timeout;
        let result = timeout(deadline, self.run_single_sweep_impl(tx)).await;
        match result {
            Ok(r) => r,
            Err(_) => Err(RecoveryError::SweepTimeout { elapsed: deadline }),
        }
    }

    /// Run a single sweep with a batch limit.
    ///
    /// # Errors
    ///
    /// Returns `RecoveryError::SweepChannelClosed` if the channel is closed.
    pub async fn run_with_batch_limit(
        &self,
        tx: mpsc::Sender<OrphanProcess>,
        batch_limit: usize,
    ) -> Result<(), RecoveryError> {
        let orphans = self.query.query_orphans().await?;
        let to_send = orphans.into_iter().take(batch_limit);
        for orphan in to_send {
            tx.send(orphan)
                .await
                .map_err(|_| RecoveryError::SweepChannelClosed)?;
        }
        Ok(())
    }

    /// Run a single sweep and collect metrics.
    ///
    /// Returns `RecoveryMetrics` with detection and enqueue statistics.
    pub async fn run_and_collect_metrics(
        &self,
        tx: mpsc::Sender<OrphanProcess>,
    ) -> RecoveryMetrics {
        let start = tokio::time::Instant::now();
        let mut detected = 0usize;
        let mut enqueued = 0usize;
        let mut rejected = 0usize;

        match self.query.query_orphans().await {
            Ok(orphans) => {
                detected = orphans.len();
                for orphan in orphans {
                    if tx.send(orphan).await.is_ok() {
                        enqueued += 1;
                    } else {
                        rejected += 1;
                    }
                }
            }
            Err(_) => {
                // Query failure doesn't count as detected
            }
        }

        let elapsed = start.elapsed();

        RecoveryMetrics {
            detected,
            enqueued,
            rejected,
            elapsed,
        }
    }

    /// Run a single sweep operation.
    ///
    /// # Errors
    ///
    /// Returns `RecoveryError::SweepChannelClosed` if the channel is closed,
    /// or query errors from the underlying query implementation.
    pub async fn run_single_sweep(
        &self,
        tx: mpsc::Sender<OrphanProcess>,
    ) -> Result<(), RecoveryError> {
        self.run_single_sweep_impl(tx).await
    }

    async fn run_single_sweep_impl(
        &self,
        tx: mpsc::Sender<OrphanProcess>,
    ) -> Result<(), RecoveryError> {
        let orphans = self.query.query_orphans().await?;
        for orphan in orphans {
            tx.send(orphan)
                .await
                .map_err(|_| RecoveryError::SweepChannelClosed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::sleep;

    struct MockQuery {
        counter: Arc<AtomicUsize>,
    }

    impl MockQuery {
        fn new(counter: Arc<AtomicUsize>) -> Self {
            Self { counter }
        }
    }

    impl OrphanQuery for MockQuery {
        async fn query_orphans(&self) -> Result<Vec<OrphanProcess>, String> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(vec![OrphanProcess {
                instance_id: "test".to_string(),
                lineage_id: "lineage-1".to_string(),
                failed_at: Duration::from_secs(0),
            }])
        }
    }

    #[tokio::test]
    async fn detector_runs_query_on_interval() {
        let counter = Arc::new(AtomicUsize::new(0));
        let query = MockQuery::new(counter.clone());
        let (tx, _rx) = mpsc::channel(10);

        let detector = OrphanDetector::new(Duration::from_millis(20), query);

        tokio::spawn(async move {
            detector.run(tx).await;
        });

        sleep(Duration::from_millis(100)).await;

        let count = counter.load(Ordering::SeqCst);
        assert!(
            count >= 3 && count <= 6,
            "Expected ~5 sweeps in 100ms with 20ms interval, got {}",
            count
        );
    }

    #[tokio::test]
    async fn detector_stops_when_channel_closes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let query = MockQuery::new(counter.clone());
        let (tx, mut rx) = mpsc::channel(1);

        let detector = OrphanDetector::new(Duration::from_millis(10), query);

        let handle = tokio::spawn(async move {
            detector.run(tx).await;
        });

        let _ = rx.recv().await;
        drop(rx);

        handle.await.ok();

        let count = counter.load(Ordering::SeqCst);
        assert!(count <= 2, "Should stop when channel closes, got {}", count);
    }

    struct ErrorQuery;

    impl ErrorQuery {
        fn new() -> Self {
            Self
        }
    }

    impl OrphanQuery for ErrorQuery {
        async fn query_orphans(&self) -> Result<Vec<OrphanProcess>, String> {
            Err("query failed".to_string())
        }
    }

    #[tokio::test]
    async fn detector_handles_query_errors() {
        let query = ErrorQuery::new();
        let (tx, _rx) = mpsc::channel(10);

        let detector = OrphanDetector::new(Duration::from_millis(10), query);

        tokio::spawn(async move {
            detector.run(tx).await;
        });

        sleep(Duration::from_millis(50)).await;
    }
}
