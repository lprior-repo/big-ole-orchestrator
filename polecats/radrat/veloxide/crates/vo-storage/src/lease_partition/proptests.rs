use proptest::prelude::*;

use super::{decode_lease_entry, encode_lease_entry, LeaseEntry, LeaseStoreError};

fn valid_entry_strategy() -> impl Strategy<Value = LeaseEntry> {
    (
        "[A-Za-z0-9_-]{1,64}",
        "[A-Za-z0-9_-]{1,64}",
        1u64..=u64::MAX,
        any::<u64>(),
    )
        .prop_filter_map(
            "LeaseEntry::new should accept generated values",
            |(instance_id, step_id, fence_token, expires_at)| {
                LeaseEntry::new(instance_id, step_id, fence_token, expires_at).ok()
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn lease_entry_new_preserves_valid_fields(
        instance_id in "[A-Za-z0-9_-]{1,64}",
        step_id in "[A-Za-z0-9_-]{1,64}",
        fence_token in 1u64..=u64::MAX,
        expires_at in any::<u64>(),
    ) {
        let result = LeaseEntry::new(instance_id.clone(), step_id.clone(), fence_token, expires_at);

        prop_assert_eq!(
            result,
            Ok(LeaseEntry {
                instance_id,
                step_id,
                fence_token,
                expires_at,
            })
        );
    }

    #[test]
    fn lease_entry_new_rejects_invalid_argument_when_any_required_field_missing(
        instance_id in prop_oneof![Just(String::new()), "[A-Za-z0-9_-]{1,64}".prop_map(std::convert::identity)],
        step_id in prop_oneof![Just(String::new()), "[A-Za-z0-9_-]{1,64}".prop_map(std::convert::identity)],
        fence_token in any::<u64>(),
        expires_at in any::<u64>(),
    ) {
        let result = LeaseEntry::new(instance_id.clone(), step_id.clone(), fence_token, expires_at);
        let invalid = instance_id.is_empty() || step_id.is_empty() || fence_token == 0;

        if invalid {
            prop_assert_eq!(result, Err(LeaseStoreError::InvalidArgument));
        } else {
            prop_assert_eq!(
                result,
                Ok(LeaseEntry {
                    instance_id,
                    step_id,
                    fence_token,
                    expires_at,
                })
            );
        }
    }

    #[test]
    fn lease_entry_is_expired_is_monotonic(
        entry in valid_entry_strategy(),
        now_a in any::<u64>(),
        delta in any::<u64>(),
    ) {
        let now_b = now_a.saturating_add(delta);

        prop_assume!(now_a <= now_b);

        let expired_a = entry.is_expired(now_a);
        let expired_b = entry.is_expired(now_b);

        prop_assert!(!expired_a || expired_b);
    }

    #[test]
    fn encode_decode_lease_entry_round_trips(entry in valid_entry_strategy()) {
        let encoded = encode_lease_entry(&entry);
        let decoded = encoded.and_then(|bytes| decode_lease_entry(&bytes));

        prop_assert_eq!(decoded, Ok(entry));
    }

    #[test]
    fn decode_lease_entry_preserves_shape_valid_payloads(
        instance_id in ".{0,64}",
        step_id in ".{0,64}",
        fence_token in any::<u64>(),
        expires_at in any::<u64>(),
    ) {
        let payload = serde_json::json!({
            "instance_id": instance_id,
            "step_id": step_id,
            "fence_token": fence_token,
            "expires_at": expires_at,
        })
        .to_string();

        prop_assert_eq!(
            decode_lease_entry(payload.as_bytes()),
            Ok(LeaseEntry {
                instance_id,
                step_id,
                fence_token,
                expires_at,
            })
        );
    }
}
