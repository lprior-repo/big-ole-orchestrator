//! Hierarchical lifecycle superstates (ADR-039).
//!
//! Top-level grouping of the flat [`LifecycleState`](super::state::LifecycleState)
//! into broader operational categories used by the scheduler, visibility layer,
//! and compensation planner.

use serde::{Deserialize, Serialize};

/// Hierarchical superstate grouping for the flat [`LifecycleState`](super::state::LifecycleState) (ADR-039).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    use crate::state::{BlockedReason, LifecycleState, OperationalStatus, TransitionEvent};

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
    fn waiting_for_signal_maps_to_suspended() {
        assert_eq!(
            LifecycleState::WaitingForSignal.superstate(),
            LifecycleSuperstate::Suspended
        );
    }

    #[test]
    fn pending_publication_maps_to_suspended() {
        assert_eq!(
            LifecycleState::PendingPublication.superstate(),
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

    // --- LifecycleState serde roundtrip (ADR-039) ---

    #[test]
    fn lifecycle_state_pending_round_trips_via_serde() {
        let json = "\"pending\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::Pending);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_running_decision_round_trips_via_serde() {
        let json = "\"running_decision\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::RunningDecision);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_step_scheduled_round_trips_via_serde() {
        let json = "\"step_scheduled\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::StepScheduled);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_step_executing_round_trips_via_serde() {
        let json = "\"step_executing\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::StepExecuting);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_waiting_for_timer_round_trips_via_serde() {
        let json = "\"waiting_for_timer\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::WaitingForTimer);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_waiting_for_signal_round_trips_via_serde() {
        let json = "\"waiting_for_signal\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::WaitingForSignal);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_completed_round_trips_via_serde() {
        let json = "\"completed\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::Completed);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_failed_round_trips_via_serde() {
        let json = "\"failed\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::Failed);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_cancelled_round_trips_via_serde() {
        let json = "\"cancelled\"";
        let parsed: LifecycleState = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, LifecycleState::Cancelled);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn lifecycle_state_rejects_unknown_variant() {
        let result: Result<LifecycleState, _> = serde_json::from_str("\"zombie\"");
        assert!(result.is_err(), "should reject unknown lifecycle state");
    }

    #[test]
    fn lifecycle_state_all_variants_exhaustive() {
        use crate::state::LifecycleState;
        let states = [
            LifecycleState::Pending,
            LifecycleState::RunningDecision,
            LifecycleState::StepScheduled,
            LifecycleState::StepExecuting,
            LifecycleState::WaitingForTimer,
            LifecycleState::WaitingForSignal,
            LifecycleState::PendingPublication,
            LifecycleState::Completed,
            LifecycleState::Failed,
            LifecycleState::Cancelled,
        ];
        for state in states {
            let json = serde_json::to_string(&state).expect("serialize");
            let restored: LifecycleState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, state, "roundtrip failed for {:?}", state);
        }
    }

    // --- TransitionEvent serde roundtrip ---

    #[test]
    fn transition_event_all_variants_exhaustive() {
        use crate::state::TransitionEvent;
        let events = TransitionEvent::all_variants();
        for event in events {
            let json = serde_json::to_string(event).expect("serialize");
            let restored: TransitionEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, *event, "roundtrip failed for {:?}", event);
        }
    }

    #[test]
    fn transition_event_rejects_unknown_variant() {
        let result: Result<TransitionEvent, _> = serde_json::from_str("\"fake_event\"");
        assert!(result.is_err(), "should reject unknown transition event");
    }

    // --- OperationalStatus serde roundtrip ---

    #[test]
    fn operational_status_healthy_round_trips_via_serde() {
        let json = "\"healthy\"";
        let parsed: OperationalStatus = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, OperationalStatus::Healthy);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn operational_status_recovering_round_trips_via_serde() {
        let json = "\"recovering\"";
        let parsed: OperationalStatus = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(parsed, OperationalStatus::Recovering);
        let roundtrip = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(roundtrip, json);
    }

    #[test]
    fn operational_status_blocked_round_trips_via_serde() {
        let original = OperationalStatus::Blocked(BlockedReason::DependenciesPending);
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: OperationalStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn operational_status_all_variants_exhaustive() {
        let variants = [
            OperationalStatus::Healthy,
            OperationalStatus::Recovering,
            OperationalStatus::Blocked(BlockedReason::DependenciesPending),
            OperationalStatus::Blocked(BlockedReason::ResourceContention),
            OperationalStatus::Blocked(BlockedReason::ManualHold),
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).expect("serialize");
            let restored: OperationalStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, variant, "roundtrip failed for {:?}", variant);
        }
    }

    // --- BlockedReason serde roundtrip ---

    #[test]
    fn blocked_reason_all_variants_exhaustive() {
        let reasons = [
            BlockedReason::DependenciesPending,
            BlockedReason::ResourceContention,
            BlockedReason::ManualHold,
        ];
        for reason in &reasons {
            let json = serde_json::to_string(reason).expect("serialize");
            let restored: BlockedReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, *reason, "roundtrip failed for {:?}", reason);
        }
    }
}
