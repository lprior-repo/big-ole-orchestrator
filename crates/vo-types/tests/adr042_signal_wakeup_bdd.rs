use std::collections::HashSet;

use vo_types::{
    signal::{signal_match, SignalDedupeKey, SignalMatchResult},
    BufferPolicy, Epoch, IdempotencyKey, InstanceId, SignalAddress, TimestampMs, WaitKey,
    WaitRecord,
};

fn valid_instance_id() -> InstanceId {
    InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
}

fn other_instance_id() -> InstanceId {
    InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID for test setup")
}

// ===========================================================================
// BDD Scenario 1: Given workflow waiting on signal S,
//   When S arrives, Then exactly one wake-up occurs.
// ===========================================================================

mod exactly_one_wakeup_when_signal_matches {
    use super::*;

    #[test]
    fn given_workflow_waiting_when_matching_signal_arrives_then_exactly_one_wakeup() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_matched(),
            "BDD: When matching signal arrives for waiting workflow, exactly one wake-up must occur (Matched)"
        );
    }

    #[test]
    fn given_workflow_waiting_when_non_matching_lineage_signal_arrives_then_no_wakeup() {
        let lineage_id = valid_instance_id();
        let other_lineage = other_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let signal = SignalAddress::lineage_wide(
            other_lineage.clone(),
            instance_id.clone(),
            wait_key.clone(),
        );

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_mismatch(),
            "BDD: Signal from different lineage must not wake this workflow"
        );
        assert!(
            matches!(result, SignalMatchResult::LineageMismatch { .. }),
            "BDD: Must be LineageMismatch, not any other mismatch variant"
        );
    }

    #[test]
    fn given_workflow_waiting_when_non_matching_instance_signal_arrives_then_no_wakeup() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let other_instance = other_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let wait = WaitRecord::new(
            instance_id,
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), other_instance, wait_key.clone());

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_mismatch(),
            "BDD: Signal for different instance must not wake this workflow"
        );
        assert!(
            matches!(result, SignalMatchResult::InstanceMismatch { .. }),
            "BDD: Must be InstanceMismatch"
        );
    }

    #[test]
    fn given_workflow_waiting_when_non_matching_wait_key_signal_arrives_then_no_wakeup() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key,
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let other_key = WaitKey::parse("rejection").expect("valid key");
        let signal = SignalAddress::lineage_wide(lineage_id.clone(), instance_id, other_key);

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_mismatch(),
            "BDD: Signal with different wait_key must not wake this workflow"
        );
        assert!(
            matches!(result, SignalMatchResult::WaitKeyMismatch { .. }),
            "BDD: Must be WaitKeyMismatch"
        );
    }

    #[test]
    fn given_workflow_waiting_when_multiple_non_matching_signals_arrive_then_still_no_wakeup() {
        let lineage_id = valid_instance_id();
        let other_lineage = other_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let wrong_lineage_signal = SignalAddress::lineage_wide(
            other_lineage.clone(),
            instance_id.clone(),
            wait_key.clone(),
        );
        let wrong_key_signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            WaitKey::parse("wrong-key").expect("valid key"),
        );
        let wrong_instance_signal =
            SignalAddress::lineage_wide(lineage_id.clone(), other_lineage, wait_key.clone());

        let signals = [
            &wrong_lineage_signal,
            &wrong_key_signal,
            &wrong_instance_signal,
        ];

        for signal in signals {
            let result = signal_match(signal, &wait, &lineage_id, Epoch::ZERO);
            assert!(
                result.is_mismatch(),
                "BDD: Non-matching signal must never wake workflow (no spurious wake-ups)"
            );
        }
    }
}

// ===========================================================================
// BDD Scenario 2: Given signal arriving before wait,
//   When workflow reaches wait, Then immediate wake-up.
//
// This tests that signal_match is idempotent: a signal address constructed
// before the wait record will still match once the wait record exists,
// simulating "buffered" or "pre-arrived" signals that trigger immediate
// wake-up when the wait is registered.
// ===========================================================================

mod immediate_wakeup_when_signal_arrived_before_wait {
    use super::*;

    #[test]
    fn given_signal_constructed_before_wait_when_wait_is_registered_then_match_succeeds() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("gate-open").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_matched(),
            "BDD: Signal that arrived before wait must match and trigger immediate wake-up"
        );
    }

    #[test]
    fn given_pre_arrived_signal_with_buffer_one_when_wait_registered_then_match_succeeds() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("buffered-signal").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::BufferOne,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_matched(),
            "BDD: Pre-arrived signal with BufferOne policy must match immediately"
        );
    }

    #[test]
    fn given_pre_arrived_signal_with_buffer_many_when_wait_registered_then_match_succeeds() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("multi-signal").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::BufferMany,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_matched(),
            "BDD: Pre-arrived signal with BufferMany policy must match immediately"
        );
    }

    #[test]
    fn given_pre_arrived_epoch_local_signal_when_wait_registered_then_match_succeeds() {
        use vo_types::Epoch;

        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("epoch-gate").expect("valid key");

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            BufferPolicy::Reject,
            TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id, Epoch::ZERO);

        assert!(
            result.is_matched(),
            "BDD: Pre-arrived epoch-local signal must match when wait is registered"
        );
    }
}

// ===========================================================================
// BDD Scenario 3: Given multiple matching signals,
//   When deduplication applied, Then only first signal wakes.
//
// This tests SignalDedupeKey semantics: identical dedupe keys collapse to
// a single wake-up, while different command_ids produce distinct keys.
// ===========================================================================

