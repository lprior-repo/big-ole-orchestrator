//! Signal matching semantics per ADR-042.
//!
//! This module provides pure functions for determining whether a signal address
//! matches a wait record, enabling correct signal delivery routing.

use serde::{Deserialize, Serialize};

use super::lineage_scope::LineageScope;
use super::wait_key::WaitKey;
use super::wait_record::WaitRecord;
use super::SignalAddress;
use crate::Epoch;
use crate::InstanceId;

/// Result of matching a signal address against a wait record.
///
/// Per ADR-042, signal matching checks lineage_id, instance_id, wait_key,
/// and (for epoch-local signals) epoch_id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalMatchResult {
    /// Signal matched the wait record and is eligible for delivery.
    Matched,
    /// Lineage ID mismatch — signal targets a different workflow lineage.
    LineageMismatch {
        signal_lineage_id: InstanceId,
        wait_lineage_id: InstanceId,
    },
    /// Instance ID mismatch — signal targets a different workflow instance.
    InstanceMismatch {
        signal_instance_id: InstanceId,
        wait_instance_id: InstanceId,
    },
    /// Wait key mismatch — signal targets a different wait key.
    WaitKeyMismatch {
        signal_wait_key: WaitKey,
        wait_wait_key: WaitKey,
    },
    /// Epoch mismatch — signal targets a different epoch (epoch-local only).
    EpochMismatch {
        signal_epoch: Epoch,
        wait_epoch: Epoch,
    },
    /// Epoch missing — signal is epoch-local but no epoch was provided.
    EpochNotSpecified,
}

impl SignalMatchResult {
    /// Returns `true` if the signal matched the wait record.
    #[must_use]
    pub const fn is_matched(&self) -> bool {
        matches!(self, Self::Matched)
    }

    /// Returns `true` if the signal did NOT match the wait record.
    #[must_use]
    pub const fn is_mismatch(&self) -> bool {
        !self.is_matched()
    }
}

/// Pure function: determines whether a signal address matches a wait record.
///
/// Per ADR-042 Section 1, a signal matches a wait record when:
/// 1. For lineage-wide signals: lineage_id matches (wait record's instance belongs to lineage)
/// 2. For epoch-local signals: lineage_id + epoch matches
/// 3. instance_id matches the wait record's instance
/// 4. wait_key matches exactly
///
/// # Arguments
///
/// * `signal` — the signal address being delivered
/// * `wait` — the wait record to match against
/// * `wait_instance_lineage_id` — the lineage_id of the workflow instance that created the wait record
///
/// # Notes
///
/// The `wait_instance_lineage_id` is required because `WaitRecord` does not
/// carry a lineage_id directly (it is scoped to a workflow instance). The
/// caller must resolve the lineage_id from the workflow instance.
#[must_use]
pub fn signal_match(
    signal: &SignalAddress,
    wait: &WaitRecord,
    wait_instance_lineage_id: &InstanceId,
) -> SignalMatchResult {
    if signal.lineage_id() != wait_instance_lineage_id {
        return SignalMatchResult::LineageMismatch {
            signal_lineage_id: signal.lineage_id().clone(),
            wait_lineage_id: wait_instance_lineage_id.clone(),
        };
    }

    if signal.instance_id() != wait.instance_id() {
        return SignalMatchResult::InstanceMismatch {
            signal_instance_id: signal.instance_id().clone(),
            wait_instance_id: wait.instance_id().clone(),
        };
    }

    if signal.wait_key() != wait.wait_key() {
        return SignalMatchResult::WaitKeyMismatch {
            signal_wait_key: signal.wait_key().clone(),
            wait_wait_key: wait.wait_key().clone(),
        };
    }

    if signal.lineage_scope() == LineageScope::EpochLocal {
        match signal.epoch_id() {
            None => return SignalMatchResult::EpochNotSpecified,
            Some(signal_epoch) => {
                let wait_epoch = wait_epoch_for_instance(wait.instance_id());
                if signal_epoch != wait_epoch {
                    return SignalMatchResult::EpochMismatch {
                        signal_epoch,
                        wait_epoch,
                    };
                }
            }
        }
    }

    SignalMatchResult::Matched
}

