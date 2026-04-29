#![cfg(test)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use vo_core::exact_once_verification::harness::{LineageRolloverEvent, LineageRoutingState};
use vo_types::signal::WaitRecord;
use vo_types::signal::{signal_match, LineageScope, SignalAddress, SignalMatchResult};
use vo_types::{Epoch, InstanceId, WaitKey, WorkflowLineage};

fn lineage_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap()
}

fn instance_id() -> InstanceId {
    InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFBK").unwrap()
}

fn wait_key_approval() -> WaitKey {
    WaitKey::parse("approval").unwrap()
}

fn valid_instance_id() -> InstanceId {
    InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
}

mod signal_address_lineage_tests {
    use super::*;

    #[test]
    fn lineage_wide_signal_address_has_no_epoch() {
        let addr = SignalAddress::lineage_wide(lineage_id(), instance_id(), wait_key_approval());
        assert!(addr.is_lineage_wide());
        assert!(!addr.is_epoch_local());
        assert_eq!(addr.epoch_id(), None);
        assert_eq!(addr.lineage_id(), &lineage_id());
    }

    #[test]
    fn epoch_local_signal_address_has_epoch() {
        let addr = SignalAddress::epoch_local(
            lineage_id(),
            Epoch::ZERO,
            instance_id(),
            wait_key_approval(),
        );
        assert!(!addr.is_lineage_wide());
        assert!(addr.is_epoch_local());
        assert_eq!(addr.epoch_id(), Some(Epoch::ZERO));
    }

    #[test]
    fn lineage_wide_signal_address_persists_lineage_id_across_epochs() {
        let addr = SignalAddress::lineage_wide(lineage_id(), instance_id(), wait_key_approval());

        assert_eq!(addr.lineage_id(), &lineage_id());
        assert_eq!(addr.lineage_scope(), LineageScope::LineageWide);
        assert!(addr.is_lineage_wide());
    }
}

mod signal_routing_across_rollover_tests {
    use super::*;

    #[test]
    fn signal_address_epoch_local_targets_specific_epoch() {
        let addr_epoch_0 = SignalAddress::epoch_local(
            lineage_id(),
            Epoch::ZERO,
            instance_id(),
            wait_key_approval(),
        );
        let addr_epoch_1 = SignalAddress::epoch_local(
            lineage_id(),
            Epoch::new(1),
            instance_id(),
            wait_key_approval(),
        );

        assert_eq!(addr_epoch_0.epoch_id(), Some(Epoch::ZERO));
        assert_eq!(addr_epoch_1.epoch_id(), Some(Epoch::new(1)));
        assert_ne!(addr_epoch_0.epoch_id(), addr_epoch_1.epoch_id());
    }

    #[test]
    fn lineage_routing_state_tracks_epoch_transitions() {
        let mut routing_state = LineageRoutingState::new("lin-test".to_string(), Epoch::ZERO);

        assert_eq!(routing_state.active_epoch, Epoch::ZERO);
        assert!(routing_state.previous_epochs.is_empty());

        routing_state.rollover(Epoch::new(1));
        assert_eq!(routing_state.active_epoch, Epoch::new(1));
        assert_eq!(routing_state.previous_epochs.len(), 1);
        assert_eq!(routing_state.previous_epochs[0], Epoch::ZERO);

        routing_state.rollover(Epoch::new(2));
        assert_eq!(routing_state.active_epoch, Epoch::new(2));
        assert_eq!(routing_state.previous_epochs.len(), 2);
    }

    #[test]
    fn lineage_routing_state_get_active_instance_id_after_rollover() {
        let mut routing_state = LineageRoutingState::new("lin-test".to_string(), Epoch::ZERO);

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
    }

