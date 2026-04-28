//! Priority-related tests for vo-executor scheduler

#[cfg(test)]
mod scheduler_priority_tests {
    use vo_executor::JobPriority;

    #[test]
    fn job_priority_higher_than_low() {
        assert!(JobPriority::High > JobPriority::Low);
    }

    #[test]
    fn job_priority_higher_than_medium() {
        assert!(JobPriority::High > JobPriority::Medium);
    }

    #[test]
    fn job_priority_medium_greater_than_low() {
        assert!(JobPriority::Medium > JobPriority::Low);
    }

    #[test]
    fn job_priority_low_is_not_greater_than_medium() {
        assert!(JobPriority::Low < JobPriority::Medium);
    }

    #[test]
    fn job_priority_higher_than_critical() {
        assert!(JobPriority::Critical > JobPriority::High);
    }
}
