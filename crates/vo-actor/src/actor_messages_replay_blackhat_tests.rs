//! BLACKHAT adversarial tests for message replay attacks.
//!
//! These tests probe whether actor messages can be replayed to cause
//! duplicate processing or other security issues.
//!
//! bead_id: ve-3l6el
//! bead_title: BLACKHAT: vo-actor — actor_messages — message replay attack
//! module: vo-actor (actor message replay protection)

use vo_types::{InstanceId, NodeName, SequenceNumber, TimerId};

use crate::actor_messages::{
    ControlActorMessage, InstanceActorMessage,
};
use crate::WaitKey;

// =============================================================================
// Helper Functions
// =============================================================================

fn make_instance_id(seed: u8) -> InstanceId {
    InstanceId::from_bytes([seed; 16])
}

fn make_node_name(s: &str) -> NodeName {
    NodeName::parse(s).expect("valid node name")
}

fn make_timer_id(seed: u8) -> TimerId {
    TimerId::from_bytes([seed; 16])
}

// =============================================================================
// BLACKHAT: Message Replay Attack Tests
// ADR-016: Message replay protection
// EARS Ubiquitous: THE SYSTEM SHALL prevent message replay attacks
// EARS Event-Driven: When duplicate message received, THE SYSTEM SHALL detect and reject
// EARS Unwanted: If message replay succeeds, THE SYSTEM SHALL process duplicate
// =============================================================================

#[cfg(test)]
mod message_replay_attack {
    use super::*;

    // BH-MR01: InstanceActorMessage can be cloned and resent (replay vulnerability)
    // EARS Ubiquitous: THE SYSTEM SHALL prevent message replay attacks
    #[test]
    fn bh_instance_message_replay_via_clone() {
        let instance_id = make_instance_id(1);

        // Original StepCompleted message
        let original = InstanceActorMessage::StepCompleted {
            instance_id: instance_id.clone(),
            node_name: make_node_name("node-1"),
            sequence: SequenceNumber::new(1),
        };

        // Replay: Clone the exact same message (simulating network retry)
        let replay = original.clone();

        // The messages are identical - no built-in deduplication
        assert_eq!(original, replay, "Messages are identical after clone - potential replay");

        // Extract the sequence number - both have same sequence
        let InstanceActorMessage::StepCompleted { sequence: seq_orig, .. } = &original;
        let InstanceActorMessage::StepCompleted { sequence: seq_replay, .. } = &replay;

        assert_eq!(
            seq_orig, seq_replay,
            "BH-MR01 VULNERABILITY: Same sequence number on cloned message - no unique message ID"
        );
    }

    // BH-MR02: StepFailed message replay leaves no trace of duplicate
    // EARS Event-Driven: When duplicate message received, THE SYSTEM SHALL detect and reject
    #[test]
    fn bh_step_failed_replay_indistinguishable() {
        let instance_id = make_instance_id(1);

        let original = InstanceActorMessage::StepFailed {
            instance_id: instance_id.clone(),
            node_name: make_node_name("node-1"),
            sequence: SequenceNumber::new(1),
            error: "Connection timeout".to_string(),
        };

        let replay = original.clone();

        // Both messages have same content - no message ID for deduplication
        assert_eq!(original, replay);

        // If these were processed by an actor, the actor cannot distinguish
        // original from replay based on message content alone
        let InstanceActorMessage::StepFailed {
            error: err_orig,
            sequence: seq_orig,
            ..
        } = &original;
        let InstanceActorMessage::StepFailed {
            error: err_replay,
            sequence: seq_replay,
            ..
        } = &replay;

        assert_eq!(err_orig, err_replay);
        assert_eq!(seq_orig, seq_replay);
        // BH-MR02: No unique identifier to detect replay
    }

    // BH-MR03: TimerFired without unique timer fire ID enables replay
    // EARS Ubiquitous: THE SYSTEM SHALL prevent message replay attacks
    #[test]
    fn bh_timer_fired_replay_same_timer_id() {
        let instance_id = make_instance_id(1);
        let timer_id = make_timer_id(42);

        let original = InstanceActorMessage::TimerFired {
            instance_id: instance_id.clone(),
            timer_id,
        };

        // Simulate network retry sending same TimerFired
        let replay = InstanceActorMessage::TimerFired {
            instance_id: instance_id.clone(),
            timer_id, // Same timer_id
        };

        assert_eq!(original, replay);

        // BH-MR03: If a workflow resumes on TimerFired, replay could cause double-resume
        // No fire-count or unique fire ID to prevent double-fire
        let same_instance = matches!((&original, &replay),
            (InstanceActorMessage::TimerFired { instance_id: i1, .. },
             InstanceActorMessage::TimerFired { instance_id: i2, .. }) if i1 == i2);
        assert!(same_instance, "TimerFired messages are identical - replay vulnerability");
    }

