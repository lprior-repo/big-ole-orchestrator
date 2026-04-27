//! Comprehensive property tests for ALL production partition key encoders (ADR-020).
//!
//! BDD Scenario:
//!   Given random valid key components are generated for every partition encoder
//!   When keys are encoded and decoded or compared
//!   Then round-trip/collision/order invariants hold
//!
//! Run with: cargo test -p vo-storage proptest_all_partition_key_encoders

#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use vo_types::{EffectKind, InstanceId, SequenceNumber, StepId};

use vo_storage::key_encoding::{
    decode_dedupe_key, decode_effect_key, decode_event_key, decode_lease_key, decode_timer_key,
    encode_dedupe_key, encode_effect_key, encode_event_key, encode_instance_index_key_for_status,
    encode_lease_key, encode_timer_key, get_dedupe_key_prefix, get_event_key_prefix,
    get_lease_key_prefix_for_instance, get_timer_key_prefix_for_time,
};
use vo_storage::receipts::{
    decode_receipt, decode_receipt_key, encode_receipt, encode_receipt_key,
};
use vo_storage::snapshots::{decode_snapshot_key, encode_snapshot_key};

fn arb_instance_id() -> impl Strategy<Value = InstanceId> {
    proptest::array::uniform16(proptest::num::u8::ANY).prop_map(InstanceId::from_bytes)
}

fn arb_sequence_number() -> impl Strategy<Value = SequenceNumber> {
    any::<u64>().prop_map(|n| SequenceNumber::try_from(n).unwrap())
}

fn arb_step_id() -> impl Strategy<Value = StepId> {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_"
        .chars()
        .collect();
    proptest::collection::vec(proptest::sample::subsequence(chars, 1..50), 1..=1).prop_map(
        |chars| {
            let s: String = chars.into_iter().flatten().collect();
            StepId::parse(&s).unwrap()
        },
    )
}

fn arb_effect_kind() -> impl Strategy<Value = EffectKind> {
    prop_oneof![
        Just(EffectKind::HttpCall),
        Just(EffectKind::SqlQuery),
        Just(EffectKind::BlobWrite),
    ]
}

