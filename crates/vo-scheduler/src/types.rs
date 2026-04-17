use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

const CRON_FIELD_COUNT: usize = 5;
const MINUTE_MIN: u32 = 0;
const MINUTE_MAX: u32 = 59;
const HOUR_MIN: u32 = 0;
const HOUR_MAX: u32 = 23;
const DAY_OF_MONTH_MIN: u32 = 1;
const DAY_OF_MONTH_MAX: u32 = 31;
const MONTH_MIN: u32 = 1;
const MONTH_MAX: u32 = 12;
const DAY_OF_WEEK_MIN: u32 = 0;
const DAY_OF_WEEK_MAX: u32 = 7;

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

pub fn validate_cron_expression(expr: &str) -> Result<(), String> {
    let fields: Vec<&str> = expr.split_whitespace().collect();

    if fields.len() != CRON_FIELD_COUNT {
        return Err(format!(
            "cron expression must have exactly {} fields, got {}",
            CRON_FIELD_COUNT,
            fields.len()
        ));
    }

    for (i, field) in fields.iter().enumerate() {
        validate_cron_field(*field, i)?;
    }

    Ok(())
}

fn validate_cron_field(field: &str, field_index: usize) -> Result<(), String> {
    if field == "*" {
        return Ok(());
    }

    if field.contains(',') {
        let parts: Vec<&str> = field.split(',').collect();
        for part in parts {
            validate_cron_part(part, field_index)?;
        }
        return Ok(());
    }

    validate_cron_part(field, field_index)
}

fn validate_cron_part(part: &str, field_index: usize) -> Result<(), String> {
    if part.contains('-') {
        let parts: Vec<&str> = part.split('-').collect();
        if parts.len() != 2 {
            return Err(format!("invalid range '{}'", part));
        }

        let start: u32 = parts[0]
            .parse()
            .map_err(|_| format!("invalid number '{}' in range", parts[0]))?;
        let end: u32 = parts[1]
            .parse()
            .map_err(|_| format!("invalid number '{}' in range", parts[1]))?;

        let (min, max) = get_field_bounds(field_index);
        if start < min || end > max || start > end {
            return Err(format!(
                "range {}-{} out of bounds [{}, {}] for field {}",
                start, end, min, max, field_index
            ));
        }
        return Ok(());
    }

    if part.contains('/') {
        let parts: Vec<&str> = part.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid step expression '{}'", part));
        }

        let step: u32 = parts[1]
            .parse()
            .map_err(|_| format!("invalid step value '{}'", parts[1]))?;
        if step == 0 {
            return Err(format!("step value cannot be 0 in '{}'", part));
        }

        if parts[0] != "*" {
            validate_cron_part(parts[0], field_index)?;
        }
        return Ok(());
    }

    let value: u32 = part
        .parse()
        .map_err(|_| format!("invalid number '{}'", part))?;

    let (min, max) = get_field_bounds(field_index);
    if value < min || value > max {
        return Err(format!(
            "value {} out of bounds [{}, {}] for field {}",
            value, min, max, field_index
        ));
    }

    Ok(())
}

fn get_field_bounds(field_index: usize) -> (u32, u32) {
    match field_index {
        0 => (MINUTE_MIN, MINUTE_MAX),
        1 => (HOUR_MIN, HOUR_MAX),
        2 => (DAY_OF_MONTH_MIN, DAY_OF_MONTH_MAX),
        3 => (MONTH_MIN, MONTH_MAX),
        4 => (DAY_OF_WEEK_MIN, DAY_OF_WEEK_MAX),
        _ => unreachable!(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_valid_expressions() {
        assert!(validate_cron_expression("*/5 * * * *").is_ok());
        assert!(validate_cron_expression("0 0 * * *").is_ok());
        assert!(validate_cron_expression("0 12 * * *").is_ok());
        assert!(validate_cron_expression("0 0 1 * *").is_ok());
        assert!(validate_cron_expression("0 0 * 1 *").is_ok());
        assert!(validate_cron_expression("0 0 * * 0").is_ok());
        assert!(validate_cron_expression("0 0 * * 7").is_ok());
        assert!(validate_cron_expression("0-30 0-12 1-15 1-6 0-7").is_ok());
        assert!(validate_cron_expression("0,15,30,45 * * * *").is_ok());
        assert!(validate_cron_expression("0 0 1,15 * *").is_ok());
        assert!(validate_cron_expression("*/15 0-23 * * *").is_ok());
    }

    #[test]
    fn cron_invalid_field_count() {
        assert!(validate_cron_expression("* * *").is_err());
        assert!(validate_cron_expression("* * * * * *").is_err());
        assert!(validate_cron_expression("invalid").is_err());
    }

    #[test]
    fn cron_invalid_minute() {
        assert!(validate_cron_expression("60 * * * *").is_err());
        assert!(validate_cron_expression("-1 * * * *").is_err());
        assert!(validate_cron_expression("0-60 * * * *").is_err());
    }

    #[test]
    fn cron_invalid_hour() {
        assert!(validate_cron_expression("* 24 * * *").is_err());
        assert!(validate_cron_expression("* 0-24 * * *").is_err());
    }

    #[test]
    fn cron_invalid_day_of_month() {
        assert!(validate_cron_expression("* * 0 * *").is_err());
        assert!(validate_cron_expression("* * 32 * *").is_err());
        assert!(validate_cron_expression("* * 1-32 * *").is_err());
    }

    #[test]
    fn cron_invalid_month() {
        assert!(validate_cron_expression("* * * 0 *").is_err());
        assert!(validate_cron_expression("* * * 13 *").is_err());
        assert!(validate_cron_expression("* * * 1-13 *").is_err());
    }

    #[test]
    fn cron_invalid_day_of_week() {
        assert!(validate_cron_expression("* * * * -1").is_err());
        assert!(validate_cron_expression("* * * * 8").is_err());
        assert!(validate_cron_expression("* * * * 0-8").is_err());
    }

    #[test]
    fn cron_invalid_range_order() {
        assert!(validate_cron_expression("30-10 * * * *").is_err());
        assert!(validate_cron_expression("* 12-6 * * *").is_err());
    }

    #[test]
    fn cron_invalid_step() {
        assert!(validate_cron_expression("*/0 * * * *").is_err());
        assert!(validate_cron_expression("*/abc * * * *").is_err());
    }

    #[test]
    fn cron_invalid_numbers() {
        assert!(validate_cron_expression("abc * * * *").is_err());
        assert!(validate_cron_expression("* def * * *").is_err());
        assert!(validate_cron_expression("* * ghi * *").is_err());
        assert!(validate_cron_expression("* * * jkl *").is_err());
        assert!(validate_cron_expression("* * * * mno").is_err());
    }
}