    // BH-MR04: CancelRequested replay could cause incorrect state
    #[test]
    fn bh_cancel_requested_replay() {
        let instance_id = make_instance_id(1);

        let original = InstanceActorMessage::CancelRequested {
            instance_id: instance_id.clone(),
        };

        let replay = original.clone();

        assert_eq!(original, replay);

        // If cancel is idempotent (correct), replay is harmless
        // If cancel is not idempotent (bug), replay could cause issues
        // BH-MR04: Cannot determine from message alone if cancel is idempotent
    }

    // BH-MR05: ControlActorMessage AcceptAndResume replay vulnerability
    // EARS Unwanted: If message replay succeeds, THE SYSTEM SHALL process duplicate
    #[test]
    fn bh_accept_and_resume_replay_same_signal() {
        let instance_id = make_instance_id(1);
        let wait_key = WaitKey::parse("approval").expect("valid");
        let signal_id = "sig-123".to_string();

        let original = ControlActorMessage::AcceptAndResume {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
            signal_id: signal_id.clone(),
            payload: crate::SignalPayload::empty(),
        };

        // Network retry could send same AcceptAndResume
        let replay = ControlActorMessage::AcceptAndResume {
            instance_id: instance_id.clone(),
            wait_key: wait_key.clone(),
            signal_id: signal_id.clone(), // Same signal_id
            payload: crate::SignalPayload::empty(),
        };

        assert_eq!(original, replay);

        // If AcceptAndResume is not idempotent, replay could:
        // - Resume the instance twice
        // - Process the same signal twice
        // BH-MR05: No transaction ID or dedupe key to prevent replay
    }

    // BH-MR06: ContinueAsNew replay could create fork/branch confusion
    #[test]
    fn bh_continue_as_new_replay() {
        let instance_id = make_instance_id(1);
        let lineage_id = "lineage-abc".to_string();
        let new_instance_id = make_instance_id(2);

        let original = ControlActorMessage::ContinueAsNew {
            instance_id: instance_id.clone(),
            lineage_id: lineage_id.clone(),
            new_instance_id: new_instance_id.clone(),
        };

        let replay = ControlActorMessage::ContinueAsNew {
            instance_id: instance_id.clone(),
            lineage_id: lineage_id.clone(),
            new_instance_id: new_instance_id.clone(),
        };

        assert_eq!(original, replay);

        // BH-MR06: Replay of ContinueAsNew could cause duplicate epoch creation
    }

    // BH-MR07: StartWorkflow replay could start duplicate instances
    #[test]
    fn bh_start_workflow_replay() {
        let instance_id = make_instance_id(1);
        let workflow_name = vo_types::WorkflowName::parse("test-workflow").expect("valid");
        let node_name = make_node_name("start-node");

        let original = InstanceActorMessage::StartWorkflow {
            instance_id: instance_id.clone(),
            workflow_name: workflow_name.clone(),
            node_name: node_name.clone(),
        };

        // Network retry
        let replay = InstanceActorMessage::StartWorkflow {
            instance_id: instance_id.clone(),
            workflow_name: workflow_name.clone(),
            node_name: node_name.clone(),
        };

        assert_eq!(original, replay);

        // BH-MR07: If StartWorkflow is not idempotent, replay could
        // create duplicate workflow instances
    }

    // BH-MR08: GetStatus query replay is harmless but reveals system state
    #[test]
    fn bh_get_status_replay_reveals_state() {
        let instance_id = make_instance_id(1);

        let original = InstanceActorMessage::GetStatus {
            instance_id: instance_id.clone(),
        };

        let replay = original.clone();

        assert_eq!(original, replay);

        // BH-MR08: GetStatus replay reveals same state - information disclosure
        // if attacker can observe response timing (side channel)
    }

    // BH-MR09: Resume message without unique resume ID enables replay
    #[test]
    fn bh_resume_replay() {
        let instance_id = make_instance_id(1);

        let original = ControlActorMessage::Resume {
            instance_id: instance_id.clone(),
        };

        let replay = original.clone();

        assert_eq!(original, replay);

        // BH-MR09: No unique resume ID - cannot distinguish original from replay
    }

    // BH-MR10: Cancel message without unique cancel ID enables replay
    #[test]
    fn bh_cancel_replay() {
        let instance_id = make_instance_id(1);

        let original = ControlActorMessage::Cancel {
            instance_id: instance_id.clone(),
        };

        let replay = original.clone();

        assert_eq!(original, replay);

        // BH-MR10: No unique cancel ID - cannot distinguish original from replay
    }
}
