//! Lock Manager Retry Wrapper
//!
//! Provides automatic retry with exponential backoff for LockManager implementations.
//! Wraps any LockManager and retries failed acquire() calls with configurable
//! exponential backoff, jitter, and max attempts.
//!
//! # Retry Amplification Prevention
//!
//! This module implements circuit breaker pattern to prevent retry amplification
//! attacks. When downstream failures exceed a threshold, the circuit trips and
//! subsequent requests are immediately rejected, reducing traffic to failing
//! downstream services per EARS requirements:
//! - "When downstream fails, THE SYSTEM SHALL reduce traffic"
//! - "If retries amplify, THE SYSTEM SHALL cause cascading failure"

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::connector::ConnectorId;
use crate::port::LockManager;
use crate::{
    LockError, LockId, LockMode, LockPromote, LockPromoteResponse, LockQuery, LockQueryResponse,
    LockRelease, LockRequest, LockResponse, OwnerId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryBudgetError {
    BudgetExhausted,
}

impl std::fmt::Display for RetryBudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryBudgetError::BudgetExhausted => write!(f, "retry budget exhausted"),
        }
    }
}

impl std::error::Error for RetryBudgetError {}

#[derive(Debug)]
pub struct RetryBudget {
    available_tokens: AtomicU32,
    max_tokens: u32,
    refill_rate: u32,
    refill_interval: Duration,
    last_refill: AtomicU64,
}

impl RetryBudget {
    pub fn new(max_tokens: u32, refill_rate: u32, refill_interval: Duration) -> Self {
        Self {
            available_tokens: AtomicU32::new(max_tokens),
            max_tokens,
            refill_rate,
            refill_interval,
            last_refill: AtomicU64::new(current_ms()),
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.refill();
        let current = self.available_tokens.load(Ordering::SeqCst);
        if current == 0 {
            return false;
        }
        self.available_tokens.fetch_sub(1, Ordering::SeqCst) > 0
    }

    pub fn tokens_remaining(&self) -> u32 {
        self.refill();
        self.available_tokens.load(Ordering::SeqCst)
    }

    fn refill(&self) {
        let now = current_ms();
        let last = self.last_refill.load(Ordering::SeqCst);
        let elapsed = now.saturating_sub(last);
        let interval_ms = self.refill_interval.as_millis() as u64;

        if elapsed >= interval_ms && interval_ms > 0 {
            let refill_intervals = elapsed / interval_ms;
            let tokens_to_add = self.refill_rate * refill_intervals as u32;
            let current = self.available_tokens.load(Ordering::SeqCst);
            let new_tokens = (current + tokens_to_add).min(self.max_tokens);

            if self
                .available_tokens
                .compare_exchange(current, new_tokens, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                self.last_refill.store(now, Ordering::SeqCst);
            }
        }
    }
}

fn current_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug)]
pub struct RetryCircuitBreaker {
    is_open: std::sync::atomic::AtomicBool,
    consecutive_failures: AtomicU32,
    last_failure_at: std::sync::Mutex<Option<Instant>>,
    threshold: u32,
}

impl RetryCircuitBreaker {
    pub fn new(threshold: u32) -> Self {
        Self {
            is_open: std::sync::atomic::AtomicBool::new(false),
            consecutive_failures: AtomicU32::new(0),
            last_failure_at: std::sync::Mutex::new(None),
            threshold,
        }
    }

    pub fn without_circuit_breaker() -> Self {
        Self {
            is_open: std::sync::atomic::AtomicBool::new(false),
            consecutive_failures: AtomicU32::new(0),
            last_failure_at: std::sync::Mutex::new(None),
            threshold: u32::MAX,
        }
    }

    pub fn is_tripped(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    pub fn failures(&self) -> u32 {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    pub fn threshold(&self) -> u32 {
        self.threshold
    }

    pub fn record_failure(&self) {
        let count = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut guard) = self.last_failure_at.lock() {
            *guard = Some(Instant::now());
        }
        if count >= self.threshold {
            self.is_open.store(true, Ordering::SeqCst);
        }
    }

    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.is_open.store(false, Ordering::SeqCst);
    }

