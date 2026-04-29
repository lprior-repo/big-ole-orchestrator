//! Scheduler domain types
//!
//! Types aligned to ADR-047 Background Job Scheduler Contract.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Duration;
use ulid::Ulid;

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
    Interval(Duration),
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

    pub fn interval(duration: Duration) -> Self {
        Self::Interval(duration)
    }

    pub fn next_fire_time(&self, last_fire_ms: u64) -> Option<u64> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        match self {
            Self::At(dt) => {
                let fire_ms = dt.timestamp_millis() as u64;
                if fire_ms > now_ms {
                    Some(fire_ms)
                } else {
                    Some(now_ms)
                }
            }
            Self::After(delay) => Some(now_ms.saturating_add(delay.as_millis() as u64)),
            Self::Immediate => Some(now_ms),
            Self::Cron(_) => None,
            Self::Interval(interval) => {
                if last_fire_ms == 0 {
                    Some(now_ms.saturating_add(interval.as_millis() as u64))
                } else {
                    Some(last_fire_ms.saturating_add(interval.as_millis() as u64))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SchedulerRetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl SchedulerRetryPolicy {
    pub fn new(
        max_attempts: u32,
        backoff_multiplier: f64,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Self {
        Self {
            max_attempts,
            backoff_multiplier,
            initial_delay,
            max_delay,
        }
    }

    pub fn default_retry() -> Self {
        Self {
            max_attempts: 3,
            backoff_multiplier: 2.0,
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
        }
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

#[derive(Debug, Clone, PartialEq)]
pub enum ScheduledJobValidationError {
    RetryPolicyMaxAttemptsZero,
    RetryPolicyBackoffMultiplierBelowOne {
        value: f64,
    },
    RetryPolicyInitialDelayZero,
    RetryCountExceedsMax {
        attempt_count: u32,
        max_attempts: u32,
    },
    LastErrorMismatch {
        has_error: bool,
        state: JobState,
    },
}

impl std::fmt::Display for ScheduledJobValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RetryPolicyMaxAttemptsZero => {
                write!(f, "retry_policy.max_attempts must be > 0")
            }
            Self::RetryPolicyBackoffMultiplierBelowOne { value } => {
                write!(
                    f,
                    "retry_policy.backoff_multiplier {} must be >= 1.0",
                    value
                )
            }
            Self::RetryPolicyInitialDelayZero => {
                write!(f, "retry_policy.initial_delay must be > 0")
            }
            Self::RetryCountExceedsMax {
                attempt_count,
                max_attempts,
            } => {
                write!(
                    f,
                    "attempt_count {} exceeds max_attempts {}",
                    attempt_count, max_attempts
                )
            }
            Self::LastErrorMismatch { has_error, state } => {
                write!(
                    f,
                    "last_error mismatch: has_error={} but state={:?}",
                    has_error, state
                )
            }
        }
    }
}

impl std::error::Error for ScheduledJobValidationError {}

impl ScheduledJob {
    pub fn validate(&self) -> Result<(), ScheduledJobValidationError> {
        if self.retry_policy.max_attempts == 0 {
            return Err(ScheduledJobValidationError::RetryPolicyMaxAttemptsZero);
        }
        if self.retry_policy.backoff_multiplier < 1.0 {
            return Err(
                ScheduledJobValidationError::RetryPolicyBackoffMultiplierBelowOne {
                    value: self.retry_policy.backoff_multiplier,
                },
            );
        }
        if self.retry_policy.initial_delay.is_zero() {
            return Err(ScheduledJobValidationError::RetryPolicyInitialDelayZero);
        }
        if self.state == JobState::Retrying && self.attempt_count >= self.retry_policy.max_attempts
        {
            return Err(ScheduledJobValidationError::RetryCountExceedsMax {
                attempt_count: self.attempt_count,
                max_attempts: self.retry_policy.max_attempts,
            });
        }
        let error_mismatch = self.last_error.is_some()
            != matches!(self.state, JobState::Failed | JobState::Retrying);
        if error_mismatch {
            return Err(ScheduledJobValidationError::LastErrorMismatch {
                has_error: self.last_error.is_some(),
                state: self.state,
            });
        }
        Ok(())
    }
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
    pub schedule: SchedulePolicy,
    pub priority: JobPriority,
    pub max_retries: u32,
    pub backoff_ms: u64,
}

impl Job {
    pub fn new(id: JobId, payload: String, schedule: SchedulePolicy) -> Self {
        Self {
            id: JobId::generate(),
            payload,
            schedule,
            priority: JobPriority::Normal,
            max_retries: 3,
            backoff_ms: 1000,
        }
    }

    pub fn with_id(mut self, id: JobId) -> Self {
        self.id = id;
        self
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

/// A validated job identifier wrapping a `Ulid`.
///
/// Construct via [`JobId::generate`] (infallible) or [`JobId::parse`] (validates
/// string input). Implements [`FromStr`] and [`TryFrom<&str>`] for ergonomic integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Ulid);

impl JobId {
    #[must_use]
    pub fn generate() -> Self {
        Self(Ulid::new())
    }

    /// Parse a string into a `JobId`, trimming whitespace first.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::InvalidJobId`] if the input is empty or not a valid ULID.
    pub fn parse(input: &str) -> Result<Self, super::SchedulerError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(super::SchedulerError::InvalidJobId(
                "JobId must not be empty".to_string(),
            ));
        }
        Ulid::from_str(trimmed)
            .map(Self)
            .map_err(|e| super::SchedulerError::InvalidJobId(format!("invalid ULID: {e}")))
    }

    #[must_use]
    pub fn get(&self) -> Ulid {
        self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for JobId {
    type Err = super::SchedulerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<&str> for JobId {
    type Error = super::SchedulerError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl AsRef<Ulid> for JobId {
    fn as_ref(&self) -> &Ulid {
        &self.0
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
    fn job_builder() {
        let job = Job::new(
            "test payload".to_string(),
            Schedule::one_shot(Duration::from_secs(10)),
        )
        .with_priority(JobPriority::High)
        .with_retries(5, 500);

        assert_eq!(job.priority, JobPriority::High);
        assert_eq!(job.max_retries, 5);
        assert_eq!(job.backoff_ms, 500);
    }

    #[test]
    fn job_id_parse_valid() {
        let ulid_str = "01ARYZ6S41TVGZFMASEG1RB1EMN";
        assert_eq!(JobId::parse(ulid_str).unwrap(), JobId::generate());
        assert_eq!(
            JobId::parse("01aryz6s41tvgzfmaseg1rb1emn").unwrap(),
            JobId::generate()
        );
    }

    #[test]
    fn job_id_parse_invalid() {
        assert!(JobId::parse("").is_err());
        assert!(JobId::parse("   ").is_err());
        assert!(JobId::parse("abc").is_err());
        assert!(JobId::parse("not-a-ulid").is_err());
    }

    #[test]
    fn job_id_get() {
        let id = JobId::generate();
        assert_eq!(id.get(), id.0);
        assert_eq!(
            JobId::parse("01ARYZ6S41TVGZFMASEG1RB1EMN").unwrap().get(),
            Ulid::from_str("01ARYZ6S41TVGZFMASEG1RB1EMN").unwrap()
        );
    }

    #[test]
    fn job_id_from_str_valid() {
        use std::str::FromStr;
        let ulid_str = "01ARYZ6S41TVGZFMASEG1RB1EMN";
        let id = JobId::from_str(ulid_str).unwrap();
        assert_eq!(id.get(), Ulid::from_str(ulid_str).unwrap());
    }

    #[test]
    fn job_id_from_str_invalid() {
        use std::str::FromStr;
        assert!(JobId::from_str("").is_err());
        assert!(JobId::from_str("abc").is_err());
    }

    #[test]
    fn job_id_try_from_str() {
        let ulid_str = "01ARYZ6S41TVGZFMASEG1RB1EMN";
        let id = JobId::try_from(ulid_str).unwrap();
        assert_eq!(id.get(), Ulid::from_str(ulid_str).unwrap());
        assert!(JobId::try_from("").is_err());
        assert!(JobId::try_from("not-a-number").is_err());
    }

    #[test]
    fn job_id_as_ref() {
        let id = JobId::generate();
        assert_eq!(*id.as_ref(), id.0);
    }

    #[test]
    fn job_id_display() {
        let id = JobId::parse("01ARYZ6S41TVZFMASEG1RB1EMN").unwrap();
        assert_eq!(format!("{}", id), "01ARYZ6S41TVZFMASEG1RB1EMN");
    }

    #[test]
    fn scheduled_job_validate_success() {
        let job = ScheduledJob {
            id: JobId::generate(),
            kind: JobKind::OneShot,
            state: JobState::Pending,
            priority: JobPriority::Normal,
            schedule_policy: SchedulePolicy::Immediate,
            retry_policy: SchedulerRetryPolicy::default_retry(),
            attempt_count: 0,
            due_at: Utc::now(),
            payload: SerializedPayload::new("{}".to_string()),
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(job.validate().is_ok());
    }

    #[test]
    fn scheduled_job_validate_retrying_exceeds_max() {
        let mut job = ScheduledJob {
            id: JobId::generate(),
            kind: JobKind::OneShot,
            state: JobState::Retrying,
            priority: JobPriority::Normal,
            schedule_policy: SchedulePolicy::Immediate,
            retry_policy: SchedulerRetryPolicy::default_retry(),
            attempt_count: 3,
            due_at: Utc::now(),
            payload: SerializedPayload::new("{}".to_string()),
            last_error: Some("error".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(matches!(
            job.validate(),
            Err(ScheduledJobValidationError::RetryCountExceedsMax { .. })
        ));

        job.attempt_count = 2;
        assert!(job.validate().is_ok());
    }

    #[test]
    fn scheduled_job_validate_last_error_mismatch() {
        let mut job = ScheduledJob {
            id: JobId::generate(),
            kind: JobKind::OneShot,
            state: JobState::Failed,
            priority: JobPriority::Normal,
            schedule_policy: SchedulePolicy::Immediate,
            retry_policy: SchedulerRetryPolicy::default_retry(),
            attempt_count: 1,
            due_at: Utc::now(),
            payload: SerializedPayload::new("{}".to_string()),
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(matches!(
            job.validate(),
            Err(ScheduledJobValidationError::LastErrorMismatch { .. })
        ));

        job.last_error = Some("failed".to_string());
        assert!(job.validate().is_ok());
    }
}
