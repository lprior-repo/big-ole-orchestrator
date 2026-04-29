//! Audit logging for the actor supervisor.
//!
//! Provides structured audit entries for actor panics, restarts, and isolations.

use std::time::SystemTime;

use vo_types::InstanceId;

#[derive(Debug, Clone)]
pub struct ActorSupervisorAuditEntry {
    pub timestamp: SystemTime,
    pub event_type: ActorSupervisorEventType,
    pub instance_id: InstanceId,
    pub details: ActorSupervisorAuditDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorSupervisorEventType {
    ActorPanic,
    ActorRestart,
    ActorIsolation,
    ActorPermanentFailure,
    BacktraceCaptured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorSupervisorAuditDetails {
    Panic {
        panic_message: String,
        backtrace_available: bool,
        backtrace_status: String,
        restart_attempts_before: u32,
    },
    Restart {
        restart_attempt: u32,
        backoff_ms: Option<u64>,
        previous_state: Option<String>,
    },
    Isolation {
        total_restart_attempts: u32,
        max_attempts: u32,
        last_known_good_state: Option<String>,
    },
    PermanentFailure {
        reason: String,
        final_state: Option<String>,
    },
    BacktraceCapture {
        backtrace_length: usize,
        capture_successful: bool,
    },
}

impl ActorSupervisorAuditEntry {
    pub fn new_panic(
        instance_id: InstanceId,
        panic_message: String,
        backtrace_available: bool,
        restart_attempts_before: u32,
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event_type: ActorSupervisorEventType::ActorPanic,
            instance_id,
            details: ActorSupervisorAuditDetails::Panic {
                panic_message,
                backtrace_available,
                backtrace_status: if backtrace_available {
                    "captured"
                } else {
                    "not_captured"
                }
                .to_string(),
                restart_attempts_before,
            },
        }
    }

    pub fn new_restart(
        instance_id: InstanceId,
        restart_attempt: u32,
        backoff_ms: Option<u64>,
        previous_state: Option<String>,
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event_type: ActorSupervisorEventType::ActorRestart,
            instance_id,
            details: ActorSupervisorAuditDetails::Restart {
                restart_attempt,
                backoff_ms,
                previous_state,
            },
        }
    }

    pub fn new_isolation(
        instance_id: InstanceId,
        total_restart_attempts: u32,
        max_attempts: u32,
        last_known_good_state: Option<String>,
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event_type: ActorSupervisorEventType::ActorIsolation,
            instance_id,
            details: ActorSupervisorAuditDetails::Isolation {
                total_restart_attempts,
                max_attempts,
                last_known_good_state,
            },
        }
    }

    pub fn new_permanent_failure(
        instance_id: InstanceId,
        reason: String,
        final_state: Option<String>,
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event_type: ActorSupervisorEventType::ActorPermanentFailure,
            instance_id,
            details: ActorSupervisorAuditDetails::PermanentFailure {
                reason,
                final_state,
            },
        }
    }

    pub fn new_backtrace_capture(
        instance_id: InstanceId,
        backtrace_length: usize,
        capture_successful: bool,
    ) -> Self {
        Self {
            timestamp: SystemTime::now(),
            event_type: ActorSupervisorEventType::BacktraceCaptured,
            instance_id,
            details: ActorSupervisorAuditDetails::BacktraceCapture {
                backtrace_length,
                capture_successful,
            },
        }
    }
}

impl std::fmt::Display for ActorSupervisorAuditEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] {:?} for {:?}: {:?}",
            self.timestamp, self.event_type, self.instance_id, self.details
        )
    }
}

pub trait AuditLog: Send + Sync {
    fn log_audit_entry(&self, entry: ActorSupervisorAuditEntry);
}

#[derive(Debug, Clone, Default)]
pub struct NoOpAuditLog;

impl AuditLog for NoOpAuditLog {
    fn log_audit_entry(&self, _entry: ActorSupervisorAuditEntry) {}
}

pub fn log_audit_entry_sync(
    audit_log: &dyn AuditLog,
    entry: ActorSupervisorAuditEntry,
) {
    audit_log.log_audit_entry(entry);
}

pub fn format_audit_entry_for_tracing(entry: &ActorSupervisorAuditEntry) -> String {
    format!("AUDIT: {}", entry)
}

pub fn emit_audit_log(entry: ActorSupervisorAuditEntry) {
    let formatted = format_audit_entry_for_tracing(&entry);
    match entry.event_type {
        ActorSupervisorEventType::ActorPanic => {
            tracing::error!(audit = true, "{}", formatted);
        }
        ActorSupervisorEventType::ActorRestart => {
            tracing::info!(audit = true, "{}", formatted);
        }
        ActorSupervisorEventType::ActorIsolation => {
            tracing::warn!(audit = true, "{}", formatted);
        }
        ActorSupervisorEventType::ActorPermanentFailure => {
            tracing::error!(audit = true, "{}", formatted);
        }
        ActorSupervisorEventType::BacktraceCaptured => {
            tracing::debug!(audit = true, "{}", formatted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    #[test]
    fn audit_entry_panic() {
        let entry = ActorSupervisorAuditEntry::new_panic(
            test_instance_id(),
            "test panic message".to_string(),
            true,
            0,
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorPanic);
        assert_eq!(entry.instance_id, test_instance_id());
    }

    #[test]
    fn audit_entry_restart() {
        let entry = ActorSupervisorAuditEntry::new_restart(
            test_instance_id(),
            1,
            Some(100),
            Some("running".to_string()),
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorRestart);
    }

    #[test]
    fn audit_entry_isolation() {
        let entry = ActorSupervisorAuditEntry::new_isolation(
            test_instance_id(),
            3,
            3,
            Some("last good state".to_string()),
        );

        assert_eq!(entry.event_type, ActorSupervisorEventType::ActorIsolation);
    }

    #[test]
    fn display_trait_audit_entry() {
        let entry = ActorSupervisorAuditEntry::new_panic(
            test_instance_id(),
            "test".to_string(),
            false,
            0,
        );

        let display = format!("{}", entry);
        assert!(display.contains("ActorPanic"));
        assert!(display.contains("test_instance_id"));
    }

    #[test]
    fn noop_audit_log() {
        let audit = NoOpAuditLog;
        let entry = ActorSupervisorAuditEntry::new_panic(
            test_instance_id(),
            "test".to_string(),
            false,
            0,
        );

        audit.log_audit_entry(entry);
    }
}