use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionState {
    #[default]
    Idle,
    Queued,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl ExecutionState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Skipped)
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Queued)
    }

    #[must_use]
    pub const fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: ExecutionState,
    pub to: ExecutionState,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid state transition: {} -> {}", self.from, self.to)
    }
}

impl std::error::Error for InvalidTransition {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IdleState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunningState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailedState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkippedState;

impl IdleState {
    pub const fn queue(self) -> QueuedState {
        QueuedState
    }

    pub const fn skip(self) -> SkippedState {
        SkippedState
    }
}

impl QueuedState {
    pub const fn start(self) -> RunningState {
        RunningState
    }

    pub const fn skip(self) -> SkippedState {
        SkippedState
    }
}

impl RunningState {
    pub const fn complete(self) -> CompletedState {
        CompletedState
    }

    pub const fn fail(self) -> FailedState {
        FailedState
    }
}

pub trait TerminalState: sealed::TerminalSealed {}

impl TerminalState for CompletedState {}
impl TerminalState for FailedState {}
impl TerminalState for SkippedState {}

mod sealed {
    pub trait TerminalSealed {}
    impl TerminalSealed for super::CompletedState {}
    impl TerminalSealed for super::FailedState {}
    impl TerminalSealed for super::SkippedState {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateTransition {
    IdleToQueued,
    IdleToSkipped,
    QueuedToRunning,
    QueuedToSkipped,
    RunningToCompleted,
    RunningToFailed,
}

impl StateTransition {
    #[must_use]
    pub fn apply(self) -> ExecutionState {
        match self {
            Self::IdleToQueued => ExecutionState::Queued,
            Self::IdleToSkipped => ExecutionState::Skipped,
            Self::QueuedToRunning => ExecutionState::Running,
            Self::QueuedToSkipped => ExecutionState::Skipped,
            Self::RunningToCompleted => ExecutionState::Completed,
            Self::RunningToFailed => ExecutionState::Failed,
        }
    }

