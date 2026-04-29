//! Throttled recovery queue channel with bounded capacity and rate limiting.
//!
//! Implements backpressure to ensure the recovery queue ingestion rate never
//! exceeds the configured throttle, satisfying the invariant:
//! "Recovery queue ingestion rate never exceeds configured throttle"

use std::time::Duration;

use super::{RecoveryError, RecoveryItem, RecoveryResult};

#[derive(Debug, Clone)]
pub struct RecoveryThrottleConfig {
    pub capacity: usize,
    pub refill_rate: usize,
    pub refill_period: Duration,
}

impl RecoveryThrottleConfig {
    pub fn new(capacity: usize, refill_rate: usize, refill_period: Duration) -> Self {
        Self {
            capacity,
            refill_rate,
            refill_period,
        }
    }
}

#[derive(Debug)]
struct TokenBucket {
    tokens: usize,
    max_tokens: usize,
    refill_rate: usize,
    refill_period: Duration,
    last_refill: u64,
    current_time: u64,
    depth: usize,
    rejections: usize,
}

impl TokenBucket {
    fn new(capacity: usize, refill_rate: usize, refill_period: Duration) -> Self {
        Self {
            tokens: capacity,
            max_tokens: capacity,
            refill_rate,
            refill_period,
            last_refill: 0,
            current_time: 0,
            depth: 0,
            rejections: 0,
        }
    }

    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            self.rejections += 1;
            false
        }
    }

    fn refill(&mut self) {
        if self.current_time >= self.last_refill {
            let elapsed = self.current_time - self.last_refill;
            let periods = elapsed / self.refill_period.as_millis() as u64;
            if periods > 0 {
                let refill_amount = (periods as usize).saturating_mul(self.refill_rate);
                self.tokens = (self.tokens + refill_amount).min(self.max_tokens);
                self.last_refill = self.current_time;
            }
        }
    }

    fn advance_time(&mut self, duration: Duration) {
        let ms = duration.as_millis() as u64;
        self.current_time += ms;
        self.refill();
    }

    fn available(&self) -> usize {
        self.tokens
    }

    pub fn release(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    pub fn current_depth(&self) -> usize {
        self.depth
    }

    pub fn rejection_count(&self) -> usize {
        self.rejections
    }

    pub fn push(&mut self) {
        self.depth += 1;
    }

    #[allow(dead_code)]
    pub fn pop(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }
}

#[derive(Debug)]
pub struct RecoveryThrottle {
    config: RecoveryThrottleConfig,
    bucket: TokenBucket,
}

impl RecoveryThrottle {
    pub fn new(config: RecoveryThrottleConfig) -> Self {
        let bucket = TokenBucket::new(config.capacity, config.refill_rate, config.refill_period);
        Self { config, bucket }
    }

    pub async fn enqueue(&mut self, _item: RecoveryItem) -> RecoveryResult<()> {
        if !self.bucket.try_consume() {
            return Err(RecoveryError::QueueFull);
        }
        self.bucket.push();
        Ok(())
    }

    pub fn available_capacity(&self) -> usize {
        self.bucket.available()
    }

    pub fn total_capacity(&self) -> usize {
        self.config.capacity
    }

    pub fn config(&self) -> &RecoveryThrottleConfig {
        &self.config
    }

    pub fn advance_time(&mut self, duration: Duration) {
        self.bucket.advance_time(duration);
    }

    pub fn release(&mut self) {
        self.bucket.release();
    }

    pub fn current_depth(&self) -> usize {
        self.bucket.current_depth()
    }

    pub fn total_rejections(&self) -> usize {
        self.bucket.rejection_count()
    }
}

#[cfg(test)]
mod tests {
    use super::super::OrphanProcess;
    use super::*;
    

    fn make_test_item(id: &str) -> RecoveryItem {
        RecoveryItem {
            orphan: OrphanProcess {
                instance_id: id.to_string(),
                lineage_id: "lineage-1".to_string(),
                failed_at: Duration::from_secs(0),
            },
            enqueued_at: Duration::from_secs(0),
        }
    }

    #[tokio::test]
    async fn throttle_respects_initial_capacity() {
        let config = RecoveryThrottleConfig::new(2, 1, Duration::from_secs(1));
        let mut throttle = RecoveryThrottle::new(config);

        assert!(throttle.enqueue(make_test_item("1")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("2")).await.is_ok());
        assert!(matches!(
            throttle.enqueue(make_test_item("3")).await,
            Err(RecoveryError::QueueFull)
        ));
    }

    #[tokio::test]
    async fn throttle_refills_tokens_over_time() {
        let config = RecoveryThrottleConfig::new(1, 1, Duration::from_millis(100));
        let mut throttle = RecoveryThrottle::new(config);

        assert!(throttle.enqueue(make_test_item("1")).await.is_ok());
        assert!(matches!(
            throttle.enqueue(make_test_item("2")).await,
            Err(RecoveryError::QueueFull)
        ));

        throttle.advance_time(Duration::from_millis(150));

        assert!(throttle.enqueue(make_test_item("2")).await.is_ok());
    }

    #[tokio::test]
    async fn throttle_refills_multiple_tokens() {
        let config = RecoveryThrottleConfig::new(3, 2, Duration::from_millis(100));
        let mut throttle = RecoveryThrottle::new(config);

        assert!(throttle.enqueue(make_test_item("1")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("2")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("3")).await.is_ok());
        assert!(matches!(
            throttle.enqueue(make_test_item("4")).await,
            Err(RecoveryError::QueueFull)
        ));

        throttle.advance_time(Duration::from_millis(200));

        assert!(throttle.enqueue(make_test_item("4")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("5")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("6")).await.is_ok());
        assert!(matches!(
            throttle.enqueue(make_test_item("7")).await,
            Err(RecoveryError::QueueFull)
        ));
    }

    #[tokio::test]
    async fn throttle_does_not_overfill_on_rapid_refill() {
        let config = RecoveryThrottleConfig::new(2, 10, Duration::from_millis(100));
        let mut throttle = RecoveryThrottle::new(config);

        assert!(throttle.enqueue(make_test_item("1")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("2")).await.is_ok());

        throttle.advance_time(Duration::from_secs(1));

        assert_eq!(throttle.available_capacity(), 2);
        assert!(throttle.enqueue(make_test_item("3")).await.is_ok());
        assert!(throttle.enqueue(make_test_item("4")).await.is_ok());
        assert!(matches!(
            throttle.enqueue(make_test_item("5")).await,
            Err(RecoveryError::QueueFull)
        ));
    }

    #[tokio::test]
    async fn throttle_available_capacity_reflects_current_state() {
        let config = RecoveryThrottleConfig::new(5, 1, Duration::from_secs(1));
        let throttle = RecoveryThrottle::new(config);

        assert_eq!(throttle.available_capacity(), 5);
        assert_eq!(throttle.total_capacity(), 5);
    }
}
