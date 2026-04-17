use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("invalid cron expression: expected 5 fields, got {0}")]
    WrongFieldCount(usize),
    #[error("invalid cron expression: field {field} position has invalid value '{value}'")]
    InvalidField { field: usize, value: String },
    #[error("invalid cron expression: field {field} out of range {min}-{max}, got {value}")]
    OutOfRange {
        field: usize,
        value: i64,
        min: i64,
        max: i64,
    },
}

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

pub fn validate_cron_expression(expr: &str) -> Result<(), CronError> {
    let fields: Vec<&str> = expr.split_whitespace().collect();

    if fields.len() != 5 {
        return Err(CronError::WrongFieldCount(fields.len()));
    }

    let minute_range = 0..=59i64;
    let hour_range = 0..=23i64;
    let day_of_month_range = 1..=31i64;
    let month_range = 1..=12i64;
    let day_of_week_range = 0..=7i64;

    let ranges: [std::ops::RangeInclusive<i64>; 5] = [
        minute_range,
        hour_range,
        day_of_month_range,
        month_range,
        day_of_week_range,
    ];
    let field_names = ["minute", "hour", "day-of-month", "month", "day-of-week"];

    for (i, field) in fields.iter().enumerate() {
        if *field == "*" {
            continue;
        }

        if let Some(value) = parse_cron_field(*field, i, &ranges[i], field_names[i])? {
            if !ranges[i].contains(&value) {
                return Err(CronError::OutOfRange {
                    field: i,
                    value,
                    min: *ranges[i].start(),
                    max: *ranges[i].end(),
                });
            }
        }
    }

    Ok(())
}

fn parse_cron_field(
    field: &str,
    position: usize,
    range: &std::ops::RangeInclusive<i64>,
    field_name: &str,
) -> Result<Option<i64>, CronError> {
    if field == "*" {
        return Ok(None);
    }

    // Handle */step syntax (e.g., */5)
    if let Some(step_str) = field.strip_prefix("*/") {
        if step_str.parse::<i64>().is_ok() {
            // */n is valid - means "every n units"
            return Ok(None);
        }
    }

    if field.contains('-') {
        let parts: Vec<&str> = field.split('-').collect();
        if parts.len() == 2 {
            if let (Ok(_start), Ok(_end)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
                // Range is valid format, we just need to check bounds later
                return Ok(None);
            }
        }
        return Err(CronError::InvalidField {
            field: position,
            value: field.to_string(),
        });
    }

    if let Ok(value) = field.parse::<i64>() {
        Ok(Some(value))
    } else {
        Err(CronError::InvalidField {
            field: position,
            value: field.to_string(),
        })
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
