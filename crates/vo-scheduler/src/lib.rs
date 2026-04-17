pub mod error;
pub mod job;
pub mod queue;
pub mod types;

#[cfg(test)]
mod queue_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{validate_cron_expression, CronError};

    #[test]
    fn cron_valid_expressions() {
        assert!(validate_cron_expression("*/5 * * * *").is_ok());
        assert!(validate_cron_expression("0 0 * * *").is_ok());
        assert!(validate_cron_expression("30 8 * * 1").is_ok());
        assert!(validate_cron_expression("0 0 1 * *").is_ok());
        assert!(validate_cron_expression("0 0 * 1 *").is_ok());
        assert!(validate_cron_expression("0 0 * * 0").is_ok());
        assert!(validate_cron_expression("0 0 * * 7").is_ok());
        assert!(validate_cron_expression("* * * * *").is_ok());
    }

    #[test]
    fn cron_invalid_wrong_field_count() {
        assert!(matches!(
            validate_cron_expression("* * * *"),
            Err(CronError::WrongFieldCount(4))
        ));
        assert!(matches!(
            validate_cron_expression("* * * * * *"),
            Err(CronError::WrongFieldCount(6))
        ));
        assert!(matches!(
            validate_cron_expression("invalid"),
            Err(CronError::WrongFieldCount(1))
        ));
    }

    #[test]
    fn cron_invalid_field_out_of_range() {
        assert!(matches!(
            validate_cron_expression("60 * * * *"),
            Err(CronError::OutOfRange {
                field: 0,
                value: 60,
                ..
            })
        ));
        assert!(matches!(
            validate_cron_expression("59 24 * * *"),
            Err(CronError::OutOfRange {
                field: 1,
                value: 24,
                ..
            })
        ));
        assert!(matches!(
            validate_cron_expression("0 0 32 * *"),
            Err(CronError::OutOfRange {
                field: 2,
                value: 32,
                ..
            })
        ));
        assert!(matches!(
            validate_cron_expression("0 0 * 13 *"),
            Err(CronError::OutOfRange {
                field: 3,
                value: 13,
                ..
            })
        ));
        assert!(matches!(
            validate_cron_expression("0 0 * * 8"),
            Err(CronError::OutOfRange {
                field: 4,
                value: 8,
                ..
            })
        ));
    }

    #[test]
    fn cron_invalid_negative_values() {
        assert!(validate_cron_expression("-1 * * * *").is_err());
        assert!(validate_cron_expression("0 -1 * * *").is_err());
    }

    #[test]
    fn scheduled_job_new_rejects_invalid_cron() {
        use crate::types::{JobKind, JobPriority, RetryPolicy, SchedulePolicy};
        use bytes::Bytes;

        let result = job::ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Cron("invalid".to_string()),
            RetryPolicy::default_policy(),
            Bytes::new(),
        );
        assert!(result.is_err());

        let result = job::ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Cron("* * * *".to_string()),
            RetryPolicy::default_policy(),
            Bytes::new(),
        );
        assert!(result.is_err());

        let result = job::ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Cron("60 * * * *".to_string()),
            RetryPolicy::default_policy(),
            Bytes::new(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn scheduled_job_new_accepts_valid_cron() {
        use crate::types::{JobKind, JobPriority, RetryPolicy, SchedulePolicy};
        use bytes::Bytes;

        let result = job::ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Cron("0 0 * * *".to_string()),
            RetryPolicy::default_policy(),
            Bytes::new(),
        );
        assert!(result.is_ok());

        let result = job::ScheduledJob::new(
            JobKind::Recurring,
            JobPriority::Normal,
            SchedulePolicy::Cron("*/5 * * * *".to_string()),
            RetryPolicy::default_policy(),
            Bytes::new(),
        );
        assert!(result.is_ok());
    }
}