    #[must_use]
    pub fn from_states(self) -> (ExecutionState, ExecutionState) {
        match self {
            Self::IdleToQueued => (ExecutionState::Idle, ExecutionState::Queued),
            Self::IdleToSkipped => (ExecutionState::Idle, ExecutionState::Skipped),
            Self::QueuedToRunning => (ExecutionState::Queued, ExecutionState::Running),
            Self::QueuedToSkipped => (ExecutionState::Queued, ExecutionState::Skipped),
            Self::RunningToCompleted => (ExecutionState::Running, ExecutionState::Completed),
            Self::RunningToFailed => (ExecutionState::Running, ExecutionState::Failed),
        }
    }
}

#[must_use]
pub fn try_transition(from: ExecutionState, to: ExecutionState) -> Option<StateTransition> {
    match (from, to) {
        (ExecutionState::Idle, ExecutionState::Queued) => Some(StateTransition::IdleToQueued),
        (ExecutionState::Idle, ExecutionState::Skipped) => Some(StateTransition::IdleToSkipped),
        (ExecutionState::Queued, ExecutionState::Running) => Some(StateTransition::QueuedToRunning),
        (ExecutionState::Queued, ExecutionState::Skipped) => Some(StateTransition::QueuedToSkipped),
        (ExecutionState::Running, ExecutionState::Completed) => {
            Some(StateTransition::RunningToCompleted)
        }
        (ExecutionState::Running, ExecutionState::Failed) => Some(StateTransition::RunningToFailed),
        (ExecutionState::Idle, ExecutionState::Idle)
        | (ExecutionState::Queued, ExecutionState::Queued)
        | (ExecutionState::Running, ExecutionState::Running)
        | (ExecutionState::Completed, ExecutionState::Completed)
        | (ExecutionState::Failed, ExecutionState::Failed)
        | (ExecutionState::Skipped, ExecutionState::Skipped) => None,
        _ => None,
    }
}

#[must_use]
pub fn can_transition(from: ExecutionState, to: ExecutionState) -> bool {
    try_transition(from, to).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_six_variants() {
        let states = [
            ExecutionState::Idle,
            ExecutionState::Queued,
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Skipped,
        ];
        assert_eq!(states.len(), 6);
    }

    #[test]
    fn variant_idle_exists() {
        let state = ExecutionState::Idle;
        assert!(!state.is_active());
        assert!(!state.is_terminal());
    }

    #[test]
    fn variant_queued_exists() {
        let state = ExecutionState::Queued;
        assert!(state.is_active());
        assert!(!state.is_terminal());
    }

    #[test]
    fn variant_running_exists() {
        let state = ExecutionState::Running;
        assert!(state.is_active());
        assert!(!state.is_terminal());
    }

    #[test]
    fn variant_completed_exists() {
        let state = ExecutionState::Completed;
        assert!(!state.is_active());
        assert!(state.is_terminal());
    }

    #[test]
    fn variant_failed_exists() {
        let state = ExecutionState::Failed;
        assert!(!state.is_active());
        assert!(state.is_terminal());
    }

    #[test]
    fn variant_skipped_exists() {
        let state = ExecutionState::Skipped;
        assert!(!state.is_active());
        assert!(state.is_terminal());
    }

    #[test]
    fn display_outputs_lowercase() {
        assert_eq!(ExecutionState::Idle.to_string(), "idle");
        assert_eq!(ExecutionState::Queued.to_string(), "queued");
        assert_eq!(ExecutionState::Running.to_string(), "running");
        assert_eq!(ExecutionState::Completed.to_string(), "completed");
        assert_eq!(ExecutionState::Failed.to_string(), "failed");
        assert_eq!(ExecutionState::Skipped.to_string(), "skipped");
    }

    #[test]
    fn is_terminal_returns_true_for_terminal_states() {
        assert!(ExecutionState::Completed.is_terminal());
        assert!(ExecutionState::Failed.is_terminal());
        assert!(ExecutionState::Skipped.is_terminal());
    }

    #[test]
    fn is_terminal_returns_false_for_non_terminal_states() {
        assert!(!ExecutionState::Idle.is_terminal());
        assert!(!ExecutionState::Queued.is_terminal());
        assert!(!ExecutionState::Running.is_terminal());
    }

    #[test]
    fn is_active_returns_true_for_active_states() {
        assert!(ExecutionState::Running.is_active());
        assert!(ExecutionState::Queued.is_active());
    }

    #[test]
    fn is_active_returns_false_for_inactive_states() {
        assert!(!ExecutionState::Idle.is_active());
        assert!(!ExecutionState::Completed.is_active());
        assert!(!ExecutionState::Failed.is_active());
        assert!(!ExecutionState::Skipped.is_active());
    }

    #[test]
    fn default_is_idle() {
        assert_eq!(ExecutionState::default(), ExecutionState::Idle);
    }

    #[test]
    fn serialization_roundtrip() {
        let states = [
            ExecutionState::Idle,
            ExecutionState::Queued,
            ExecutionState::Running,
            ExecutionState::Completed,
            ExecutionState::Failed,
            ExecutionState::Skipped,
        ];

        for state in states {
            let json = serde_json::to_string(&state).expect("serialize");
            let deserialized: ExecutionState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn serialization_uses_lowercase() {
        assert_eq!(
            serde_json::to_string(&ExecutionState::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionState::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionState::Completed).unwrap(),
            "\"completed\""
        );
    }

    #[test]
    fn deserialization_accepts_lowercase() {
        let state: ExecutionState = serde_json::from_str("\"idle\"").unwrap();
        assert_eq!(state, ExecutionState::Idle);

        let state: ExecutionState = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(state, ExecutionState::Running);

        let state: ExecutionState = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(state, ExecutionState::Completed);
    }

    #[test]
    fn transition_idle_to_running_is_valid() {
        let from = ExecutionState::Idle;
        let to = ExecutionState::Running;
        assert!(from != to);
    }

    #[test]
    fn transition_running_to_completed_is_valid() {
        let from = ExecutionState::Running;
        let to = ExecutionState::Completed;
        assert!(from != to);
    }

    #[test]
    fn transition_running_to_failed_is_valid() {
        let from = ExecutionState::Running;
        let to = ExecutionState::Failed;
        assert!(from != to);
    }

    #[test]
    fn transition_queued_to_running_is_valid() {
        let from = ExecutionState::Queued;
        let to = ExecutionState::Running;
        assert!(from != to);
    }

    #[test]
    fn skipped_is_external_transition() {
        let state = ExecutionState::Skipped;
        assert!(state.is_terminal());
    }

    mod state_machine {
        use super::*;

        #[test]
        fn idle_can_queue() {
            let idle = IdleState;
            let queued = idle.queue();
            assert_eq!(ExecutionState::Queued, queued.into());
        }

        #[test]
        fn idle_can_skip() {
            let idle = IdleState;
            let skipped = idle.skip();
            assert_eq!(ExecutionState::Skipped, skipped.into());
        }

        #[test]
        fn queued_can_start() {
            let queued = QueuedState;
            let running = queued.start();
            assert_eq!(ExecutionState::Running, running.into());
        }

        #[test]
        fn queued_can_skip() {
            let queued = QueuedState;
            let skipped = queued.skip();
            assert_eq!(ExecutionState::Skipped, skipped.into());
        }

        #[test]
        fn running_can_complete() {
            let running = RunningState;
            let completed = running.complete();
            assert_eq!(ExecutionState::Completed, completed.into());
        }

        #[test]
        fn running_can_fail() {
            let running = RunningState;
            let failed = running.fail();
            assert_eq!(ExecutionState::Failed, failed.into());
        }

        impl From<IdleState> for ExecutionState {
            fn from(_: IdleState) -> Self {
                Self::Idle
            }
        }

        impl From<QueuedState> for ExecutionState {
            fn from(_: QueuedState) -> Self {
                Self::Queued
            }
        }

        impl From<RunningState> for ExecutionState {
            fn from(_: RunningState) -> Self {
                Self::Running
            }
        }

        impl From<CompletedState> for ExecutionState {
            fn from(_: CompletedState) -> Self {
                Self::Completed
            }
        }

        impl From<FailedState> for ExecutionState {
            fn from(_: FailedState) -> Self {
                Self::Failed
            }
        }

        impl From<SkippedState> for ExecutionState {
            fn from(_: SkippedState) -> Self {
                Self::Skipped
            }
        }
    }

    mod state_transitions {
        use super::*;

        #[test]
        fn try_transition_idle_to_queued_is_valid() {
            let transition = try_transition(ExecutionState::Idle, ExecutionState::Queued);
            assert!(transition.is_some());
            assert_eq!(transition.unwrap().apply(), ExecutionState::Queued);
        }

        #[test]
        fn try_transition_idle_to_running_is_invalid() {
            let transition = try_transition(ExecutionState::Idle, ExecutionState::Running);
            assert!(transition.is_none());
        }

        #[test]
        fn try_transition_running_to_completed_is_valid() {
            let transition = try_transition(ExecutionState::Running, ExecutionState::Completed);
            assert!(transition.is_some());
        }

        #[test]
        fn try_transition_completed_to_running_is_invalid() {
            let transition = try_transition(ExecutionState::Completed, ExecutionState::Running);
            assert!(transition.is_none());
        }

        #[test]
        fn can_transition_validates_correctly() {
            assert!(can_transition(ExecutionState::Idle, ExecutionState::Queued));
            assert!(can_transition(
                ExecutionState::Queued,
                ExecutionState::Running
            ));
            assert!(can_transition(
                ExecutionState::Running,
                ExecutionState::Completed
            ));
            assert!(!can_transition(
                ExecutionState::Completed,
                ExecutionState::Running
            ));
            assert!(!can_transition(
                ExecutionState::Failed,
                ExecutionState::Queued
            ));
        }

        #[test]
        fn invalid_transition_display_shows_states() {
            let err = InvalidTransition {
                from: ExecutionState::Completed,
                to: ExecutionState::Running,
            };
            assert_eq!(
                err.to_string(),
                "Invalid state transition: completed -> running"
            );
        }
    }
}
