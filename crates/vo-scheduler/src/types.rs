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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    OneShot,
    Recurring,
    Delayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchedulePolicy {
    At(DateTime<Utc>),
    After(#[serde(with = "humantime_serde")] Duration),
    Cron(String),
    Immediate,
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

impl RetryPolicy {
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
