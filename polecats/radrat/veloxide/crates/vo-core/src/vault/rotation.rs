use vo_types::credentials::{RotationPolicy, RotationState, RotationStatus};
use vo_types::TimestampMs;

pub struct RotationStateMachine {
    state: RotationState,
}

impl RotationStateMachine {
    pub fn new() -> Self {
        Self {
            state: RotationState::new(),
        }
    }

    pub fn state(&self) -> &RotationState {
        &self.state
    }

    pub fn start_rotation(&mut self) -> Result<(), RotationStateError> {
        match self.state.state() {
            RotationStatus::Idle => {
                self.state = RotationState {
                    state: RotationStatus::Rotating,
                    last_rotation: None,
                    next_scheduled_rotation: None,
                    consecutive_failures: 0,
                    last_failure_reason: None,
                };
                Ok(())
            }
            RotationStatus::Rotating => Err(RotationStateError::AlreadyRotating),
            RotationStatus::WaitingForOverlap => Err(RotationStateError::AlreadyRotating),
            RotationStatus::Failed(_) => {
                self.state = RotationState {
                    state: RotationStatus::Rotating,
                    last_rotation: self.state.last_rotation,
                    next_scheduled_rotation: self.state.next_scheduled_rotation,
                    consecutive_failures: self.state.consecutive_failures,
                    last_failure_reason: self.state.last_failure_reason.clone(),
                };
                Ok(())
            }
        }
    }

    pub fn complete_rotation(&mut self, next_rotation: Option<TimestampMs>) {
        self.state = RotationState {
            state: RotationStatus::Idle,
            last_rotation: Some(TimestampMs::now()),
            next_scheduled_rotation: next_rotation,
            consecutive_failures: 0,
            last_failure_reason: None,
        };
    }

    pub fn fail_rotation(&mut self, reason: String) {
        self.state = RotationState {
            state: RotationStatus::Failed(reason.clone()),
            last_rotation: self.state.last_rotation,
            next_scheduled_rotation: self.state.next_scheduled_rotation,
            consecutive_failures: self.state.consecutive_failures + 1,
            last_failure_reason: Some(reason),
        };
    }

    pub fn enter_overlap(&mut self) {
        self.state = RotationState {
            state: RotationStatus::WaitingForOverlap,
            last_rotation: self.state.last_rotation,
            next_scheduled_rotation: self.state.next_scheduled_rotation,
            consecutive_failures: self.state.consecutive_failures,
            last_failure_reason: self.state.last_failure_reason.clone(),
        };
    }

    pub fn acknowledge_failure(&mut self) {
        self.state = RotationState {
            state: RotationStatus::Idle,
            last_rotation: self.state.last_rotation,
            next_scheduled_rotation: self.state.next_scheduled_rotation,
            consecutive_failures: 0,
            last_failure_reason: None,
        };
    }

    pub fn compute_next_rotation(
        policy: &RotationPolicy,
        last_rotation: TimestampMs,
    ) -> Option<TimestampMs> {
        match policy {
            RotationPolicy::Manual => None,
            RotationPolicy::TimeBased { interval, .. } => Some(TimestampMs::new_unchecked(
                last_rotation.as_u64() + interval.as_u64(),
            )),
            RotationPolicy::UsageBased { .. } => None,
            RotationPolicy::EventBased { .. } => None,
        }
    }
}

impl Default for RotationStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationStateError {
    AlreadyRotating,
    InvalidTransition,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::DurationMs;

