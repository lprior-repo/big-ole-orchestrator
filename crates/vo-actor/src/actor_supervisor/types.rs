//! Types for the actor supervisor module.
//!
//! Provides panic recovery for ractor actors with backtrace logging,
//! metrics, audit logging, and state isolation.

use std::backtrace::BacktraceStatus;
use std::time::Instant;

use vo_types::InstanceId;

use crate::lifecycle::ActorLifecycleState;

#[derive(Debug, Clone, PartialEq)]
pub struct ActorSupervisorConfig {
    pub max_restart_attempts: u32,
    pub initial_backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
}

impl Default for ActorSupervisorConfig {
    fn default() -> Self {
        Self {
            max_restart_attempts: 3,
            initial_backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorSupervisorState {
    pub lifecycle_state: ActorLifecycleState,
    pub restart_attempts: u32,
    pub last_restart_at: Option<Instant>,
    pub last_panic_at: Option<Instant>,
    pub last_known_good_state: Option<String>,
}

impl Default for ActorSupervisorState {
    fn default() -> Self {
        Self {
            lifecycle_state: ActorLifecycleState::Pending,
            restart_attempts: 0,
            last_restart_at: None,
            last_panic_at: None,
            last_known_good_state: None,
        }
    }
}

impl ActorSupervisorState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_running() -> Self {
        Self {
            lifecycle_state: ActorLifecycleState::Running,
            ..Default::default()
        }
    }

    pub fn record_panic(&mut self, backtrace: String) {
        self.last_panic_at = Some(Instant::now());
        self.last_known_good_state = Some(backtrace);
    }

    pub fn record_restart(&mut self) {
        self.restart_attempts += 1;
        self.last_restart_at = Some(Instant::now());
        self.lifecycle_state = ActorLifecycleState::Running;
    }

    pub fn can_restart(&self, max_attempts: u32) -> bool {
        self.restart_attempts < max_attempts
    }

    pub fn should_isolate(&self, max_attempts: u32) -> bool {
        self.restart_attempts >= max_attempts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicInfo {
    pub instance_id: InstanceId,
    pub panic_message: String,
    pub backtrace: String,
    pub backtrace_status: String,
    pub occurred_at: Instant,
}

impl PanicInfo {
    pub fn new(instance_id: InstanceId, panic_message: String, backtrace: String) -> Self {
        let backtrace_status = match backtrace.is_empty() {
            false => "captured",
            true => "not captured",
        }.to_string();

        Self {
            instance_id,
            panic_message,
            backtrace,
            backtrace_status,
            occurred_at: Instant::now(),
        }
    }

    pub fn is_backtrace_available(&self) -> bool {
        !self.backtrace.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActorSupervisorError {
    #[error("actor {instance_id} panic: {panic_message}")]
    ActorPanic {
        instance_id: InstanceId,
        panic_message: String,
        backtrace: String,
    },

    #[error("actor {instance_id} exceeded max restart attempts ({max_attempts})")]
    MaxRestartsExceeded {
        instance_id: InstanceId,
        max_attempts: u32,
    },

    #[error("actor {instance_id} isolated due to repeated panics")]
    ActorIsolated { instance_id: InstanceId },

    #[error("supervisor not running for actor {instance_id}")]
    SupervisorNotRunning { instance_id: InstanceId },

    #[error("invalid state transition for actor {instance_id}: {reason}")]
    InvalidStateTransition { instance_id: InstanceId, reason: String },
}

impl ActorSupervisorError {
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::ActorPanic { .. })
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::MaxRestartsExceeded { .. } | Self::ActorIsolated { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDecision {
    RestartNow,
    RestartWithBackoff(u64),
    Isolate,
    NoRestart,
}

impl RestartDecision {
    pub fn should_restart(&self) -> bool {
        !matches!(self, Self::NoRestart | Self::Isolate)
    }

    pub fn should_isolate(&self) -> bool {
        matches!(self, Self::Isolate)
    }
}

pub fn compute_restart_decision(
    state: &ActorSupervisorState,
    config: &ActorSupervisorConfig,
) -> RestartDecision {
    if state.should_isolate(config.max_restart_attempts) {
        return RestartDecision::Isolate;
    }

    if !state.can_restart(config.max_restart_attempts) {
        return RestartDecision::NoRestart;
    }

    let backoff = if state.restart_attempts > 0 {
        let exponent = (state.restart_attempts - 1) as f64;
        let delay = (config.initial_backoff_ms as f64)
            * config.backoff_multiplier.powf(exponent);
        Some(delay.min(config.max_backoff_ms as f64) as u64)
    } else {
        None
    };

    match backoff {
        Some(delay) if delay > 0 => RestartDecision::RestartWithBackoff(delay),
        Some(_) | None => RestartDecision::RestartNow,
    }
}

pub fn format_backtrace(backtrace: &std::backtrace::Backtrace) -> String {
    format!("{:?}", backtrace)
}

pub fn capture_panic_info<E: std::fmt::Display>(
    instance_id: InstanceId,
    panic_error: &E,
) -> PanicInfo {
    let panic_message = panic_error.to_string();
    let backtrace = capture_current_backtrace();
    PanicInfo::new(instance_id, panic_message, backtrace)
}

fn capture_current_backtrace() -> String {
    std::backtrace::Backtrace::capture()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_instance_id() -> InstanceId {
        InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
    }

    #[test]
    fn supervisor_state_records_panic() {
        let mut state = ActorSupervisorState::new();
        assert!(state.last_panic_at.is_none());

        state.record_panic("test backtrace".to_string());
        assert!(state.last_panic_at.is_some());
        assert_eq!(state.last_known_good_state, Some("test backtrace".to_string()));
    }

    #[test]
    fn supervisor_state_records_restart() {
        let mut state = ActorSupervisorState::new();
        assert_eq!(state.restart_attempts, 0);

        state.record_restart();
        assert_eq!(state.restart_attempts, 1);
        assert!(state.last_restart_at.is_some());
    }

    #[test]
    fn supervisor_state_can_restart() {
        let mut state = ActorSupervisorState::new();
        assert!(state.can_restart(3));

        state.restart_attempts = 3;
        assert!(!state.can_restart(3));
    }

    #[test]
    fn supervisor_state_should_isolate() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 3;
        assert!(state.should_isolate(3));

        state.restart_attempts = 2;
        assert!(!state.should_isolate(3));
    }

    #[test]
    fn restart_decision_restart_now() {
        let state = ActorSupervisorState::new();
        let config = ActorSupervisorConfig::default();
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::RestartNow));
    }

    #[test]
    fn restart_decision_isolate_after_max() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 3;
        let config = ActorSupervisorConfig::default();
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::Isolate));
    }

    #[test]
    fn restart_decision_with_backoff_after_first_failure() {
        let mut state = ActorSupervisorState::new();
        state.restart_attempts = 1;
        let config = ActorSupervisorConfig::default();
        let decision = compute_restart_decision(&state, &config);
        assert!(matches!(decision, RestartDecision::RestartWithBackoff(_)));
    }

    #[test]
    fn panic_info_detection() {
        let info = PanicInfo::new(
            test_instance_id(),
            "test panic".to_string(),
            "backtrace content".to_string(),
        );
        assert!(info.is_backtrace_available());
        assert_eq!(info.backtrace_status, "captured");
    }

    #[test]
    fn panic_info_empty_backtrace() {
        let info = PanicInfo::new(
            test_instance_id(),
            "test panic".to_string(),
            "".to_string(),
        );
        assert!(!info.is_backtrace_available());
        assert_eq!(info.backtrace_status, "not captured");
    }

    #[test]
    fn error_is_transient() {
        let error = ActorSupervisorError::ActorPanic {
            instance_id: test_instance_id(),
            panic_message: "test".to_string(),
            backtrace: "".to_string(),
        };
        assert!(error.is_transient());
        assert!(!error.is_fatal());
    }

    #[test]
    fn error_is_fatal() {
        let error = ActorSupervisorError::MaxRestartsExceeded {
            instance_id: test_instance_id(),
            max_attempts: 3,
        };
        assert!(!error.is_transient());
        assert!(error.is_fatal());
    }
}