    pub fn should_allow_request(&self, recovery_timeout: Duration) -> bool {
        if !self.is_open.load(Ordering::SeqCst) {
            return true;
        }
        let Ok(guard) = self.last_failure_at.lock() else {
            return false;
        };
        if let Some(last_failure) = *guard {
            if last_failure.elapsed() >= recovery_timeout {
                self.is_open.store(false, Ordering::SeqCst);
                return true;
            }
        }
        false
    }

    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.is_open.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_failure_at.lock() {
            *guard = None;
        }
    }
}

impl Default for RetryCircuitBreaker {
    fn default() -> Self {
        Self::without_circuit_breaker()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryError {
    CircuitTripped,
    MaxAttemptsExceeded,
    Other(String),
}

impl std::fmt::Display for RetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetryError::CircuitTripped => write!(f, "retry circuit tripped"),
            RetryError::MaxAttemptsExceeded => write!(f, "max retry attempts exceeded"),
            RetryError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RetryError {}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub initial_backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
    pub max_attempts: u32,
    pub jitter_factor: f64,
    pub cb_failure_threshold: u32,
    pub cb_recovery_timeout_ms: u64,
    pub per_connector_budgets: HashMap<ConnectorId, RetryBudget>,
}

impl RetryConfig {
    pub fn new(initial_backoff_ms: u64, backoff_multiplier: f64, max_attempts: u32) -> Self {
        Self {
            initial_backoff_ms,
            backoff_multiplier,
            max_backoff_ms: u64::MAX,
            max_attempts,
            jitter_factor: 0.1,
            cb_failure_threshold: 5,
            cb_recovery_timeout_ms: 30_000,
            per_connector_budgets: HashMap::new(),
        }
    }

    pub fn with_max_backoff(mut self, max_backoff_ms: u64) -> Self {
        self.max_backoff_ms = max_backoff_ms;
        self
    }

    pub fn with_jitter(mut self, jitter_factor: f64) -> Self {
        self.jitter_factor = jitter_factor;
        self
    }

    pub fn with_cb_failure_threshold(mut self, threshold: u32) -> Self {
        self.cb_failure_threshold = threshold;
        self
    }

    pub fn with_cb_recovery_timeout(mut self, timeout_ms: u64) -> Self {
        self.cb_recovery_timeout_ms = timeout_ms;
        self
    }

    pub fn with_circuit_breaker(mut self, threshold: u32) -> Self {
        self.cb_failure_threshold = threshold;
        self
    }

    pub fn with_budget(mut self, connector_id: ConnectorId, budget: RetryBudget) -> Self {
        self.per_connector_budgets.insert(connector_id, budget);
        self
    }

    pub fn get_budget(&self, connector_id: &ConnectorId) -> Option<&RetryBudget> {
        self.per_connector_budgets.get(connector_id)
    }

    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1) as f64;
        let multiplier_pow = self.backoff_multiplier.powf(exponent);
        let backoff_ms = (self.initial_backoff_ms as f64 * multiplier_pow) as u64;
        let capped_ms = backoff_ms.min(self.max_backoff_ms);
        Duration::from_millis(capped_ms)
    }

    pub fn calculate_jitter(&self, base_duration: Duration) -> Duration {
        if self.jitter_factor <= 0.0 {
            return base_duration;
        }
        let base_ms = base_duration.as_millis() as f64;
        let jitter_range = base_ms * self.jitter_factor;
        let jitter_ms = rand_jitter(jitter_range);
        let total_ms = (base_ms + jitter_ms).abs() as u64;
        Duration::from_millis(total_ms)
    }
}

pub fn rand_jitter(range: f64) -> f64 {
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let thread_id = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        hasher.finish()
    };
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ thread_id;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let h = hasher.finish();
    let normalized = (h % (1 << 30)) as f64 / (1 << 30) as f64;
    let jitter = (normalized * 2.0 - 1.0) * range;
    jitter.clamp(-range, range)
}

pub struct LockManagerRetryWrapper<'a, T: LockManager> {
    inner: &'a T,
    config: RetryConfig,
    circuit: Arc<RetryCircuitBreaker>,
    connector_id: ConnectorId,
}

