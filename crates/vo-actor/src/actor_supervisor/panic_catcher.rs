//! Panic catching utilities for actor supervision.
//!
//! Provides mechanisms to catch panics from actors and extract
//! backtraces for logging and recovery.

use std::panic::{catch_unwind, AssertUnwindSafe};

use vo_types::InstanceId;

use super::types::{PanicInfo, ActorSupervisorError};
use super::metrics::ActorSupervisorMetrics;

pub struct PanicCatcher;

impl PanicCatcher {
    pub fn catch_panic<F, R>(
        instance_id: InstanceId,
        f: F,
        metrics: &ActorSupervisorMetrics,
    ) -> Result<R, ActorSupervisorError>
    where
        F: FnOnce() -> R + std::panic::UnwindSafe,
    {
        let result = catch_unwind(AssertUnwindSafe(f));

        match result {
            Ok(value) => Ok(value),
            Err(panic_payload) => {
                metrics.record_panic();

                let panic_info = Self::extract_panic_info(instance_id.clone(), panic_payload);
                let has_backtrace = panic_info.is_backtrace_available();

                if has_backtrace {
                    metrics.record_backtrace_capture();
                }

                Err(ActorSupervisorError::ActorPanic {
                    instance_id,
                    panic_message: panic_info.panic_message.clone(),
                    backtrace: panic_info.backtrace.clone(),
                })
            }
        }
    }

    pub fn catch_panic_with_backtrace<F, R>(
        instance_id: InstanceId,
        f: F,
        metrics: &ActorSupervisorMetrics,
    ) -> Result<R, (ActorSupervisorError, Option<String>)>
    where
        F: FnOnce() -> R + std::panic::UnwindSafe,
    {
        let result = catch_unwind(AssertUnwindSafe(f));

        match result {
            Ok(value) => Ok(value),
            Err(panic_payload) => {
                metrics.record_panic();

                let panic_info = Self::extract_panic_info(instance_id.clone(), panic_payload);
                let has_backtrace = panic_info.is_backtrace_available();

                if has_backtrace {
                    metrics.record_backtrace_capture();
                }

                let error = ActorSupervisorError::ActorPanic {
                    instance_id,
                    panic_message: panic_info.panic_message.clone(),
                    backtrace: panic_info.backtrace.clone(),
                };

                Err((error, Some(panic_info.backtrace)))
            }
        }
    }

    fn extract_panic_info(
        instance_id: InstanceId,
        panic_payload: Box<dyn std::any::Any + Send>,
    ) -> PanicInfo {
        let panic_message = if let Some(s) = panic_payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_payload.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = panic_payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload type".to_string()
        };

        let backtrace = Self::capture_safe_backtrace();

        PanicInfo::new(instance_id, panic_message, backtrace)
    }

    fn capture_safe_backtrace() -> String {
        std::backtrace::Backtrace::capture().to_string()
    }

    pub fn is_panic_payload<T: 'static + Send + std::panic::RefUnwindSafe>(
        payload: &(dyn std::any::Any + Send),
    ) -> bool {
        payload.is::<T>()
    }
}

pub fn log_panic_with_backtrace(
    instance_id: &InstanceId,
    panic_message: &str,
    backtrace: &str,
) {
    if backtrace.is_empty() {
        tracing::error!(
            instance_id = %instance_id,
            panic_message = %panic_message,
            "Actor panicked without backtrace"
        );
    } else {
        tracing::error!(
            instance_id = %instance_id,
            panic_message = %panic_message,
            backtrace = %backtrace,
            "Actor panicked with backtrace"
        );
    }
}

pub fn log_panic_info(panic_info: &PanicInfo) {
    log_panic_with_backtrace(
        &panic_info.instance_id,
        &panic_info.panic_message,
        &panic_info.backtrace,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    #[test]
    fn catch_panic_success() {
        let metrics = ActorSupervisorMetrics::new();
        let result = PanicCatcher::catch_panic(test_instance_id(), || 42, &metrics);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(metrics.get_panic_count(), 0);
    }

    #[test]
    fn catch_panic_failure() {
        let metrics = ActorSupervisorMetrics::new();

        let result = PanicCatcher::catch_panic(
            test_instance_id(),
            || panic!("test panic"),
            &metrics,
        );

        assert!(result.is_err());
        assert_eq!(metrics.get_panic_count(), 1);
    }

    #[test]
    fn catch_panic_with_backtrace_success() {
        let metrics = ActorSupervisorMetrics::new();
        let result = PanicCatcher::catch_panic_with_backtrace(
            test_instance_id(),
            || 42,
            &metrics,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn catch_panic_with_backtrace_failure() {
        let metrics = ActorSupervisorMetrics::new();

        let result = PanicCatcher::catch_panic_with_backtrace(
            test_instance_id(),
            || panic!("test panic with backtrace"),
            &metrics,
        );

        assert!(result.is_err());
        let (error, backtrace) = result.unwrap_err();
        assert!(backtrace.is_some());
        assert!(matches!(error, ActorSupervisorError::ActorPanic { .. }));
    }

    #[test]
    fn catch_panic_with_string_message() {
        let metrics = ActorSupervisorMetrics::new();

        let result = PanicCatcher::catch_panic(
            test_instance_id(),
            || panic!("string panic"),
            &metrics,
        );

        assert!(result.is_err());
        if let Err(ActorSupervisorError::ActorPanic { panic_message, .. }) = result {
            assert_eq!(panic_message, "string panic");
        }
    }

    #[test]
    fn log_panic_with_backtrace_empty() {
        let instance_id = test_instance_id();
        log_panic_with_backtrace(&instance_id, "test message", "");
    }

    #[test]
    fn log_panic_with_backtrace_with_content() {
        let instance_id = test_instance_id();
        log_panic_with_backtrace(&instance_id, "test message", "backtrace content");
    }
}