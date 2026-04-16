use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Ulid);

impl JobId {
    pub fn generate() -> Self {
        Self(Ulid::new())
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_non_terminal(self) -> bool {
        !self.is_terminal()
    }
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduled => write!(f, "scheduled"),
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Retrying => write!(f, "retrying"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    OneShot,
    Recurring,
    Delayed,
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OneShot => write!(f, "one_shot"),
            Self::Recurring => write!(f, "recurring"),
            Self::Delayed => write!(f, "delayed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchedulePolicy {
    At(DateTime<Utc>),
    After(#[serde(with = "humantime_serde")] Duration),
    Cron(String),
    Immediate,
}

impl fmt::Display for SchedulePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::At(t) => write!(f, "at({})", t),
            Self::After(d) => write!(f, "after({:?})", d),
            Self::Cron(expr) => write!(f, "cron({})", expr),
            Self::Immediate => write!(f, "immediate"),
        }
    }
}

mod humantime_serde {
    use std::time::Duration;

    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(dur: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(dur.as_nanos() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u64::deserialize(deserializer)?;
        Ok(Duration::from_nanos(nanos))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetryPolicyError {
    MaxAttemptsZero,
    BackoffMultiplierBelowOne { value: f64 },
    InitialDelayZero,
    MaxDelayBelowInitial,
}

impl fmt::Display for RetryPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxAttemptsZero => write!(f, "max_attempts must be > 0"),
            Self::BackoffMultiplierBelowOne { value } => {
                write!(f, "backoff_multiplier {} must be >= 1.0", value)
            }
            Self::InitialDelayZero => write!(f, "initial_delay must be > 0"),
            Self::MaxDelayBelowInitial => write!(f, "max_delay must be >= initial_delay"),
        }
    }
}

impl std::error::Error for RetryPolicyError {}

impl RetryPolicy {
    pub fn try_new(
        max_attempts: u32,
        backoff_multiplier: f64,
        initial_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::MaxAttemptsZero);
        }
        if backoff_multiplier < 1.0 {
            return Err(RetryPolicyError::BackoffMultiplierBelowOne {
                value: backoff_multiplier,
            });
        }
        if initial_delay.is_zero() {
            return Err(RetryPolicyError::InitialDelayZero);
        }
        if max_delay < initial_delay {
            return Err(RetryPolicyError::MaxDelayBelowInitial);
        }
        Ok(Self {
            max_attempts,
            backoff_multiplier,
            initial_delay,
            max_delay,
        })
    }

    pub fn default_policy() -> Self {
        Self {
            max_attempts: 3,
            backoff_multiplier: 2.0,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300),
        }
    }

    pub fn compute_backoff(&self, attempt: u32) -> Duration {
        let delay_secs =
            self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);
        Duration::from_secs_f64(delay_secs.min(self.max_delay.as_secs_f64()))
    }

    pub fn can_retry(&self, attempt_count: u32) -> bool {
        attempt_count < self.max_attempts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[repr(u8)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl fmt::Display for JobPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::High => write!(f, "high"),
            Self::Normal => write!(f, "normal"),
            Self::Low => write!(f, "low"),
            Self::Background => write!(f, "background"),
        }
    }
}

const _: () = {
    fn assert_send_sync<T: Send + Sync>() {}
    fn check() {
        assert_send_sync::<JobId>();
        assert_send_sync::<JobState>();
        assert_send_sync::<JobKind>();
        assert_send_sync::<SchedulePolicy>();
        assert_send_sync::<RetryPolicy>();
        assert_send_sync::<JobPriority>();
    }
};
