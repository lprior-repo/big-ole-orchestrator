use chrono::Duration;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobId(pub ulid::Ulid);

impl JobId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum JobPriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
    Background = 4,
}

impl JobPriority {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Critical),
            1 => Some(Self::High),
            2 => Some(Self::Normal),
            3 => Some(Self::Low),
            4 => Some(Self::Background),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    OneShot,
    Recurring,
    Delayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulePolicy {
    At(chrono::DateTime<chrono::Utc>),
    After(Duration),
    Cron(String),
    Immediate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum RetryPolicyError {
    #[error("max_attempts must be >= 1, got 0")]
    ZeroAttempts,

    #[error("backoff_multiplier must be >= 1.0, got {got}")]
    InvalidMultiplier { got: f64 },

    #[error("max_delay must be >= initial_delay, got max_delay={max}, initial_delay={initial}")]
    MaxDelayTooSmall { max: i64, initial: i64 },

    #[error("all retry attempts exhausted")]
    MaxAttemptsReached,

    #[error("backoff calculation overflowed")]
    BackoffOverflow,

    #[error("job kind does not support retries")]
    RetryNotAllowed,
}

impl RetryPolicy {
    pub fn new(
        max_attempts: u32,
        initial_delay: Duration,
        backoff_multiplier: f64,
        max_delay: Duration,
    ) -> Result<Self, RetryPolicyError> {
        if max_attempts == 0 {
            return Err(RetryPolicyError::ZeroAttempts);
        }
        if !backoff_multiplier.is_finite() || backoff_multiplier < 1.0 {
            return Err(RetryPolicyError::InvalidMultiplier {
                got: backoff_multiplier,
            });
        }
        if max_delay < initial_delay {
            return Err(RetryPolicyError::MaxDelayTooSmall {
                max: max_delay.num_milliseconds(),
                initial: initial_delay.num_milliseconds(),
            });
        }
        Ok(Self {
            max_attempts,
            backoff_multiplier,
            initial_delay,
            max_delay,
        })
    }

    pub fn calculate_backoff_delay(&self, attempt: u32) -> Result<Duration, RetryPolicyError> {
        if attempt == 0 {
            return Ok(Duration::zero());
        }
        if attempt > self.max_attempts {
            return Err(RetryPolicyError::MaxAttemptsReached);
        }

        let exponent = (attempt - 1) as f64;
        let multiplier_pow = self.backoff_multiplier.powf(exponent);

        let initial_ms = self.initial_delay.num_milliseconds() as f64;
        let max_ms = self.max_delay.num_milliseconds() as f64;

        let product = initial_ms * multiplier_pow;

        if product.is_infinite() {
            return Err(RetryPolicyError::BackoffOverflow);
        }

        let capped = product.min(max_ms);
        Ok(Duration::milliseconds(capped as i64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_new_returns_zero_attempts_error_when_max_attempts_is_zero() {
        let err = RetryPolicy::new(0, Duration::milliseconds(100), 2.0, Duration::seconds(10))
            .unwrap_err();
        assert!(matches!(err, RetryPolicyError::ZeroAttempts));
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_less_than_one() {
        let err = RetryPolicy::new(3, Duration::milliseconds(100), 0.5, Duration::seconds(10))
            .unwrap_err();
        assert!(matches!(
            err,
            RetryPolicyError::InvalidMultiplier { got } if got == 0.5
        ));
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_nan() {
        let err = RetryPolicy::new(
            3,
            Duration::milliseconds(100),
            f64::NAN,
            Duration::seconds(10),
        )
        .unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { .. }));
    }

    #[test]
    fn retry_policy_new_returns_invalid_multiplier_error_when_multiplier_is_infinite() {
        let err = RetryPolicy::new(
            3,
            Duration::milliseconds(100),
            f64::INFINITY,
            Duration::seconds(10),
        )
        .unwrap_err();
        assert!(matches!(err, RetryPolicyError::InvalidMultiplier { .. }));
    }

    #[test]
    fn retry_policy_new_returns_max_delay_too_small_error_when_max_delay_less_than_initial() {
        let err = RetryPolicy::new(
            3,
            Duration::milliseconds(500),
            2.0,
            Duration::milliseconds(100),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            RetryPolicyError::MaxDelayTooSmall {
                max: 100,
                initial: 500
            }
        ));
    }

    #[test]
    fn retry_policy_calculate_backoff_delay_returns_zero_for_attempt_zero() {
        let policy =
            RetryPolicy::new(3, Duration::milliseconds(100), 2.0, Duration::seconds(10)).unwrap();
        let delay = policy.calculate_backoff_delay(0).unwrap();
        assert_eq!(delay, Duration::zero());
    }

    #[test]
    fn retry_policy_calculate_backoff_delay_returns_initial_delay_for_attempt_one() {
        let policy =
            RetryPolicy::new(3, Duration::milliseconds(100), 2.0, Duration::seconds(10)).unwrap();
        let delay = policy.calculate_backoff_delay(1).unwrap();
        assert_eq!(delay, Duration::milliseconds(100));
    }

    #[test]
    fn retry_policy_calculate_backoff_delay_returns_exponential_backoff() {
        let policy =
            RetryPolicy::new(5, Duration::milliseconds(100), 2.0, Duration::seconds(1000)).unwrap();
        assert_eq!(
            policy.calculate_backoff_delay(1).unwrap(),
            Duration::milliseconds(100)
        );
        assert_eq!(
            policy.calculate_backoff_delay(2).unwrap(),
            Duration::milliseconds(200)
        );
        assert_eq!(
            policy.calculate_backoff_delay(3).unwrap(),
            Duration::milliseconds(400)
        );
    }

    #[test]
    fn retry_policy_calculate_backoff_delay_caps_at_max_delay() {
        let policy = RetryPolicy::new(
            10,
            Duration::milliseconds(100),
            2.0,
            Duration::milliseconds(500),
        )
        .unwrap();
        assert_eq!(
            policy.calculate_backoff_delay(3).unwrap(),
            Duration::milliseconds(400)
        );
        assert_eq!(
            policy.calculate_backoff_delay(4).unwrap(),
            Duration::milliseconds(500)
        );
        assert_eq!(
            policy.calculate_backoff_delay(5).unwrap(),
            Duration::milliseconds(500)
        );
    }

    #[test]
    fn retry_policy_calculate_backoff_delay_returns_error_when_attempts_exhausted() {
        let policy =
            RetryPolicy::new(3, Duration::milliseconds(100), 2.0, Duration::seconds(10)).unwrap();
        let result = policy.calculate_backoff_delay(4);
        assert!(matches!(result, Err(RetryPolicyError::MaxAttemptsReached)));
    }

    #[test]
    fn job_priority_as_u8_returns_correct_values() {
        assert_eq!(JobPriority::Critical as u8, 0);
        assert_eq!(JobPriority::High as u8, 1);
        assert_eq!(JobPriority::Normal as u8, 2);
        assert_eq!(JobPriority::Low as u8, 3);
        assert_eq!(JobPriority::Background as u8, 4);
    }

    #[test]
    fn job_priority_from_u8_returns_correct_variants() {
        assert_eq!(JobPriority::from_u8(0), Some(JobPriority::Critical));
        assert_eq!(JobPriority::from_u8(1), Some(JobPriority::High));
        assert_eq!(JobPriority::from_u8(2), Some(JobPriority::Normal));
        assert_eq!(JobPriority::from_u8(3), Some(JobPriority::Low));
        assert_eq!(JobPriority::from_u8(4), Some(JobPriority::Background));
        assert_eq!(JobPriority::from_u8(5), None);
    }
}