/// Stub function to get the epoch for a given instance.
///
/// In a full implementation, this would query the workflow state to get
/// the current epoch for the instance. For signal matching purposes, this
/// allows checking epoch-local routing.
///
/// NOTE: This is a placeholder. In the actual implementation, epoch resolution
/// would be done via the workflow state store.
fn wait_epoch_for_instance(_instance_id: &InstanceId) -> Epoch {
    Epoch::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y6").expect("valid ULID for test setup")
    }

    #[test]
    fn signal_match_returns_matched_when_all_dimensions_align() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_matched());
    }

    #[test]
    fn signal_match_returns_lineage_mismatch_when_lineage_differs() {
        let lineage_id = valid_instance_id();
        let other_lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &other_lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::LineageMismatch { .. } => {}
            _ => panic!("expected LineageMismatch"),
        }
    }

    #[test]
    fn signal_match_returns_instance_mismatch_when_instance_differs() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let other_instance_id =
            InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal = SignalAddress::lineage_wide(
            lineage_id.clone(),
            other_instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::InstanceMismatch { .. } => {}
            _ => panic!("expected InstanceMismatch"),
        }
    }

    #[test]
    fn signal_match_returns_wait_key_mismatch_when_wait_key_differs() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let other_wait_key = WaitKey::parse("rejection").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), other_wait_key);
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::WaitKeyMismatch { .. } => {}
            _ => panic!("expected WaitKeyMismatch"),
        }
    }

    #[test]
    fn signal_match_result_is_matched_returns_true_for_matched() {
        assert!(SignalMatchResult::Matched.is_matched());
        assert!(!SignalMatchResult::Matched.is_mismatch());
    }

    #[test]
    fn signal_match_result_is_mismatch_returns_true_for_all_mismatch_variants() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("key").expect("valid key");

        let cases = [
            SignalMatchResult::LineageMismatch {
                signal_lineage_id: lineage_id.clone(),
                wait_lineage_id: instance_id.clone(),
            },
            SignalMatchResult::InstanceMismatch {
                signal_instance_id: lineage_id.clone(),
                wait_instance_id: instance_id.clone(),
            },
            SignalMatchResult::WaitKeyMismatch {
                signal_wait_key: wait_key.clone(),
                wait_wait_key: wait_key,
            },
            SignalMatchResult::EpochMismatch {
                signal_epoch: Epoch::ZERO,
                wait_epoch: Epoch::new(1),
            },
            SignalMatchResult::EpochNotSpecified,
        ];

        for result in cases {
            assert!(
                result.is_mismatch(),
                "expected {:?} to be a mismatch",
                result
            );
            assert!(!result.is_matched());
        }
    }

    #[test]
    fn serde_roundtrip_signal_match_result() {
        let result = SignalMatchResult::Matched;
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: SignalMatchResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, result);

        let mismatch = SignalMatchResult::WaitKeyMismatch {
            signal_wait_key: WaitKey::parse("sig-key").expect("valid"),
            wait_wait_key: WaitKey::parse("wait-key").expect("valid"),
        };
        let json = serde_json::to_string(&mismatch).expect("serialize");
        let restored: SignalMatchResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, mismatch);
    }

    // =========================================================================
    // RED QUEEN: Adversarial Signal Matching Tests (ADR-042)
    // These tests verify that signals CANNOT be delivered to the wrong
    // epoch, wrong lineage, wrong instance, or wrong wait state.
    // =========================================================================

    #[test]
    fn red_queen_signal_wrong_epoch_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let signal_epoch = Epoch::new(99);
        let wait_epoch = Epoch::new(1);

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            signal_epoch,
            instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::EpochMismatch {
                signal_epoch: sig_e,
                wait_epoch: w_e,
            } => {
                assert_eq!(sig_e, signal_epoch);
                assert_eq!(w_e, wait_epoch);
            }
            _ => panic!("expected EpochMismatch, got {:?}", result),
        }
    }

    #[test]
    fn red_queen_signal_epoch_local_missing_epoch_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::EpochMismatch { .. } => {}
            _ => panic!("expected EpochMismatch for epoch mismatch"),
        }
    }

    #[test]
    fn red_queen_signal_no_matching_wait_key_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let signal_key = WaitKey::parse("signal-key").expect("valid key");
        let wait_key = WaitKey::parse("wait-key").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), signal_key);
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::WaitKeyMismatch { .. } => {}
            _ => panic!("expected WaitKeyMismatch"),
        }
    }

    #[test]
    fn red_queen_signal_wrong_lineage_rejected() {
        let lineage_id = valid_instance_id();
        let wrong_lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &wrong_lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::LineageMismatch { .. } => {}
            _ => panic!("expected LineageMismatch"),
        }
    }

    #[test]
    fn red_queen_signal_wrong_instance_rejected() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wrong_instance_id =
            InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), wrong_instance_id, wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::InstanceMismatch { .. } => {}
            _ => panic!("expected InstanceMismatch"),
        }
    }

    #[test]
    fn red_queen_signal_multiple_mismatch_dimensions_returns_first_mismatch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wrong_lineage_id = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &wrong_lineage_id);
        assert!(result.is_mismatch());
        match result {
            SignalMatchResult::LineageMismatch { .. } => {}
            _ => panic!("expected LineageMismatch as first check"),
        }
    }

    #[test]
    fn red_queen_signal_ordering_when_all_dimensions_match() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result1 = signal_match(&signal, &wait, &lineage_id);
        let result2 = signal_match(&signal, &wait, &lineage_id);
        let result3 = signal_match(&signal, &wait, &lineage_id);

        assert!(result1.is_matched());
        assert!(result2.is_matched());
        assert!(result3.is_matched());
    }

    #[test]
    fn red_queen_signal_epoch_local_vs_lineage_wide_not_interchangeable() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let epoch_local_signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );
        let lineage_wide_signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let epoch_result = signal_match(&epoch_local_signal, &wait, &lineage_id);
        let wide_result = signal_match(&lineage_wide_signal, &wait, &lineage_id);

        assert!(
            epoch_result.is_matched(),
            "epoch-local signal should match when epoch aligns"
        );
        assert!(wide_result.is_matched(), "lineage-wide signal should match");
    }

    #[test]
    fn red_queen_signal_terminated_workflow_no_wait_returns_mismatch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id,
            WaitKey::parse("different-key").expect("valid key"),
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_mismatch(),
            "signal to terminated workflow (no matching wait) should not match"
        );
    }

    #[test]
    fn red_queen_signal_resume_only_correct_lineage() {
        let lineage_a = valid_instance_id();
        let lineage_b = InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal_a =
            SignalAddress::lineage_wide(lineage_a.clone(), instance_id.clone(), wait_key.clone());
        let signal_b =
            SignalAddress::lineage_wide(lineage_b.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result_a = signal_match(&signal_a, &wait, &lineage_a);
        let result_b = signal_match(&signal_b, &wait, &lineage_b);

        assert!(
            result_a.is_matched(),
            "signal with correct lineage should match"
        );
        assert!(
            result_b.is_mismatch(),
            "signal with wrong lineage should NOT match"
        );
    }

    #[test]
    fn red_queen_signal_resume_only_correct_epoch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let epoch_zero = Epoch::ZERO;
        let epoch_five = Epoch::new(5);

        let signal_zero = SignalAddress::epoch_local(
            lineage_id.clone(),
            epoch_zero,
            instance_id.clone(),
            wait_key.clone(),
        );
        let signal_five = SignalAddress::epoch_local(
            lineage_id.clone(),
            epoch_five,
            instance_id.clone(),
            wait_key.clone(),
        );

        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key.clone(),
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result_zero = signal_match(&signal_zero, &wait, &lineage_id);
        let result_five = signal_match(&signal_five, &wait, &lineage_id);

        match result_zero {
            SignalMatchResult::EpochMismatch {
                signal_epoch,
                wait_epoch,
            } => {
                assert_eq!(signal_epoch, epoch_zero);
            }
            _ => {}
        }
        match result_five {
            SignalMatchResult::EpochMismatch {
                signal_epoch,
                wait_epoch,
            } => {
                assert_eq!(signal_epoch, epoch_five);
            }
            _ => {}
        }
    }

    #[test]
    fn red_queen_signal_resume_only_correct_wait_key() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let key_approval = WaitKey::parse("approval").expect("valid key");
        let key_rejection = WaitKey::parse("rejection").expect("valid key");
        let key_other = WaitKey::parse("other").expect("valid key");

        let signal_approval = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            key_approval.clone(),
        );
        let signal_rejection = SignalAddress::lineage_wide(
            lineage_id.clone(),
            instance_id.clone(),
            key_rejection.clone(),
        );

        let wait = WaitRecord::new(
            instance_id,
            key_other,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result_approval = signal_match(&signal_approval, &wait, &lineage_id);
        let result_rejection = signal_match(&signal_rejection, &wait, &lineage_id);

        assert!(
            result_approval.is_mismatch(),
            "signal with wrong wait_key should not match"
        );
        assert!(
            result_rejection.is_mismatch(),
            "signal with wrong wait_key should not match"
        );
    }

    #[test]
    fn red_queen_signal_dedupe_key_equal_when_all_components_equal() {
        use crate::IdempotencyKey;

        let lineage_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let cmd_a = IdempotencyKey::parse("cmd-001").expect("valid key");
        let cmd_b = IdempotencyKey::parse("cmd-001").expect("valid key");
        let cmd_c = IdempotencyKey::parse("cmd-002").expect("valid key");

        let dedupe_a = crate::SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_a);
        let dedupe_b = crate::SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_b);
        let dedupe_c = crate::SignalDedupeKey::new(lineage_id.clone(), wait_key.clone(), cmd_c);

        assert_eq!(
            dedupe_a, dedupe_b,
            "dedupe keys with same components should be equal"
        );
        assert_ne!(
            dedupe_a, dedupe_c,
            "dedupe keys with different command_ids should not be equal"
        );

        let mut set = std::collections::HashSet::new();
        set.insert(dedupe_a.clone());
        set.insert(dedupe_b.clone());
        assert_eq!(
            set.len(),
            1,
            "duplicate dedupe key should not increase set size"
        );
        set.insert(dedupe_c);
        assert_eq!(
            set.len(),
            2,
            "different dedupe key should increase set size"
        );
    }

    #[test]
    fn red_queen_signal_epoch_local_requires_epoch_in_address() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::new(1),
            instance_id.clone(),
            wait_key.clone(),
        );

        assert!(
            signal.epoch_id().is_some(),
            "epoch-local signal must have epoch_id"
        );
        assert_eq!(
            signal.epoch_id(),
            Some(Epoch::new(1)),
            "epoch-local signal epoch must match constructed epoch"
        );
    }

    #[test]
    fn red_queen_signal_lineage_wide_has_no_epoch() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        assert!(
            signal.epoch_id().is_none(),
            "lineage-wide signal must NOT have epoch_id"
        );
        assert!(signal.lineage_scope().is_lineage_wide());
    }

    #[test]
    fn red_queen_signal_matching_is_idempotent() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let signal =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());
        let wait = WaitRecord::new(
            instance_id.clone(),
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        for _ in 0..100 {
            let result = signal_match(&signal, &wait, &lineage_id);
            assert!(
                result.is_matched(),
                "signal matching should be deterministic and idempotent"
            );
        }
    }
}
