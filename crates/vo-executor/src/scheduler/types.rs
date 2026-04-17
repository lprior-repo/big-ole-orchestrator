//! Scheduler domain types
//!
//! Types aligned to ADR-047 Background Job Scheduler Contract.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    #[default]
    Normal = 2,
    Low = 3,
    Background = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Scheduled,
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        )
    }

    pub fn is_non_terminal(&self) -> bool {
        !self.is_terminal()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    OneShot,
    Recurring,
    Delayed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePolicy {
    At(DateTime<Utc>),
    After(Duration),
    Cron(String),
    Immediate,
}

impl SchedulePolicy {
    pub fn at(dt: DateTime<Utc>) -> Self {
        Self::At(dt)
    }

    pub fn after(duration: Duration) -> Self {
        Self::After(duration)
    }

    pub fn cron(expr: &str) -> Self {
        Self::Cron(expr.to_string())
    }

    pub fn immediate() -> Self {
        Self::Immediate
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchedulerRetryPolicyError {
    #[error("initial_delay ({initial_ms}ms) must be <= max_delay ({max_ms}ms)")]
    InitialDelayExceedsMax { initial_ms: u64, max_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SchedulerRetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl SchedulerRetryPolicy {
    /// Create a new `SchedulerRetryPolicy` with validation.
    ///
    /// # Errors
    ///
    /// Returns `SchedulerRetryPolicyError::InitialDelayExceedsMax` if
    /// `initial_delay > max_delay`.
    pub fn new(
        max_attempts: u32,
        backoff_multiplier: f64,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, SchedulerRetryPolicyError> {
        if initial_delay > max_delay {
            return Err(SchedulerRetryPolicyError::InitialDelayExceedsMax {
                initial_ms: initial_delay.as_millis() as u64,
                max_ms: max_delay.as_millis() as u64,
            });
        }
        Ok(Self {
            max_attempts,
            backoff_multiplier,
            initial_delay,
            max_delay,
        })
    }

    pub fn default_retry() -> Self {
        Self::new(3, 2.0, Duration::from_millis(1000), Duration::from_secs(60))
            .expect("default_retry values are valid")
    }

    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32)) as u64;
        Duration::from_millis(delay_ms.min(self.max_delay.as_millis() as u64))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledJob {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub priority: JobPriority,
    pub schedule_policy: SchedulePolicy,
    pub retry_policy: SchedulerRetryPolicy,
    pub attempt_count: u32,
    pub due_at: DateTime<Utc>,
    pub payload: SerializedPayload,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPayload(pub String);

impl SerializedPayload {
    pub fn new(data: String) -> Self {
        Self(data)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Schedule {
    Cron(String),
    OneShot { fire_at_ms: u64 },
    Interval { interval_ms: u64 },
}

impl Schedule {
    pub fn cron(expr: &str) -> Self {
        Self::Cron(expr.to_string())
    }

    pub fn one_shot(delay: Duration) -> Self {
        let fire_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
            + delay.as_millis() as u64;
        Self::OneShot { fire_at_ms }
    }

    pub fn interval(interval: Duration) -> Self {
        Self::Interval {
            interval_ms: interval.as_millis() as u64,
        }
    }

    pub fn next_fire_time(&self, last_fire_ms: u64) -> Option<u64> {
        match self {
            Self::Cron(_) => {
                // For cron, we'd need a proper cron parser
                // For now, return None (caller should use cron crate)
                None
            }
            Self::OneShot { fire_at_ms } => {
                if last_fire_ms == 0 {
                    Some(*fire_at_ms)
                } else {
                    None
                }
            }
            Self::Interval { interval_ms } => {
                if last_fire_ms == 0 {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis() as u64);
                    Some(now_ms + interval_ms)
                } else {
                    Some(last_fire_ms.saturating_add(*interval_ms))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub payload: String,
    pub schedule: Schedule,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub backoff_ms: u64,
}

impl Job {
    pub fn new(id: JobId, payload: String, schedule: Schedule) -> Self {
        Self {
            id,
            payload,
            schedule,
            priority: JobPriority::Normal,
            max_retries: 3,
            backoff_ms: 1000,
        }
    }

    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_retries(mut self, max_retries: u32, backoff_ms: u64) -> Self {
        self.max_retries = max_retries;
        self.backoff_ms = backoff_ms;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub u64);

impl JobId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn parse(input: &str) -> Result<Self, super::SchedulerError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(super::SchedulerError::InvalidJobId(
                "JobId must not be empty".to_string(),
            ));
        }
        let id: u64 = trimmed
            .parse()
            .map_err(|e| super::SchedulerError::InvalidJobId(format!("invalid u64: {e}")))?;
        Ok(Self(id))
    }

    #[must_use]
    pub fn get(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "job-{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: JobId,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub attempt: u32,
}

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_concurrent: usize,
    pub scan_interval: Duration,
    pub max_jobs_per_scan: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 10,
            scan_interval: Duration::from_millis(100),
            max_jobs_per_scan: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_priority_ordering() {
        assert!(JobPriority::Critical < JobPriority::High);
        assert!(JobPriority::High < JobPriority::Normal);
        assert!(JobPriority::Normal < JobPriority::Low);
    }

    #[test]
    fn schedule_one_shot_next_fire() {
        let schedule = Schedule::one_shot(Duration::from_secs(60));
        if let Schedule::OneShot { fire_at_ms } = schedule {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            assert!(fire_at_ms > now_ms);
        } else {
            panic!("Expected OneShot schedule");
        }
    }

    #[test]
    fn schedule_interval_next_fire() {
        let schedule = Schedule::interval(Duration::from_secs(30));
        let next = schedule.next_fire_time(0);
        assert!(next.is_some());
        let next2 = schedule.next_fire_time(next.unwrap());
        assert!(next2.is_some());
        assert!(next2 > next);
    }

    #[test]
    fn scheduler_retry_policy_rejects_initial_delay_exceeding_max() {
        let result = SchedulerRetryPolicy::new(
            3,
            2.0,
            Duration::from_secs(120),
            Duration::from_secs(60),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "initial_delay (120000ms) must be <= max_delay (60000ms)");
    }


    #[test]
    fn scheduler_retry_policy_valid_config_succeeds() {
        let policy = SchedulerRetryPolicy::new(
            3,
            2.0,
            Duration::from_secs(1),
            Duration::from_secs(60),
        );
        assert!(policy.is_ok());
        let p = policy.unwrap();
        assert_eq!(p.max_attempts, 3);
        assert_eq!(p.initial_delay, Duration::from_secs(1));
        assert_eq!(p.max_delay, Duration::from_secs(60));
    }

    #[test]
    fn scheduler_retry_policy_equal_delays_succeeds() {
        let policy = SchedulerRetryPolicy::new(
            1,
            1.0,
            Duration::from_secs(30),
            Duration::from_secs(30),
        );
        assert!(policy.is_ok());
    }

    #[test]
    fn scheduler_retry_policy_default_retry_is_valid() {
        let _ = SchedulerRetryPolicy::default_retry();
    }
    #[test]
    fn job_builder() {
        let job = Job::new(
            JobId::new(1),
            "test payload".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        )
        .with_priority(JobPriority::High)
        .with_retries(5, 500);

        assert_eq!(job.id, JobId::new(1));
        assert_eq!(job.priority, JobPriority::High);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.backoff_ms, 500);
    }

    #[test]
    fn job_id_parse_valid() {
        assert_eq!(JobId::parse("42").unwrap(), JobId::new(42));
        assert_eq!(JobId::parse("0").unwrap(), JobId::new(0));
        assert_eq!(JobId::parse("  7  ").unwrap(), JobId::new(7));
        assert_eq!(
            JobId::parse(&u64::MAX.to_string()).unwrap(),
            JobId::new(u64::MAX)
        );
    }

    #[test]
    fn job_id_parse_invalid() {
        assert!(JobId::parse("").is_err());
        assert!(JobId::parse("   ").is_err());
        assert!(JobId::parse("abc").is_err());
        assert!(JobId::parse("-1").is_err());
        assert!(JobId::parse("1.5").is_err());
        assert!(JobId::parse("18446744073709551616").is_err());
    }

    #[test]
    fn job_id_get() {
        assert_eq!(JobId::new(99).get(), 99);
        assert_eq!(JobId::parse("123").unwrap().get(), 123);
    }
}