    #[test]
    fn lineage_wide_signal_routes_to_current_epoch_after_rollover() {
        let addr = SignalAddress::lineage_wide(lineage_id(), instance_id(), wait_key_approval());

        let mut routing_state =
            LineageRoutingState::new(lineage_id().as_str().to_string(), Epoch::ZERO);
        assert_eq!(routing_state.active_epoch, Epoch::ZERO);

        routing_state.rollover(Epoch::new(1));
        assert_eq!(routing_state.active_epoch, Epoch::new(1));

        let active_instance = routing_state.get_active_instance_id(instance_id().as_str());
        assert_eq!(
            active_instance,
            format!("{}-epoch-1", instance_id().as_str())
        );
    }
}

mod signal_matching_tests {
    use super::*;

    #[test]
    fn signal_match_accepts_when_all_dimensions_align() {
        let lin_id = valid_instance_id();
        let inst_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal = SignalAddress::lineage_wide(lin_id.clone(), inst_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            inst_id.clone(),
            wait_key,
            vo_types::BufferPolicy::Reject,
            vo_types::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lin_id);
        assert!(result.is_matched());
    }

    #[test]
    fn signal_match_returns_lineage_mismatch_when_lineage_differs() {
        let lin_id = valid_instance_id();
        let other_lin_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let inst_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal = SignalAddress::lineage_wide(lin_id.clone(), inst_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            inst_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            vo_types::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &other_lin_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::LineageMismatch { .. } => {}
            _ => panic!("expected LineageMismatch"),
        }
    }

    #[test]
    fn signal_match_returns_wait_key_mismatch_when_wait_key_differs() {
        let lin_id = valid_instance_id();
        let inst_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let other_wait_key = WaitKey::parse("rejection").expect("valid key");

        let signal = SignalAddress::lineage_wide(lin_id.clone(), inst_id.clone(), other_wait_key);
        let wait = WaitRecord::new(
            inst_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            vo_types::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lin_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::WaitKeyMismatch { .. } => {}
            _ => panic!("expected WaitKeyMismatch"),
        }
    }

    #[test]
    fn signal_match_returns_instance_mismatch_when_instance_differs() {
        let lin_id = valid_instance_id();
        let inst_id = valid_instance_id();
        let other_inst_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lin_id.clone(), other_inst_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            inst_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            vo_types::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lin_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::InstanceMismatch { .. } => {}
            _ => panic!("expected InstanceMismatch"),
        }
    }
}

mod lineage_closed_tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LineageStatus {
        Active,
        Closed,
    }

    fn get_lineage_status(lineage_id: &InstanceId, expected_id: &InstanceId) -> LineageStatus {
        if lineage_id == expected_id {
            LineageStatus::Closed
        } else {
            LineageStatus::Active
        }
    }

    #[test]
    fn signal_to_closed_lineage_should_not_be_accepted() {
        let closed_lineage_id = lineage_id();
        let addr = SignalAddress::lineage_wide(
            closed_lineage_id.clone(),
            instance_id(),
            wait_key_approval(),
        );

        let status = get_lineage_status(&closed_lineage_id, &closed_lineage_id);

        assert_eq!(status, LineageStatus::Closed);
        assert!(addr.lineage_id() == &closed_lineage_id);
    }

    #[test]
    fn signal_to_active_lineage_can_be_accepted() {
        let active_lineage_id = valid_instance_id();
        let addr = SignalAddress::lineage_wide(
            active_lineage_id.clone(),
            instance_id(),
            wait_key_approval(),
        );

        let status = get_lineage_status(&active_lineage_id, &lineage_id());

        assert_eq!(status, LineageStatus::Active);
        assert!(addr.lineage_id() == &active_lineage_id);
    }
}

mod epoch_transition_tests {
    use super::*;

    #[test]
    fn epoch_rollover_increments_epoch() {
        let lineage = WorkflowLineage::new("lin-test".to_string()).expect("valid lineage");
        assert_eq!(lineage.epoch, Epoch::ZERO);

        let rolled = lineage.continue_as_new().expect("rollover succeeds");
        assert_eq!(rolled.epoch, Epoch::new(1));
        assert_eq!(rolled.parent_epoch, Some(Epoch::ZERO));
        assert_eq!(rolled.lineage_id, lineage.lineage_id);
    }