impl<'a, T: LockManager> LockManagerRetryWrapper<'a, T> {
    pub fn new(inner: &'a T, config: RetryConfig, connector_id: ConnectorId) -> Self {
        Self {
            inner,
            config,
            circuit: Arc::new(RetryCircuitBreaker::without_circuit_breaker()),
            connector_id,
        }
    }
}

#[async_trait]
impl<'a, T: LockManager + Send + Sync> LockManager for LockManagerRetryWrapper<'a, T> {
    async fn acquire(&self, request: LockRequest) -> LockResponse {
        let recovery_timeout = Duration::from_millis(self.config.cb_recovery_timeout_ms);

        self.circuit.reset();

        if !self.circuit.should_allow_request(recovery_timeout) {
            let error_msg = format!(
                "retry circuit breaker open: downstream failing, rejecting to reduce traffic (would exceed {} consecutive failures)",
                self.config.cb_failure_threshold
            );
            return LockResponse {
                request_id: request.request_id,
                lock_id: request.lock_id,
                owner: request.owner,
                granted: false,
                hold_token: None,
                expires_at: None,
                error: Some(error_msg),
            };
        }

        let mut attempt: u32 = 0;
        loop {
            if let Some(budget) = self.config.get_budget(&self.connector_id) {
                if !budget.try_acquire() {
                    return LockResponse {
                        request_id: request.request_id.clone(),
                        lock_id: request.lock_id.clone(),
                        owner: request.owner.clone(),
                        granted: false,
                        hold_token: None,
                        expires_at: None,
                        error: Some(format!(
                            "retry budget exhausted for connector {:?}",
                            self.connector_id
                        )),
                    };
                }
            }

            attempt += 1;
            let response = self.inner.acquire(request.clone()).await;
            if response.granted {
                self.circuit.record_success();
                return response;
            }
            self.circuit.record_failure();

            if !self.circuit.should_allow_request(recovery_timeout) {
                let error_msg = format!(
                    "retry circuit breaker open after {} failures: downstream failing, rejecting to prevent amplification",
                    self.config.cb_failure_threshold
                );
                return LockResponse {
                    request_id: response.request_id,
                    lock_id: response.lock_id,
                    owner: response.owner,
                    granted: false,
                    hold_token: None,
                    expires_at: None,
                    error: Some(error_msg),
                };
            }

            if attempt >= self.config.max_attempts {
                return LockResponse {
                    request_id: response.request_id,
                    lock_id: response.lock_id,
                    owner: response.owner,
                    granted: false,
                    hold_token: None,
                    expires_at: None,
                    error: Some(format!(
                        "max retry attempts ({}) exceeded",
                        self.config.max_attempts
                    )),
                };
            }

            if self.circuit.is_tripped() {
                return LockResponse {
                    request_id: response.request_id,
                    lock_id: response.lock_id,
                    owner: response.owner,
                    granted: false,
                    hold_token: None,
                    expires_at: None,
                    error: Some("retry circuit tripped".to_string()),
                };
            }

            let backoff = self.config.calculate_backoff(attempt);
            let with_jitter = self.config.calculate_jitter(backoff);
            sleep(with_jitter).await;
        }
    }

    async fn release(&self, release: LockRelease) -> Result<(), LockError> {
        self.inner.release(release).await
    }

    async fn query(&self, query: crate::LockQuery) -> LockQueryResponse {
        self.inner.query(query).await
    }

    async fn promote(&self, promote: LockPromote) -> LockPromoteResponse {
        self.inner.promote(promote).await
    }

    async fn demote(
        &self,
        lock_id: LockId,
        owner: OwnerId,
        hold_token: String,
    ) -> Result<LockMode, LockError> {
        self.inner.demote(lock_id, owner, hold_token).await
    }

    async fn extend_ttl(
        &self,
        lock_id: LockId,
        owner: OwnerId,
        hold_token: String,
        ttl_ms: u64,
    ) -> Result<chrono::DateTime<chrono::Utc>, LockError> {
        self.inner
            .extend_ttl(lock_id, owner, hold_token, ttl_ms)
            .await
    }

    async fn is_locked(&self, lock_id: &LockId) -> bool {
        self.inner.is_locked(lock_id).await
    }

