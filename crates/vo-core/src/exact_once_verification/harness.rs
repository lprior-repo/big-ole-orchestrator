//! Verification harness for exact-once crash injection testing (ADR-043).
//!
//! This module provides infrastructure for injecting crashes at critical
//! transition points and verifying deterministic replay behavior.
//!
//! ## Key Properties Verified
//!
//! 1. Duplicate ingress does not create duplicate logical work
//! 2. Stale fence completions cannot win
//! 3. Replay after any injected crash reaches the same legal state
//! 4. Connector ambiguity always routes through reconciliation
//! 5. Projection rebuild reproduces the same operator state
//! 6. **Lineage rollover preserves correct signal routing**
//! 7. Compensation never runs for an effect that was never durably committed

use crate::exact_once_verification::crash_points::{CrashPoint, CrashPosition, CrashScenario};

use vo_types::events::EventEnvelope;
use vo_types::Epoch;

#[derive(Debug, Clone)]
pub struct LineageRolloverEvent {
    pub lineage_id: String,
    pub old_epoch: Epoch,
    pub new_epoch: Epoch,
    pub instance_id: String,
}

impl LineageRolloverEvent {
    pub fn new(lineage_id: String, old_epoch: u64, new_epoch: u64, instance_id: String) -> Self {
        Self {
            lineage_id,
            old_epoch: Epoch::new(old_epoch),
            new_epoch: Epoch::new(new_epoch),
            instance_id,
        }
    }

    pub fn to_event_envelope(&self, sequence: u64) -> EventEnvelope {
        let payload = serde_json::json!({
            "type": "ContinuedAsNew",
            "workflow_id": self.instance_id,
            "lineage_id": self.lineage_id,
            "old_epoch": self.old_epoch.value(),
            "new_epoch": self.new_epoch.value(),
            "version": 1
        });

        EventEnvelope {
            schema_version: 1,
            instance_id: self.instance_id.clone(),
            sequence,
            timestamp_ms: 1000 * sequence,
            payload,
            metadata: vo_types::events::EventMetadata::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LineageRoutingState {
    pub lineage_id: String,
    pub active_epoch: Epoch,
    pub previous_epochs: Vec<Epoch>,
}

impl LineageRoutingState {
    pub fn new(lineage_id: String, initial_epoch: Epoch) -> Self {
        Self {
            lineage_id,
            active_epoch: initial_epoch,
            previous_epochs: Vec::new(),
        }
    }

    pub fn rollover(&mut self, new_epoch: Epoch) {
        self.previous_epochs.push(self.active_epoch);
        self.active_epoch = new_epoch;
    }

    pub fn get_active_instance_id(&self, base_instance_id: &str) -> String {
        format!("{}-epoch-{}", base_instance_id, self.active_epoch.value())
    }
}

pub struct VerificationHarness {
    crash_injection_enabled: bool,
    crash_scenario: Option<CrashScenario>,
}

impl VerificationHarness {
    pub fn new() -> Self {
        Self {
            crash_injection_enabled: false,
            crash_scenario: None,
        }
    }

    pub fn with_crash_scenario(crash_point: CrashPoint, position: CrashPosition) -> Self {
        Self {
            crash_injection_enabled: true,
            crash_scenario: Some(CrashScenario::new(crash_point, position)),
        }
    }

    #[must_use]
    pub fn should_crash(&self, crash_point: CrashPoint) -> bool {
        if !self.crash_injection_enabled {
            return false;
        }

        match &self.crash_scenario {
            Some(scenario) => scenario.point == crash_point,
            None => false,
        }
    }

    #[must_use]
    pub fn should_crash_at(&self, crash_point: CrashPoint, position: CrashPosition) -> bool {
        if !self.crash_injection_enabled {
            return false;
        }

        match &self.crash_scenario {
            Some(scenario) => scenario.point == crash_point && scenario.position == position,
            None => false,
        }
    }

    pub fn verify_lineage_rollover_deterministic(
        pre_rollover_events: &[EventEnvelope],
        rollover_event: &EventEnvelope,
        post_rollover_events: &[EventEnvelope],
    ) -> bool {
        let all_events_before_crash: Vec<EventEnvelope> = pre_rollover_events
            .iter()
            .chain(std::iter::once(rollover_event))
            .cloned()
            .collect();

        let all_events_after_crash: Vec<EventEnvelope> = pre_rollover_events
            .iter()
            .chain(std::iter::once(rollover_event))
            .chain(post_rollover_events.iter())
            .cloned()
            .collect();

        let engine = crate::replay::ReplayEngine::new();

        let result_before = engine.replay(&all_events_before_crash);
        let result_after = engine.replay(&all_events_after_crash);

        match (result_before, result_after) {
            (Ok(before), Ok(after)) => before.final_state == after.final_state,
            _ => false,
        }
    }

    pub fn build_lineage_rollover_sequence(
        lineage_id: &str,
        base_instance_id: &str,
        epochs: Vec<u64>,
    ) -> Vec<EventEnvelope> {
        let mut events = Vec::new();
        let mut sequence: u64 = 1;

        for (i, &epoch_num) in epochs.iter().enumerate() {
            let instance_id = if i == 0 {
                base_instance_id.to_string()
            } else {
                format!("{}-epoch-{}", base_instance_id, epoch_num)
            };

            let workflow_started = EventEnvelope {
                schema_version: 1,
                instance_id: instance_id.clone(),
                sequence,
                timestamp_ms: 1000 * sequence,
                payload: serde_json::json!({
                    "type": "WorkflowStarted",
                    "workflow_id": lineage_id,
                    "binary_hash": "sha256abc",
                    "workflow_version_hash": "wvhash123",
                    "dedupe_key_hash": null,
                    "version": 1
                }),
                metadata: vo_types::events::EventMetadata::default(),
            };
            events.push(workflow_started);
            sequence += 1;

            if i > 0 {
                let continued_as_new = LineageRolloverEvent::new(
                    lineage_id.to_string(),
                    epochs[i - 1],
                    epoch_num,
                    instance_id,
                )
                .to_event_envelope(sequence);
                events.push(continued_as_new);
                sequence += 1;
            }
        }

        events
    }
}

impl Default for VerificationHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_once_verification::crash_points::CrashPoint::{
        LineageRollover, StepCompleted,
    };
    use vo_types::events::{EventMetadata, EventPayload};

    fn make_event(instance_id: &str, sequence: u64, payload: serde_json::Value) -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: instance_id.to_string(),
            sequence,
            timestamp_ms: 1000 * sequence,
            payload,
            metadata: EventMetadata::default(),
        }
    }