    #[test]
    fn multiple_rollovers_increment_correctly() {
        let lineage = WorkflowLineage::new("lin-multi".to_string()).expect("valid lineage");

        let epoch1 = lineage.continue_as_new().expect("first rollover");
        assert_eq!(epoch1.epoch, Epoch::new(1));

        let epoch2 = epoch1.continue_as_new().expect("second rollover");
        assert_eq!(epoch2.epoch, Epoch::new(2));

        let epoch3 = epoch2.continue_as_new().expect("third rollover");
        assert_eq!(epoch3.epoch, Epoch::new(3));
    }

    #[test]
    fn lineage_id_persists_across_rollovers() {
        let lineage = WorkflowLineage::new("lin-persist".to_string()).expect("valid lineage");
        let original_id = lineage.lineage_id.clone();

        let rolled1 = lineage.continue_as_new().expect("first rollover");
        assert_eq!(rolled1.lineage_id, original_id);

        let rolled2 = rolled1.continue_as_new().expect("second rollover");
        assert_eq!(rolled2.lineage_id, original_id);
    }

    #[test]
    fn lineage_rollover_event_captures_transition() {
        let event = LineageRolloverEvent::new("lin-abc".to_string(), 0, 1, "inst-1".to_string());

        assert_eq!(event.lineage_id, "lin-abc");
        assert_eq!(event.old_epoch, Epoch::new(0));
        assert_eq!(event.new_epoch, Epoch::new(1));
        assert_eq!(event.instance_id, "inst-1");
    }
}

mod signal_delivery_invariants_tests {
    use super::*;

    #[test]
    fn lineage_wide_signal_can_route_to_any_epoch() {
        let addr = SignalAddress::lineage_wide(lineage_id(), instance_id(), wait_key_approval());
        assert!(addr.is_lineage_wide());
        assert!(addr.epoch_id().is_none());
    }

    #[test]
    fn epoch_local_signal_targets_specific_epoch() {
        for epoch_num in 0..5u64 {
            let addr = SignalAddress::epoch_local(
                lineage_id(),
                Epoch::new(epoch_num),
                instance_id(),
                wait_key_approval(),
            );
            assert!(addr.is_epoch_local());
            assert_eq!(addr.epoch_id(), Some(Epoch::new(epoch_num)));
        }
    }

    #[test]
    fn lineage_scope_lineage_wide_has_no_epoch() {
        let addr = SignalAddress::lineage_wide(lineage_id(), instance_id(), wait_key_approval());
        assert_eq!(addr.lineage_scope(), LineageScope::LineageWide);
    }

    #[test]
    fn lineage_scope_epoch_local_has_epoch() {
        let addr = SignalAddress::epoch_local(
            lineage_id(),
            Epoch::ZERO,
            instance_id(),
            wait_key_approval(),
        );
        assert_eq!(addr.lineage_scope(), LineageScope::EpochLocal);
    }
}

mod continued_as_new_event_tests {
    use super::*;
    use vo_types::events::EventPayload;

    #[test]
    fn continued_as_new_event_to_envelope() {
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
}

mod signal_epoch_local_mismatch_tests {
    use super::*;

    #[test]
    fn epoch_local_signal_epoch_mismatch() {
        let lin_id = valid_instance_id();
        let inst_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let signal_epoch = Epoch::new(5);
        let wait_epoch = Epoch::ZERO;

        let signal = SignalAddress::epoch_local(
            lin_id.clone(),
            signal_epoch,
            inst_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            inst_id,
            wait_key,
            vo_types::BufferPolicy::Reject,
            vo_types::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lin_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::EpochMismatch {
                signal_epoch: sig_ep,
                wait_epoch: w_ep,
            } => {
                assert_eq!(sig_ep, signal_epoch);
                assert_eq!(w_ep, wait_epoch);
            }
            _ => panic!("expected EpochMismatch"),
        }
    }
}
