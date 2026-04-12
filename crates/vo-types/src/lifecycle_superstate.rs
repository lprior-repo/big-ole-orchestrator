//! Hierarchical lifecycle superstates (ADR-039).
//!
//! Top-level grouping of the flat [`LifecycleState`](super::state::LifecycleState)
//! into broader operational categories used by the scheduler, visibility layer,
//! and compensation planner.

use serde::{Deserialize, Serialize};

/// Hierarchical superstate grouping for the flat [`LifecycleState`](super::state::LifecycleState) (ADR-039).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum LifecycleSuperstate {
    Active,
    Suspended,
    Recovering,
    Compensating,
    Terminal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LifecycleState;

    #[test]
    fn active_serializes_to_snake_case() {
        let json = serde_json::to_string(&LifecycleSuperstate::Active).unwrap();
        assert_eq!(json, "\"active\"");
    }

    #[test]
    fn suspended_round_trips_via_serde() {
        let json = "\"suspended\"";
        let result: Result<LifecycleSuperstate, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "should deserialize 'suspended': {:?}",
            result
        );
        let roundtrip = serde_json::to_string(&result.unwrap()).unwrap();
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn recovering_round_trips_via_serde() {
        let json = "\"recovering\"";
        let result: Result<LifecycleSuperstate, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "should deserialize 'recovering': {:?}",
            result
        );
        let roundtrip = serde_json::to_string(&result.unwrap()).unwrap();
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn compensating_round_trips_via_serde() {
        let json = "\"compensating\"";
        let result: Result<LifecycleSuperstate, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "should deserialize 'compensating': {:?}",
            result
        );
        let roundtrip = serde_json::to_string(&result.unwrap()).unwrap();
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn terminal_round_trips_via_serde() {
        let json = "\"terminal\"";
        let parsed: LifecycleSuperstate =
            serde_json::from_str(json).expect("should deserialize 'terminal'");
        assert_eq!(parsed, LifecycleSuperstate::Terminal);
        let roundtrip = serde_json::to_string(&parsed).unwrap();
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn rejects_unknown_variant() {
        let result: Result<LifecycleSuperstate, _> = serde_json::from_str("\"bogus\"");
        assert!(result.is_err());
    }

    // --- LifecycleState::superstate() mapping tests ---

    #[test]
    fn pending_maps_to_active_superstate() {
        assert_eq!(
            LifecycleState::Pending.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn running_decision_maps_to_active() {
        assert_eq!(
            LifecycleState::RunningDecision.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn step_scheduled_maps_to_active() {
        assert_eq!(
            LifecycleState::StepScheduled.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn step_executing_maps_to_active() {
        assert_eq!(
            LifecycleState::StepExecuting.superstate(),
            LifecycleSuperstate::Active
        );
    }

    #[test]
    fn waiting_for_timer_maps_to_suspended() {
        assert_eq!(
            LifecycleState::WaitingForTimer.superstate(),
            LifecycleSuperstate::Suspended
        );
    }

    #[test]
    fn completed_maps_to_terminal() {
        assert_eq!(
            LifecycleState::Completed.superstate(),
            LifecycleSuperstate::Terminal
        );
    }

    #[test]
    fn failed_maps_to_terminal() {
        assert_eq!(
            LifecycleState::Failed.superstate(),
            LifecycleSuperstate::Terminal
        );
    }

    #[test]
    fn cancelled_maps_to_terminal() {
        assert_eq!(
            LifecycleState::Cancelled.superstate(),
            LifecycleSuperstate::Terminal
        );
    }
}