fn arb_nonempty_string(max_len: usize) -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_:.-]{1,}".prop_map(move |s| s.chars().take(max_len).collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Comprehensive test: ALL partition key encoders satisfy round-trip,
    /// collision-freedom, and ordering invariants for random valid inputs.
    #[test]
    fn proptest_all_partition_key_encoders(
        id in arb_instance_id(),
        seq in arb_sequence_number(),
        step in arb_step_id(),
        fire_at_ms in any::<u64>(),
        dedupe_key in "[a-zA-Z0-9_-]{0,200}",
        effect_id in arb_nonempty_string(200),
        instance_id_str in arb_nonempty_string(100),
        kind in arb_effect_kind(),
        committed_at_ms in any::<u64>(),
        connector_result in "[a-zA-Z0-9 ]{0,200}",
        status_byte in 1u8..=6u8,
        created_at in any::<u64>(),
    ) {
        // ---- EVENT KEY (ADR-020 §events) ----
        let event_key = encode_event_key(&id, seq);
        let (ev_id, ev_seq) = decode_event_key(&event_key).unwrap();
        prop_assert_eq!(ev_id, id.clone(), "EV roundtrip: instance ID mismatch");
        prop_assert_eq!(ev_seq, seq, "EV roundtrip: sequence mismatch");
        prop_assert_eq!(event_key.len(), 24, "EV: must be 24 bytes");
        let ev_prefix = get_event_key_prefix(&id);
        prop_assert!(event_key.starts_with(&ev_prefix), "EV: prefix scan correctness");

        // ---- TIMER KEY (ADR-020 §timers) ----
        let timer_key = encode_timer_key(fire_at_ms, &id);
        let (tm_ts, tm_id) = decode_timer_key(&timer_key).unwrap();
        prop_assert_eq!(tm_ts, fire_at_ms, "TM roundtrip: timestamp mismatch");
        prop_assert_eq!(tm_id, id.clone(), "TM roundtrip: instance ID mismatch");
        prop_assert_eq!(timer_key.len(), 24, "TM: must be 24 bytes");
        let tm_prefix = get_timer_key_prefix_for_time(fire_at_ms);
        prop_assert!(timer_key.starts_with(&tm_prefix), "TM: prefix scan correctness");

        // ---- LEASE KEY (ADR-020 §leases) ----
        let lease_key = encode_lease_key(&id, &step);
        let (ls_id, ls_step) = decode_lease_key(&lease_key).unwrap();
        prop_assert_eq!(ls_id, id.clone(), "LS roundtrip: instance ID mismatch");
        prop_assert_eq!(ls_step, step, "LS roundtrip: step ID mismatch");
        let ls_prefix = get_lease_key_prefix_for_instance(&id);
        prop_assert!(lease_key.starts_with(&ls_prefix), "LS: prefix scan correctness");

        // ---- DEDUPE KEY (ADR-020 §dedupe) ----
        let dd_key = encode_dedupe_key(&dedupe_key);
        let dd_decoded = decode_dedupe_key(&dd_key).unwrap();
        prop_assert_eq!(dd_decoded, dedupe_key.clone(), "DD roundtrip: key mismatch");
        let dd_prefix = get_dedupe_key_prefix(&dedupe_key);
        prop_assert_eq!(dd_prefix, dd_key.clone(), "DD: prefix equals full key for short keys");

        // ---- EFFECT KEY (ADR-020 §effects) ----
        let effect_key = encode_effect_key(&id, seq);
        let (ef_id, ef_seq) = decode_effect_key(&effect_key).unwrap();
        prop_assert_eq!(ef_id, id.clone(), "EF roundtrip: instance ID mismatch");
        prop_assert_eq!(ef_seq, seq, "EF roundtrip: sequence mismatch");
        prop_assert_eq!(effect_key.len(), 25, "EF: must be 25 bytes");
        prop_assert!(event_key < effect_key, "EF: event key must sort before effect key");
        prop_assert_eq!(&effect_key[0..24], &event_key[0..24], "EF: shares 24-byte prefix with event key");

        // ---- INSTANCE INDEX KEY (ADR-020 §instances) ----
        let ii_key = encode_instance_index_key_for_status(status_byte, created_at, &id);
        prop_assert_eq!(ii_key.len(), 25, "II: must be 25 bytes (1+8+16)");

        // ---- SNAPSHOT KEY ----
        let snap_key = encode_snapshot_key(&id, seq.as_u64()).unwrap();
        let (snap_id, snap_seq) = decode_snapshot_key(&snap_key).unwrap();
        prop_assert_eq!(snap_id, id.clone(), "SNAP roundtrip: instance ID mismatch");
        prop_assert_eq!(snap_seq, seq.as_u64(), "SNAP roundtrip: sequence mismatch");
        prop_assert_eq!(snap_key.len(), 24, "SNAP: must be 24 bytes");

        // ---- RECEIPT KEY (ADR-041) ----
        let rc_key = encode_receipt_key(&effect_id);
        let rc_decoded = decode_receipt_key(&rc_key).unwrap();
        prop_assert_eq!(rc_decoded, effect_id.clone(), "RC roundtrip: effect ID mismatch");
        prop_assert!(!rc_key.is_empty(), "RC: key must be non-empty");

        // ---- RECEIPT VALUE (ADR-041) ----
        let receipt = vo_storage::receipts::ExecutionReceipt::new(
            effect_id.clone(),
            instance_id_str,
            kind,
            committed_at_ms,
            connector_result,
        )
        .unwrap();
        let rc_value = encode_receipt(&receipt).unwrap();
        let rc_value_decoded = decode_receipt(&rc_value).unwrap();
        prop_assert_eq!(rc_value_decoded, receipt, "RC value roundtrip mismatch");

        // ---- CROSS-KEY COLLISION CHECK ----
        prop_assert_ne!(event_key.clone(), effect_key, "CK: event != effect");
        prop_assert_ne!(event_key.clone(), timer_key, "CK: event != timer");
        prop_assert_ne!(event_key.clone(), dd_key, "CK: event != dedupe");
        prop_assert_ne!(lease_key, event_key, "CK: lease != event");
    }
}
