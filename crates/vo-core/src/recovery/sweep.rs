//! Orphan detection sweep mechanism.
//!
//! Periodically sweeps the storage layer to detect orphan processes that need
//! recovery. An orphan is a workflow instance that is in a failed state but
//! has not been processed for recovery.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

use super::OrphanProcess;

pub trait OrphanQuery: Send + Sync {
    fn query_orphans(&self) -> impl std::future::Future<Output = Result<Vec<OrphanProcess>, String>> + Send;
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