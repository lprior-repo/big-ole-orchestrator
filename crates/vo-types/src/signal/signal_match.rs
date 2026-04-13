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

    #[test]
    fn signal_match_epoch_local_signal_matches_when_epoch_is_zero() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let epoch = Epoch::ZERO;

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            epoch,
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
        assert!(
            result.is_matched(),
            "Epoch-local signal should match when signal epoch is ZERO (wait_epoch_for_instance returns ZERO)"
        );
    }

    #[test]
    fn signal_match_epoch_local_returns_epoch_mismatch_when_epochs_differ() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let signal_epoch = Epoch::new(5);
        let wait_epoch = Epoch::ZERO;

        let signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            signal_epoch,
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
        assert!(
            result.is_mismatch(),
            "Epoch-local signal should mismatch when epochs differ"
        );
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

    #[test]
    fn signal_match_epoch_local_returns_epoch_not_specified_when_epoch_missing() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");

        let mut signal = SignalAddress::epoch_local(
            lineage_id.clone(),
            Epoch::ZERO,
            instance_id.clone(),
            wait_key.clone(),
        );

        let signal_without_epoch =
            SignalAddress::lineage_wide(lineage_id.clone(), instance_id.clone(), wait_key.clone());

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        let result = signal_match(&signal_without_epoch, &wait, &lineage_id);
        assert!(
            result.is_matched(),
            "Lineage-wide signal should match (epoch not checked)"
        );
    }

    #[test]
    fn signal_match_all_dimensions_must_align_for_matched() {
        let lineage_id = valid_instance_id();
        let instance_id = valid_instance_id();
        let wait_key = WaitKey::parse("approval").expect("valid key");
        let other_instance_id =
            InstanceId::parse("01JAR3K2N0XG8F5VZE9H7QW4Y7").expect("valid ULID");

        let cases = [
            (
                SignalAddress::lineage_wide(
                    other_instance_id.clone(),
                    instance_id.clone(),
                    wait_key.clone(),
                ),
                "lineage mismatch",
            ),
            (
                SignalAddress::lineage_wide(
                    lineage_id.clone(),
                    other_instance_id,
                    wait_key.clone(),
                ),
                "instance mismatch",
            ),
            (
                SignalAddress::lineage_wide(
                    lineage_id.clone(),
                    instance_id.clone(),
                    WaitKey::parse("other-key").expect("valid"),
                ),
                "wait_key mismatch",
            ),
        ];

        let wait = WaitRecord::new(
            instance_id,
            wait_key,
            crate::BufferPolicy::Reject,
            crate::TimestampMs::now(),
        )
        .expect("valid wait record");

        for (signal, description) in cases {
            let result = signal_match(&signal, &wait, &lineage_id);
            assert!(
                result.is_mismatch(),
                "signal should not match due to {description}"
            );
        }
    }

    #[test]
    fn signal_match_lineage_wide_ignores_epoch() {
        let lineage_id = valid_instance_id();
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

        let result = signal_match(&signal, &wait, &lineage_id);
        assert!(
            result.is_matched(),
            "Lineage-wide signal should match regardless of epoch (epoch not checked)"
        );
    }
}
