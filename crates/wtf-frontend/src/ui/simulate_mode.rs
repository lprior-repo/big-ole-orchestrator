#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use bytes::Bytes;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::graph::Workflow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimProceduralState {
    pub checkpoint_map: HashMap<String, String>,
    pub current_op: u32,
    pub event_log: Vec<SimWorkflowEvent>,
}

impl Default for SimProceduralState {
    fn default() -> Self {
        Self::new()
    }
}

impl SimProceduralState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoint_map: HashMap::new(),
            current_op: 0,
            event_log: Vec::new(),
        }
    }

    #[must_use]
    pub fn can_advance(&self, total_ops: usize) -> bool {
        u32::try_from(total_ops).is_ok_and(|total| self.current_op < total)
    }

    #[must_use]
    pub fn current_op_index(&self) -> u32 {
        self.current_op
    }

    pub fn provide_result(
        &mut self,
        result: String,
        activity_id: &str,
        total_ops: usize,
    ) -> Result<(), SimError> {
        if result.is_empty() {
            return Err(SimError::EmptyResult);
        }

        let total = u32::try_from(total_ops).map_err(|_| SimError::NoOpsAvailable)?;

        if self.current_op >= total {
            return Err(SimError::AlreadyCompleted);
        }

        if total_ops == 0 {
            return Err(SimError::NoOpsAvailable);
        }

        let event = SimWorkflowEvent::ActivityCompleted {
            activity_id: activity_id.to_string(),
            result: Bytes::from(result.clone()),
            duration_ms: 0,
        };

        self.checkpoint_map.insert(activity_id.to_string(), result);
        self.event_log.push(event);
        self.current_op += 1;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimWorkflowEvent {
    ActivityDispatched {
        activity_id: String,
        activity_type: String,
        payload: Bytes,
        attempt: u32,
    },
    ActivityCompleted {
        activity_id: String,
        result: Bytes,
        duration_ms: u64,
    },
    ActivityFailed {
        activity_id: String,
        error: String,
        retries_exhausted: bool,
    },
    TimerScheduled {
        timer_id: String,
        fire_at: chrono::DateTime<Utc>,
    },
    TimerFired {
        timer_id: String,
    },
    SignalReceived {
        signal_name: String,
        payload: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimOp {
    CtxActivity {
        activity_id: String,
        activity_type: String,
    },
    CtxSleep {
        timer_id: String,
        duration_ms: u64,
    },
    CtxWaitSignal {
        signal_name: String,
    },
}

impl SimOp {
    pub fn activity_id(&self) -> Option<&str> {
        match self {
            Self::CtxActivity { activity_id, .. } => Some(activity_id),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::CtxActivity { .. } => "Activity",
            Self::CtxSleep { .. } => "Sleep",
            Self::CtxWaitSignal { .. } => "Wait Signal",
        }
    }
}

pub fn extract_ctx_ops_from_workflow(_workflow: &Workflow) -> Vec<SimOp> {
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimError {
    EmptyResult,
    AlreadyCompleted,
    NoOpsAvailable,
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyResult => write!(f, "result cannot be empty"),
            Self::AlreadyCompleted => write!(f, "simulation already completed"),
            Self::NoOpsAvailable => write!(f, "no operations available in workflow"),
        }
    }
}

impl std::error::Error for SimError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_has_empty_checkpoint_map() {
        let state = SimProceduralState::new();
        assert!(state.checkpoint_map.is_empty());
    }

    #[test]
    fn initial_state_has_zero_current_op() {
        let state = SimProceduralState::new();
        assert_eq!(state.current_op, 0);
    }

    #[test]
    fn initial_state_has_empty_event_log() {
        let state = SimProceduralState::new();
        assert!(state.event_log.is_empty());
    }

    #[test]
    fn provide_result_appends_activity_completed_to_event_log() {
        let mut state = SimProceduralState::new();
        state
            .provide_result("success".to_string(), "act-001", 3)
            .unwrap();
        assert_eq!(state.event_log.len(), 1);
        assert!(matches!(
            state.event_log[0],
            SimWorkflowEvent::ActivityCompleted { activity_id, .. }
                if activity_id == "act-001"
        ));
    }

    #[test]
    fn provide_result_adds_to_checkpoint_map() {
        let mut state = SimProceduralState::new();
        state
            .provide_result("result-value".to_string(), "act-001", 3)
            .unwrap();
        assert_eq!(
            state.checkpoint_map.get("act-001"),
            Some(&"result-value".to_string())
        );
    }

    #[test]
    fn provide_result_increments_current_op() {
        let mut state = SimProceduralState::new();
        assert_eq!(state.current_op, 0);
        state
            .provide_result("ok".to_string(), "act-001", 3)
            .unwrap();
        assert_eq!(state.current_op, 1);
    }

    #[test]
    fn multiple_provide_result_calls_accumulate() {
        let mut state = SimProceduralState::new();
        state
            .provide_result("r1".to_string(), "act-001", 5)
            .unwrap();
        state
            .provide_result("r2".to_string(), "act-002", 5)
            .unwrap();
        assert_eq!(state.checkpoint_map.len(), 2);
        assert_eq!(state.current_op, 2);
    }

    #[test]
    fn can_advance_returns_true_when_ops_remaining() {
        let state = SimProceduralState {
            current_op: 1,
            ..Default::default()
        };
        assert!(state.can_advance(3));
    }

    #[test]
    fn can_advance_returns_false_at_end() {
        let state = SimProceduralState {
            current_op: 3,
            ..Default::default()
        };
        assert!(!state.can_advance(3));
    }

    #[test]
    fn provide_result_returns_empty_result_error_when_result_is_empty() {
        let mut state = SimProceduralState::new();
        let result = state.provide_result(String::new(), "act-001", 3);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SimError::EmptyResult));
    }

    #[test]
    fn provide_result_returns_already_completed_when_at_end() {
        let mut state = SimProceduralState {
            current_op: 5,
            checkpoint_map: HashMap::new(),
            event_log: Vec::new(),
        };
        let result = state.provide_result("ok".to_string(), "act-1", 5);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SimError::AlreadyCompleted));
    }

    #[test]
    fn provide_result_returns_no_ops_when_ops_list_empty() {
        let mut state = SimProceduralState::new();
        let result = state.provide_result("ok".to_string(), "act-1", 0);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SimError::NoOpsAvailable));
    }

    #[test]
    fn invariant_current_op_never_exceeds_ops_length() {
        let mut state = SimProceduralState::new();
        for i in 0..5 {
            let result = state.provide_result(format!("r{i}"), format!("act-{i}"), 5);
            assert!(result.is_ok());
        }
        assert!(!state.can_advance(5));
    }

    #[test]
    fn invariant_checkpoint_map_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        for i in 0..3 {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.checkpoint_map.len(), state.current_op as usize);
        }
    }

    #[test]
    fn invariant_event_log_len_matches_current_op() {
        let mut state = SimProceduralState::new();
        for i in 0..3 {
            state
                .provide_result(format!("r{i}"), format!("act-{i}"), 3)
                .unwrap();
            assert_eq!(state.event_log.len(), state.current_op as usize);
        }
    }

    #[test]
    fn checkpoint_map_is_append_only() {
        let mut state = SimProceduralState::new();
        state
            .provide_result("r1".to_string(), "act-001", 3)
            .unwrap();
        state
            .provide_result("r2".to_string(), "act-002", 3)
            .unwrap();
        assert!(state.checkpoint_map.contains_key("act-001"));
        assert!(state.checkpoint_map.contains_key("act-002"));
    }

    #[test]
    fn event_log_is_append_only() {
        let mut state = SimProceduralState::new();
        state
            .provide_result("r1".to_string(), "act-001", 3)
            .unwrap();
        state
            .provide_result("r2".to_string(), "act-002", 3)
            .unwrap();
        assert_eq!(state.event_log.len(), 2);
    }

    #[test]
    fn sim_op_activity_id_returns_correct_id() {
        let op = SimOp::CtxActivity {
            activity_id: "test-act".to_string(),
            activity_type: "test-type".to_string(),
        };
        assert_eq!(op.activity_id(), Some("test-act"));
    }

    #[test]
    fn sim_op_activity_id_returns_none_for_non_activity() {
        let op = SimOp::CtxSleep {
            timer_id: "tmr-001".to_string(),
            duration_ms: 1000,
        };
        assert_eq!(op.activity_id(), None);
    }

    #[test]
    fn sim_op_label_returns_correct_labels() {
        assert_eq!(
            SimOp::CtxActivity {
                activity_id: String::new(),
                activity_type: String::new()
            }
            .label(),
            "Activity"
        );
        assert_eq!(
            SimOp::CtxSleep {
                timer_id: String::new(),
                duration_ms: 0
            }
            .label(),
            "Sleep"
        );
        assert_eq!(
            SimOp::CtxWaitSignal {
                signal_name: String::new()
            }
            .label(),
            "Wait Signal"
        );
    }
}