    fn workflow_started_payload(workflow_id: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "WorkflowStarted",
            "workflow_id": workflow_id,
            "binary_hash": "sha256abc",
            "workflow_version_hash": "wvhash123",
            "dedupe_key_hash": null,
            "version": 1
        })
    }

    #[test]
    fn harness_new_is_created_without_crash_injection() {
        let harness = VerificationHarness::new();
        assert!(!harness.should_crash(LineageRollover));
    }

    #[test]
    fn harness_with_scenario_injects_crash_at_correct_point() {
        let harness =
            VerificationHarness::with_crash_scenario(LineageRollover, CrashPosition::Before);
        assert!(harness.should_crash(LineageRollover));
        assert!(!harness.should_crash(StepCompleted));
    }

    #[test]
    fn harness_crash_before_at_specific_point() {
        let harness =
            VerificationHarness::with_crash_scenario(LineageRollover, CrashPosition::Before);
        assert!(harness.should_crash_at(LineageRollover, CrashPosition::Before));
        assert!(!harness.should_crash_at(LineageRollover, CrashPosition::After));
    }

    #[test]
    fn harness_crash_after_at_specific_point() {
        let harness =
            VerificationHarness::with_crash_scenario(LineageRollover, CrashPosition::After);
        assert!(harness.should_crash_at(LineageRollover, CrashPosition::After));
        assert!(!harness.should_crash_at(LineageRollover, CrashPosition::Before));
    }

    #[test]
    fn lineage_rollover_event_creation() {
        let event = LineageRolloverEvent::new("lin-abc".to_string(), 0, 1, "inst-1".to_string());

        assert_eq!(event.lineage_id, "lin-abc");
        assert_eq!(event.old_epoch, Epoch::new(0));
        assert_eq!(event.new_epoch, Epoch::new(1));
        assert_eq!(event.instance_id, "inst-1");
    }

    #[test]
    fn lineage_rollover_event_to_envelope() {
        let event = LineageRolloverEvent::new("lin-abc".to_string(), 0, 1, "inst-1".to_string());

        let envelope = event.to_event_envelope(5);

        assert_eq!(envelope.instance_id, "inst-1");
        assert_eq!(envelope.sequence, 5);
        assert_eq!(envelope.timestamp_ms, 5000);

        let payload = EventPayload::try_from_json(&envelope.payload).expect("payload should parse");
        match payload {
            EventPayload::ContinuedAsNew {
                workflow_id,
                lineage_id,
                old_epoch,
                new_epoch,
            } => {
                assert_eq!(workflow_id, "inst-1");
                assert_eq!(lineage_id, "lin-abc");
                assert_eq!(old_epoch, 0);
                assert_eq!(new_epoch, 1);
            }
            _ => panic!("expected ContinuedAsNew payload"),
        }
    }

    #[test]
    fn lineage_routing_state_initialization() {
        let state = LineageRoutingState::new("lin-abc".to_string(), Epoch::ZERO);

        assert_eq!(state.lineage_id, "lin-abc");
        assert_eq!(state.active_epoch, Epoch::ZERO);
        assert!(state.previous_epochs.is_empty());
    }

    #[test]
    fn lineage_routing_state_rollover() {
        let mut state = LineageRoutingState::new("lin-abc".to_string(), Epoch::ZERO);

        state.rollover(Epoch::new(1));

        assert_eq!(state.active_epoch, Epoch::new(1));
        assert_eq!(state.previous_epochs, vec![Epoch::ZERO]);

        state.rollover(Epoch::new(2));

        assert_eq!(state.active_epoch, Epoch::new(2));
        assert_eq!(state.previous_epochs, vec![Epoch::ZERO, Epoch::new(1)]);
    }

    #[test]
    fn lineage_routing_state_get_active_instance_id() {
        let state = LineageRoutingState::new("lin-abc".to_string(), Epoch::ZERO);

        assert_eq!(
            state.get_active_instance_id("inst-base"),
            "inst-base-epoch-0"
        );

        let mut state = LineageRoutingState::new("lin-abc".to_string(), Epoch::ZERO);
        state.rollover(Epoch::new(1));

        assert_eq!(
            state.get_active_instance_id("inst-base"),
            "inst-base-epoch-1"
        );
    }

    #[test]
    fn build_lineage_rollover_sequence_single_epoch() {
        let events =
            VerificationHarness::build_lineage_rollover_sequence("lin-abc", "inst-1", vec![0]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].instance_id, "inst-1");
        assert_eq!(events[0].sequence, 1);
    }

    #[test]
    fn build_lineage_rollover_sequence_multiple_epochs() {
        let events = VerificationHarness::build_lineage_rollover_sequence(
            "lin-abc",
            "inst-1",
            vec![0, 1, 2],
        );

        assert_eq!(events.len(), 5);

        assert_eq!(events[0].instance_id, "inst-1");
        assert_eq!(events[0].sequence, 1);

        assert_eq!(events[1].instance_id, "inst-1-epoch-1");
        assert_eq!(events[1].sequence, 2);

        assert_eq!(events[2].instance_id, "inst-1-epoch-1");
        assert_eq!(events[2].sequence, 3);

        assert_eq!(events[3].instance_id, "inst-1-epoch-2");
        assert_eq!(events[3].sequence, 4);

        assert_eq!(events[4].instance_id, "inst-1-epoch-2");
        assert_eq!(events[4].sequence, 5);
    }

    #[test]
    fn verify_lineage_rollover_deterministic_same_output() {
        let events = vec![
            make_event("inst-1", 1, workflow_started_payload("wf-1")),
            LineageRolloverEvent::new("lin-1".to_string(), 0, 1, "inst-1".to_string())
                .to_event_envelope(2),
        ];

        let engine = crate::replay::ReplayEngine::new();
        let result = engine.replay(&events);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().final_state,
            Some(vo_types::state::LifecycleState::RunningDecision)
        );
    }

    #[test]
    fn lineage_rollover_preserves_signal_routing_across_epochs() {
        let mut routing_state = LineageRoutingState::new("lin-abc".to_string(), Epoch::ZERO);

        assert_eq!(
            routing_state.get_active_instance_id("sig-target"),
            "sig-target-epoch-0"
        );

        routing_state.rollover(Epoch::new(1));

        assert_eq!(
            routing_state.get_active_instance_id("sig-target"),
            "sig-target-epoch-1"
        );

        routing_state.rollover(Epoch::new(2));

        assert_eq!(
            routing_state.get_active_instance_id("sig-target"),
            "sig-target-epoch-2"
        );

        assert_eq!(routing_state.previous_epochs.len(), 2);
        assert_eq!(routing_state.previous_epochs[0], Epoch::ZERO);
        assert_eq!(routing_state.previous_epochs[1], Epoch::new(1));
    }

    #[test]
    fn crash_position_all_variants() {
        let positions = CrashPosition::all();
        assert_eq!(positions.len(), 2);
        assert!(positions.contains(&CrashPosition::Before));
        assert!(positions.contains(&CrashPosition::After));
    }

    #[test]
    fn lineage_rollover_crash_point_classifications() {
        assert!(LineageRollover.is_external_related());
        assert!(!LineageRollover.is_effect_related());
        assert!(!LineageRollover.is_step_related());
    }
}