mod dedup_only_first_signal_wakes {
    use super::*;

    #[test]
    fn given_identical_dedupe_keys_when_inserted_into_set_then_only_one_entry() {
        let lineage_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let command_id = IdempotencyKey::parse("cmd-001").expect("valid key");

        let dk_a = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), command_id.clone());
        let dk_b = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), command_id.clone());
        let dk_c = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), command_id.clone());

        let mut seen: HashSet<SignalDedupeKey> = HashSet::new();
        let first_wake = seen.insert(dk_a);
        let second_wake = seen.insert(dk_b);
        let third_wake = seen.insert(dk_c);

        assert!(first_wake, "BDD: First signal must be accepted (wake-up)");
        assert!(
            !second_wake,
            "BDD: Second identical signal must be deduplicated (no wake-up)"
        );
        assert!(
            !third_wake,
            "BDD: Third identical signal must be deduplicated (no wake-up)"
        );
        assert_eq!(seen.len(), 1, "BDD: Only one unique wake-up must exist");
    }

    #[test]
    fn given_different_command_ids_when_deduplicated_then_both_signals_wake() {
        let lineage_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let cmd_a = IdempotencyKey::parse("cmd-001").expect("valid key");
        let cmd_b = IdempotencyKey::parse("cmd-002").expect("valid key");

        let dk_a = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_a);
        let dk_b = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_b);

        let mut seen: HashSet<SignalDedupeKey> = HashSet::new();
        let first_wake = seen.insert(dk_a);
        let second_wake = seen.insert(dk_b);

        assert!(first_wake, "BDD: First signal must be accepted");
        assert!(
            second_wake,
            "BDD: Signal with different command_id is distinct, must wake"
        );
        assert_eq!(
            seen.len(),
            2,
            "BDD: Two distinct signals must produce two wake-ups"
        );
    }

    #[test]
    fn given_different_wait_keys_when_deduplicated_then_both_signals_wake() {
        let lineage_id = valid_instance_id();
        let key_a = WaitKey::parse("approval").expect("valid key");
        let key_b = WaitKey::parse("rejection").expect("valid key");
        let command_id = IdempotencyKey::parse("cmd-001").expect("valid key");

        let dk_a = SignalDedupeKey::new(lineage_id.clone(), key_a, command_id.clone());
        let dk_b = SignalDedupeKey::new(lineage_id.clone(), key_b, command_id);

        let mut seen: HashSet<SignalDedupeKey> = HashSet::new();
        let first_wake = seen.insert(dk_a);
        let second_wake = seen.insert(dk_b);

        assert!(first_wake, "BDD: Signal for wait_key A must wake");
        assert!(
            second_wake,
            "BDD: Signal for wait_key B must wake (different target)"
        );
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn given_different_lineage_ids_when_deduplicated_then_both_signals_wake() {
        let lineage_a = valid_instance_id();
        let lineage_b = other_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let command_id = IdempotencyKey::parse("cmd-001").expect("valid key");

        let dk_a = SignalDedupeKey::new(lineage_a, wait_key.clone(), command_id.clone());
        let dk_b = SignalDedupeKey::new(lineage_b, wait_key.clone(), command_id);

        let mut seen: HashSet<SignalDedupeKey> = HashSet::new();
        let first_wake = seen.insert(dk_a);
        let second_wake = seen.insert(dk_b);

        assert!(first_wake, "BDD: Signal for lineage A must wake");
        assert!(
            second_wake,
            "BDD: Signal for lineage B must wake (different lineage)"
        );
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn given_three_signals_two_duplicate_when_deduplicated_then_only_two_wake() {
        let lineage_id = valid_instance_id();
        let wait_key = WaitKey::parse("gate").expect("valid key");
        let cmd_unique = IdempotencyKey::parse("cmd-unique").expect("valid key");
        let cmd_dup = IdempotencyKey::parse("cmd-dup").expect("valid key");

        let dk_1 = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_unique);
        let dk_2 = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_dup.clone());
        let dk_3 = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_dup);

        let mut seen: HashSet<SignalDedupeKey> = HashSet::new();
        let wake_1 = seen.insert(dk_1);
        let wake_2 = seen.insert(dk_2);
        let wake_3 = seen.insert(dk_3);

        assert!(wake_1, "BDD: Signal 1 must wake");
        assert!(wake_2, "BDD: Signal 2 must wake (distinct command_id)");
        assert!(
            !wake_3,
            "BDD: Signal 3 must be deduped (same command_id as signal 2)"
        );
        assert_eq!(
            seen.len(),
            2,
            "BDD: Exactly 2 unique wake-ups from 3 signals"
        );
    }

    #[test]
    fn dedupe_key_equality_and_hash_are_consistent_for_set_membership() {
        let lineage_id = valid_instance_id();
        let wait_key = WaitKey::parse("consistent").expect("valid key");
        let command_id = IdempotencyKey::parse("cmd-consistent").expect("valid key");

        let dk = SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), command_id.clone());
        let dk_same = SignalDedupeKey::new(lineage_id, wait_key, command_id);

        assert_eq!(dk, dk_same, "BDD: Identical SignalDedupeKeys must be equal");

        let mut set = HashSet::new();
        set.insert(dk);
        assert!(
            set.contains(&dk_same),
            "BDD: Equal keys must be found in HashSet (hash consistency)"
        );
    }
}