    #[test]
    fn rotation_state_machine_new_is_idle() {
        let machine = RotationStateMachine::new();
        assert_eq!(machine.state().state(), RotationStatus::Idle);
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_state_machine_start_rotation_from_idle() {
        let mut machine = RotationStateMachine::new();
        let result = machine.start_rotation();
        assert!(result.is_ok());
        assert_eq!(machine.state().state(), RotationStatus::Rotating);
    }

    #[test]
    fn rotation_state_machine_start_rotation_from_rotating_fails() {
        let mut machine = RotationStateMachine::new();
        machine
            .start_rotation()
            .expect("first rotation should succeed");
        let result = machine.start_rotation();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RotationStateError::AlreadyRotating
        ));
    }

    #[test]
    fn rotation_state_machine_complete_rotation_resets_failures() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().expect("rotation should start");
        machine.complete_rotation(None);
        assert_eq!(machine.state().state(), RotationStatus::Idle);
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_state_machine_fail_rotation_increments_failures() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().expect("rotation should start");
        machine.fail_rotation("encryption error".to_string());
        assert!(matches!(
            machine.state().state(),
            RotationStatus::Failed(ref s) if s == "encryption error"
        ));
        assert_eq!(machine.state().consecutive_failures(), 1);
    }

    #[test]
    fn rotation_state_machine_enter_overlap() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().expect("rotation should start");
        machine.enter_overlap();
        assert_eq!(machine.state().state(), RotationStatus::WaitingForOverlap);
    }

    #[test]
    fn rotation_state_machine_acknowledge_failure_resets() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().expect("rotation should start");
        machine.fail_rotation("temp error".to_string());
        machine.acknowledge_failure();
        assert_eq!(machine.state().state(), RotationStatus::Idle);
        assert_eq!(machine.state().consecutive_failures(), 0);
    }

    #[test]
    fn rotation_state_machine_fail_from_failed_allows_retry() {
        let mut machine = RotationStateMachine::new();
        machine.start_rotation().expect("rotation should start");
        machine.fail_rotation("temp error".to_string());
        let result = machine.start_rotation();
        assert!(result.is_ok());
        assert_eq!(machine.state().state(), RotationStatus::Rotating);
    }

    #[test]
    fn compute_next_rotation_manual_returns_none() {
        let policy = RotationPolicy::Manual;
        let last = TimestampMs::new_unchecked(1000);
        let next = RotationStateMachine::compute_next_rotation(&policy, last);
        assert!(next.is_none());
    }

    #[test]
    fn compute_next_rotation_time_based_returns_future() {
        let policy = RotationPolicy::TimeBased {
            interval: DurationMs::try_from(86400000u64).unwrap(),
            overlap_window: DurationMs::try_from(60000u64).unwrap(),
        };
        let last = TimestampMs::new_unchecked(1000);
        let next = RotationStateMachine::compute_next_rotation(&policy, last);
        assert!(next.is_some());
        assert!(next.unwrap().as_u64() > last.as_u64());
    }

    #[test]
    fn rotation_idle_to_rotating_to_idle_cycle() {
        let mut machine = RotationStateMachine::new();
        assert_eq!(machine.state().state(), RotationStatus::Idle);

        machine.start_rotation().expect("should start");
        assert_eq!(machine.state().state(), RotationStatus::Rotating);

        machine.complete_rotation(None);
        assert_eq!(machine.state().state(), RotationStatus::Idle);
    }

    #[test]
    fn rotation_idle_to_rotating_to_failed_to_idle() {
        let mut machine = RotationStateMachine::new();

        machine.start_rotation().expect("should start");
        assert_eq!(machine.state().state(), RotationStatus::Rotating);

        machine.fail_rotation("key not found".to_string());
        assert!(matches!(
            machine.state().state(),
            RotationStatus::Failed(ref s) if s == "key not found"
        ));

        machine.acknowledge_failure();
        assert_eq!(machine.state().state(), RotationStatus::Idle);
    }

    #[test]
    fn rotation_inv_failure_counter_persists_across_rotation() {
        let mut machine = RotationStateMachine::new();

        machine.start_rotation().expect("should start");
        machine.fail_rotation("error 1".to_string());
        assert_eq!(machine.state().consecutive_failures(), 1);

        machine
            .start_rotation()
            .expect("should start after failure");
        machine.fail_rotation("error 2".to_string());
        assert_eq!(machine.state().consecutive_failures(), 2);

        machine.complete_rotation(None);
        assert_eq!(machine.state().consecutive_failures(), 0);
    }
}