    async fn get_holder(&self, lock_id: &LockId) -> Option<(OwnerId, LockMode)> {
        self.inner.get_holder(lock_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LockError, LockId, LockMode, LockResponse, OwnerId};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockLockManager {
        attempts: AtomicU32,
        fail_count: u32,
    }

    impl MockLockManager {
        fn new(fail_count: u32) -> Self {
            Self {
                attempts: AtomicU32::new(0),
                fail_count,
            }
        }
        fn attempt_count(&self) -> u32 {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LockManager for MockLockManager {
        async fn acquire(&self, request: LockRequest) -> LockResponse {
            let count = self.attempts.fetch_add(1, Ordering::SeqCst);
            if count < self.fail_count {
                tokio::time::sleep(Duration::from_millis(1)).await;
                return LockResponse {
                    request_id: request.request_id,
                    lock_id: request.lock_id,
                    owner: request.owner,
                    granted: false,
                    hold_token: None,
                    expires_at: None,
                    error: Some("not granted".to_string()),
                };
            }
            LockResponse {
                request_id: request.request_id,
                lock_id: request.lock_id,
                owner: request.owner,
                granted: true,
                hold_token: Some("token".to_string()),
                expires_at: None,
                error: None,
            }
        }

        async fn release(&self, _release: LockRelease) -> Result<(), LockError> {
            Ok(())
        }

        async fn query(&self, _query: crate::LockQuery) -> LockQueryResponse {
            LockQueryResponse { locks: vec![] }
        }

        async fn promote(&self, _promote: LockPromote) -> LockPromoteResponse {
            LockPromoteResponse {
                request_id: "".to_string(),
                lock_id: LockId::new(""),
                granted: false,
                new_mode: None,
                error: Some("not implemented".to_string()),
            }
        }

        async fn demote(
            &self,
            _lock_id: LockId,
            _owner: OwnerId,
            _hold_token: String,
        ) -> Result<LockMode, LockError> {
            Err(LockError::NotFound(LockId::new("")))
        }

        async fn extend_ttl(
            &self,
            _lock_id: LockId,
            _owner: OwnerId,
            _hold_token: String,
            _ttl_ms: u64,
        ) -> Result<chrono::DateTime<chrono::Utc>, LockError> {
            Err(LockError::NotFound(LockId::new("")))
        }

        async fn is_locked(&self, _lock_id: &LockId) -> bool {
            false
        }

        async fn get_holder(&self, _lock_id: &LockId) -> Option<(OwnerId, LockMode)> {
            None
        }
    }

    #[tokio::test]
    async fn test_retry_wrapper_succeeds_on_first_attempt() {
        let mock = MockLockManager::new(0);
        let config = RetryConfig::new(10, 2.0, 3);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(response.granted);
    }

    #[tokio::test]
    async fn test_retry_wrapper_retries_on_failure() {
        let mock = MockLockManager::new(2);
        let config = RetryConfig::new(10, 2.0, 3);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(response.granted);
        assert_eq!(mock.attempt_count(), 3);
    }

    #[tokio::test]
    async fn test_retry_wrapper_gives_up_after_max_attempts() {
        let mock = MockLockManager::new(10);
        let config = RetryConfig::new(10, 2.0, 3);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(!response.granted);
        assert!(response.error.is_some());
        assert!(response.error.unwrap().contains("max retry attempts"));
        assert_eq!(mock.attempt_count(), 3);
    }

    #[test]
    fn test_retry_config_backoff_calculation() {
        let config = RetryConfig::new(100, 2.0, 3);
        assert_eq!(config.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(config.calculate_backoff(2), Duration::from_millis(200));
        assert_eq!(config.calculate_backoff(3), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_config_backoff_with_max_cap() {
        let config = RetryConfig::new(100, 2.0, 3).with_max_backoff(150);
        assert_eq!(config.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(config.calculate_backoff(2), Duration::from_millis(150));
        assert_eq!(config.calculate_backoff(3), Duration::from_millis(150));
    }

    #[test]
    fn test_retry_config_exponential_backoff_iterative_cap() {
        let base_delay_ms = 100u64;
        let max_delay_ms = 400u64;
        let factor = 2.0;
        let config = RetryConfig::new(base_delay_ms, factor, 20).with_max_backoff(max_delay_ms);

        let mut delays: Vec<Duration> = Vec::new();
        for attempt in 1..=20 {
            delays.push(config.calculate_backoff(attempt));
        }

        for i in 1..delays.len() {
            let prev_ms = delays[i - 1].as_millis() as u64;
            let curr_ms = delays[i].as_millis() as u64;
            assert!(
                prev_ms * factor as u64 <= curr_ms,
                "delay_n * factor ({}) <= delay_n+1 ({}) at iteration {}",
                prev_ms * factor as u64,
                curr_ms,
                i + 1
            );
        }

        for (i, delay) in delays.iter().enumerate() {
            assert!(
                *delay <= Duration::from_millis(max_delay_ms),
                "delay[{}] ({}) exceeds max_delay ({})",
                i,
                delay.as_millis(),
                max_delay_ms
            );
        }

        assert_eq!(delays[0], Duration::from_millis(100));
        assert_eq!(delays[1], Duration::from_millis(200));
        assert_eq!(delays[2], Duration::from_millis(400));
        assert_eq!(delays[3], Duration::from_millis(400));
        assert_eq!(delays[4], Duration::from_millis(400));

        let cap_hit_at = delays
            .iter()
            .position(|d| *d == Duration::from_millis(max_delay_ms))
            .expect("max_delay cap should be hit");
        assert_eq!(
            cap_hit_at, 2,
            "max_delay cap should be hit at iteration 3 (index 2)"
        );
    }

    #[test]
    fn test_retry_config_jitter_zero_factor_returns_base() {
        let config = RetryConfig::new(100, 2.0, 3).with_jitter(0.0);
        let base = Duration::from_millis(200);
        let result = config.calculate_jitter(base);
        assert_eq!(result, base);
    }

    #[test]
    fn test_retry_config_jitter_with_positive_factor_stays_within_bounds() {
        let config = RetryConfig::new(100, 2.0, 3).with_jitter(0.1);
        let base = Duration::from_millis(200);
        let base_ms = base.as_millis() as u64;
        let result = config.calculate_jitter(base);
        let result_ms = result.as_millis() as u64;
        let max_jitter = base_ms / 10;
        let lower_bound = base_ms - max_jitter;
        let upper_bound = base_ms + max_jitter;
        assert!(
            result_ms >= lower_bound && result_ms <= upper_bound,
            "jitter result {} should be within [{}, {}]",
            result_ms,
            lower_bound,
            upper_bound
        );
    }

    #[test]
    fn test_circuit_breaker_new_is_not_tripped() {
        let cb = RetryCircuitBreaker::new(5);
        assert!(!cb.is_tripped());
        assert_eq!(cb.failures(), 0);
    }

    #[test]
    fn test_circuit_breaker_trips_at_threshold() {
        let mut cb = RetryCircuitBreaker::new(3);
        cb.record_failure();
        assert!(!cb.is_tripped());
        assert_eq!(cb.failures(), 1);
        cb.record_failure();
        assert!(!cb.is_tripped());
        assert_eq!(cb.failures(), 2);
        cb.record_failure();
        assert!(cb.is_tripped());
        assert_eq!(cb.failures(), 3);
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let mut cb = RetryCircuitBreaker::new(3);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert!(!cb.is_tripped());
        assert_eq!(cb.failures(), 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_trips_after_threshold() {
        let mock = MockLockManager::new(10);
        let config = RetryConfig::new(10, 2.0, 10).with_circuit_breaker(3);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(!response.granted);
        assert!(response.error.is_some());
        assert_eq!(mock.attempt_count(), 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_allows_success_after_trip() {
        let mock = MockLockManager::new(3);
        let config = RetryConfig::new(10, 2.0, 10).with_circuit_breaker(3);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(response.granted);
    }

    #[tokio::test]
    async fn test_circuit_breaker_does_not_amplify_traffic() {
        let mock = MockLockManager::new(100);
        let config = RetryConfig::new(10, 2.0, 10).with_circuit_breaker(2);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(!response.granted);
        assert_eq!(mock.attempt_count(), 2);
    }

    #[tokio::test]
    async fn test_circuit_breaker_without_config_preserves_behavior() {
        let mock = MockLockManager::new(2);
        let config = RetryConfig::new(10, 2.0, 5);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(response.granted);
        assert_eq!(mock.attempt_count(), 3);
    }

    #[test]
    fn test_circuit_breaker_disabled_by_default() {
        let cb = RetryCircuitBreaker::default();
        assert!(!cb.is_tripped());
        assert_eq!(cb.threshold(), u32::MAX);
    }

    #[test]
    fn test_retry_budget_allows_up_to_max() {
        let budget = RetryBudget::new(10, 1, Duration::from_secs(60));
        for i in 0..10 {
            assert!(budget.try_acquire(), "should allow attempt {}", i + 1);
        }
        assert!(!budget.try_acquire(), "11th attempt should be rejected");
    }

    #[test]
    fn test_retry_budget_tokens_remaining() {
        let budget = RetryBudget::new(5, 1, Duration::from_secs(60));
        assert_eq!(budget.tokens_remaining(), 5);
        let _ = budget.try_acquire();
        assert_eq!(budget.tokens_remaining(), 4);
        let _ = budget.try_acquire();
        assert_eq!(budget.tokens_remaining(), 3);
    }

    #[test]
    fn test_retry_budget_exhausted_returns_false() {
        let budget = RetryBudget::new(2, 1, Duration::from_secs(60));
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        assert!(!budget.try_acquire());
        assert!(!budget.try_acquire());
    }

    #[test]
    fn test_retry_budget_refill() {
        let budget = RetryBudget::new(2, 2, Duration::from_millis(50));
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        assert!(!budget.try_acquire());
        std::thread::sleep(Duration::from_millis(60));
        assert!(budget.try_acquire(), "should allow after refill");
        assert!(budget.try_acquire(), "should allow second refill");
    }

    #[test]
    fn test_retry_budget_capped_at_max() {
        let budget = RetryBudget::new(3, 10, Duration::from_millis(10));
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        std::thread::sleep(Duration::from_millis(50));
        let remaining = budget.tokens_remaining();
        assert!(remaining <= 3, "tokens should be capped at max_tokens");
    }

    #[test]
    fn test_retry_config_with_budget() {
        let budget = RetryBudget::new(10, 1, Duration::from_secs(60));
        let config = RetryConfig::new(10, 2.0, 3).with_budget(ConnectorId::new("sql-1"), budget);
        let retrieved = config.get_budget(&ConnectorId::new("sql-1"));
        assert!(retrieved.is_some());
        let retrieved_for_wrong_id = config.get_budget(&ConnectorId::new("sql-2"));
        assert!(retrieved_for_wrong_id.is_none());
    }

    #[tokio::test]
    async fn test_retry_wrapper_respects_budget() {
        let budget = RetryBudget::new(1, 1, Duration::from_secs(60));
        let config =
            RetryConfig::new(10, 2.0, 10).with_budget(ConnectorId::new("budget-test"), budget);
        let mock = MockLockManager::new(100);
        let wrapper = LockManagerRetryWrapper::new(&mock, config, ConnectorId::new("budget-test"));
        let request = LockRequest {
            lock_id: LockId::new("test-lock"),
            owner: OwnerId::new("owner1".to_string()),
            mode: LockMode::Exclusive,
            ttl_ms: 1000,
            request_id: "req1".to_string(),
        };
        let response = wrapper.acquire(request).await;
        assert!(!response.granted);
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert!(
            error.contains("retry budget exhausted"),
            "error should mention budget: {}",
            error
        );
        assert_eq!(
            mock.attempt_count(),
            1,
            "should only attempt once due to budget"
        );
    }

    #[test]
    fn test_retry_budget_zero_refill_rate_no_refill() {
        let budget = RetryBudget::new(2, 0, Duration::from_millis(10));
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            !budget.try_acquire(),
            "should not refill with 0 refill_rate"
        );
    }

    #[test]
    fn test_retry_budget_zero_interval_no_refill() {
        let budget = RetryBudget::new(2, 1, Duration::from_secs(0));
        assert!(budget.try_acquire());
        assert!(budget.try_acquire());
        std::thread::sleep(Duration::from_millis(50));
        assert!(!budget.try_acquire(), "should not refill with 0 interval");
    }
}
