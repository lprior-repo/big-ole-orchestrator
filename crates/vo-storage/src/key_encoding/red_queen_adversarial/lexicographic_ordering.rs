use vo_types::SequenceNumber;

use crate::key_encoding::{encode_event_key, encode_instance_index_key_for_status, encode_timer_key};

use super::{max_instance_id, mid_instance_id, min_instance_id};

// ========================================================================
// DIMENSION: lexicographic-ordering — event keys
// Contract: higher sequence numbers produce lexicographically larger keys
// ========================================================================

#[test]
fn red_queen_event_key_lexicographic_increments_monotone() {
    let id = min_instance_id();
    let mut prev_key: Vec<u8> = Vec::new();

    for seq in 1..=1000u64 {
        let sn = SequenceNumber::try_from(seq).unwrap();
        let key = encode_event_key(&id, sn);
        assert!(
            key > prev_key,
            "BUG: event key seq {seq} should be > previous key; prev={prev_key:?}, curr={key:?}"
        );
        prev_key = key;
    }
}

#[test]
fn red_queen_event_key_different_instances_lexicographic_order_by_instance_prefix() {
    let id1 = min_instance_id();
    let id2 = mid_instance_id();
    let id3 = max_instance_id();
    let seq = SequenceNumber::try_from(1u64).unwrap();

    let key1 = encode_event_key(&id1, seq);
    let key2 = encode_event_key(&id2, seq);
    let key3 = encode_event_key(&id3, seq);

    assert!(
        key1 < key2,
        "BUG: min_instance_id key should be < mid_instance_id key"
    );
    assert!(
        key2 < key3,
        "BUG: mid_instance_id key should be < max_instance_id key"
    );
    assert!(
        key1 < key3,
        "BUG: min_instance_id key should be < max_instance_id key"
    );
}

#[test]
fn red_queen_event_key_instance_matters_more_than_sequence() {
    let id_small = min_instance_id();
    let id_large = max_instance_id();
    let seq_small = SequenceNumber::try_from(1u64).unwrap();
    let seq_large = SequenceNumber::try_from(u64::MAX).unwrap();

    let key_small_instance_large_seq = encode_event_key(&id_small, seq_large);
    let key_large_instance_small_seq = encode_event_key(&id_large, seq_small);

    // Instance ID is the first 16 bytes, so it dominates sequence number comparison
    assert!(
        key_small_instance_large_seq < key_large_instance_small_seq,
        "BUG: instance_id prefix should dominate sequence_number in ordering"
    );
}

// ========================================================================
// DIMENSION: lexicographic-ordering — timer keys
// Contract: earlier timestamps produce lexicographically smaller keys
// ========================================================================

#[test]
fn red_queen_timer_key_lexicographic_ordering_is_chronological() {
    let id = min_instance_id();
    let mut prev_key: Vec<u8> = Vec::new();

    for ts in 0..1000u64 {
        let key = encode_timer_key(ts, &id);
        assert!(
            key > prev_key,
            "BUG: timer key ts {ts} should be > previous key (later time = larger)"
        );
        prev_key = key;
    }
}

#[test]
fn red_queen_timer_key_different_instances_at_same_time_differ_only_in_last_16_bytes() {
    let id1 = min_instance_id();
    let id2 = max_instance_id();
    let ts = 1234567890u64;

    let key1 = encode_timer_key(ts, &id1);
    let key2 = encode_timer_key(ts, &id2);

    // Format: [timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]
    // First 10 bytes should be identical (8 timestamp + 2 length prefix)
    assert_eq!(
        &key1[0..10],
        &key2[0..10],
        "BUG: timestamp and length prefix bytes should be identical"
    );
    // Last 16 bytes should differ (instance_id)
    assert_ne!(
        &key1[10..26],
        &key2[10..26],
        "BUG: instance_id bytes should differ"
    );
    assert!(
        key1 < key2,
        "BUG: min_instance timer key should sort before max_instance"
    );
}

// ========================================================================
// DIMENSION: lexicographic-ordering — instance index keys
// Contract: status byte dominates, then created_at, then instance_id
// ========================================================================

#[test]
fn red_queen_instance_index_key_status_byte_is_primary_sort() {
    let id = min_instance_id();
    let created_at = 1000u64;

    let key_status_0 = encode_instance_index_key_for_status(0, created_at, &id);
    let key_status_1 = encode_instance_index_key_for_status(1, created_at, &id);
    let key_status_127 = encode_instance_index_key_for_status(127, created_at, &id);
    let key_status_128 = encode_instance_index_key_for_status(128, created_at, &id);
    let key_status_255 = encode_instance_index_key_for_status(255, created_at, &id);

    assert!(key_status_0 < key_status_1, "BUG: status 0 < 1");
    assert!(key_status_1 < key_status_127, "BUG: status 1 < 127");
    assert!(key_status_127 < key_status_128, "BUG: status 127 < 128");
    assert!(key_status_128 < key_status_255, "BUG: status 128 < 255");
}

#[test]
fn red_queen_instance_index_key_created_at_is_secondary_sort() {
    let id = min_instance_id();

    let key_earlier = encode_instance_index_key_for_status(0, 1000, &id);
    let key_later = encode_instance_index_key_for_status(0, 2000, &id);

    assert!(
        key_earlier < key_later,
        "BUG: earlier created_at should sort before later"
    );
}
