#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for wait, batch, and switch domains in vo-actor.
//!
//! Attack vectors:
//! - WAIT: SignalBuffer edge cases, SignalBufferConfig invariant bypass,
//!   peek_all/pop_buffered interleaving, empty-payload signals
//! - BATCH: calculate_batch_size overflow/underflow, validate_timer_record
//!   boundary corruption, AtomicTransition batch edge cases
//! - SWITCH: BufferOne→BufferMany→Reject policy transitions,
//!   Single→Many promotion under overflow, cross-key policy isolation,
//!   rapid policy cycling

use vo_actor::reanimator::{calculate_batch_size, validate_timer_record, TimerRecord};
use vo_actor::signal_buffer::{
    apply_policy, BufferResult, BufferedSignal, SignalBuffer, SignalBufferConfig,
};
use vo_actor::{SignalPayload, WaitKey};
use vo_types::{BufferPolicy, InstanceId, TimestampMs};

fn make_instance_id(seed: u64) -> InstanceId {
    let ts = 1_700_000_000_000 + seed;
    let rand_bytes = [
        (seed >> 56) as u8,
        (seed >> 48) as u8,
        (seed >> 40) as u8,
        (seed >> 32) as u8,
        (seed >> 24) as u8,
        (seed >> 16) as u8,
        (seed >> 8) as u8,
        seed as u8,
        0,
        0,
    ];
    let ulid = ulid::Ulid::from_parts(
        ts,
        u128::from_be_bytes([
            0,
            0,
            0,
            0,
            0,
            0,
            rand_bytes[0],
            rand_bytes[1],
            rand_bytes[2],
            rand_bytes[3],
            rand_bytes[4],
            rand_bytes[5],
            rand_bytes[6],
            rand_bytes[7],
            rand_bytes[8],
            rand_bytes[9],
        ]),
    );
    InstanceId::parse(&ulid.to_string()).expect("generated ULID should be valid InstanceId")
}

fn make_signal(id: &str) -> BufferedSignal {
    BufferedSignal::new(id.to_string(), SignalPayload::empty(), TimestampMs::now())
}

fn make_signal_with_payload(id: &str, payload: Vec<u8>) -> BufferedSignal {
    BufferedSignal::new(
        id.to_string(),
        SignalPayload::from_bytes(payload).expect("payload within limit"),
        TimestampMs::now(),
    )
}

fn make_wait_key(s: &str) -> WaitKey {
    WaitKey::parse(s).expect("test wait key should be valid")
}

fn make_timer_record(fire_at_ms: u64, scheduled_at_ms: u64, instance_id: &str) -> TimerRecord {
    TimerRecord {
        timer_id: None,
        instance_id: InstanceId::parse(instance_id).expect("valid instance id"),
        fire_at_ms: TimestampMs::try_from(fire_at_ms).expect("valid timestamp"),
        scheduled_at_ms: TimestampMs::try_from(scheduled_at_ms).expect("valid timestamp"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 1: WAIT — SignalBufferConfig max_buffered_per_key=0 clamped to 1
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Config with max=0 should be clamped to 1 (documented invariant).
/// BufferMany with max=0 would mean "never buffer", but the constructor
/// enforces max >= 1.
#[test]
fn attack_wait_config_max_zero_clamped_to_one() {
    let config = SignalBufferConfig::new(0);
    assert_eq!(
        config.max_buffered_per_key, 1,
        "BUG: max_buffered_per_key=0 should be clamped to 1"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 2: WAIT — peek_all then pop interleaving consistency
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: peek_all returns all buffered signals. Pop one, peek again,
/// verify the popped signal is gone and order is preserved.
#[test]
fn attack_wait_peek_pop_interleaving_consistency() {
    let config = SignalBufferConfig::new(10);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(100);
    let wk = make_wait_key("interleave");

    for i in 0..5 {
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i}")),
            BufferPolicy::BufferMany,
        );
    }

    let all = buf.peek_all(&id, &wk);
    assert_eq!(all.len(), 5);
    assert_eq!(all[0].signal_id, "sig-0");
    assert_eq!(all[4].signal_id, "sig-4");

    let popped = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(popped.signal_id, "sig-0", "pop should be FIFO");

    let remaining = buf.peek_all(&id, &wk);
    assert_eq!(remaining.len(), 4);
    assert_eq!(
        remaining[0].signal_id, "sig-1",
        "remaining should start from sig-1"
    );

    let popped2 = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(popped2.signal_id, "sig-1");

    assert_eq!(buf.buffered_count(&id, &wk), 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 3: WAIT — Pop all signals one by one, verify FIFO order
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Buffer 50 signals, pop them all, verify strict FIFO ordering.
#[test]
fn attack_wait_fifo_order_50_signals() {
    let config = SignalBufferConfig::new(50);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(101);
    let wk = make_wait_key("fifo-50");

    for i in 0..50 {
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i:03}")),
            BufferPolicy::BufferMany,
        );
    }

    for i in 0..50 {
        let popped = buf
            .pop_buffered(&id, &wk)
            .expect("should have signal to pop");
        assert_eq!(
            popped.signal_id,
            format!("sig-{i:03}"),
            "BUG: FIFO order violated at position {i}"
        );
    }

    assert!(
        buf.pop_buffered(&id, &wk).is_none(),
        "buffer should be empty after 50 pops"
    );
    assert_eq!(buf.buffered_count(&id, &wk), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 4: WAIT — Different wait keys on same instance are isolated
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Same instance, different wait keys — each should have independent buffers.
#[test]
fn attack_wait_key_isolation_same_instance() {
    let config = SignalBufferConfig::new(3);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(102);
    let wk_a = make_wait_key("approval");
    let wk_b = make_wait_key("timer");

    for i in 0..3 {
        buf.buffer_signal(
            id.clone(),
            wk_a.clone(),
            make_signal(&format!("a-{i}")),
            BufferPolicy::BufferMany,
        );
        buf.buffer_signal(
            id.clone(),
            wk_b.clone(),
            make_signal(&format!("b-{i}")),
            BufferPolicy::BufferMany,
        );
    }

    assert_eq!(buf.buffered_count(&id, &wk_a), 3);
    assert_eq!(buf.buffered_count(&id, &wk_b), 3);

    let overflow_a = buf.buffer_signal(
        id.clone(),
        wk_a.clone(),
        make_signal("a-overflow"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(overflow_a, BufferResult::Dropped);

    let overflow_b = buf.buffer_signal(
        id.clone(),
        wk_b.clone(),
        make_signal("b-overflow"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(overflow_b, BufferResult::Dropped);

    buf.clear(&id, &wk_a);

    assert_eq!(
        buf.buffered_count(&id, &wk_a),
        0,
        "cleared key should be empty"
    );
    assert_eq!(
        buf.buffered_count(&id, &wk_b),
        3,
        "uncleared key should be unaffected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 5: WAIT — Empty-payload signal buffers and retrieves correctly
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Signal with empty payload should buffer and pop normally.
#[test]
fn attack_wait_empty_payload_signal_roundtrips() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(103);
    let wk = make_wait_key("empty-payload");

    let empty_signal = make_signal("empty-sig");
    assert!(empty_signal.payload.as_bytes().is_empty());

    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        empty_signal,
        BufferPolicy::BufferOne,
    );
    assert_eq!(result, BufferResult::Buffered);

    let popped = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(popped.signal_id, "empty-sig");
    assert!(popped.payload.as_bytes().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 6: WAIT — total_buffered_count consistency across many keys
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Buffer signals across 100 distinct keys, verify total count.
#[test]
fn attack_wait_total_buffered_count_across_100_keys() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);

    for i in 0..100u64 {
        let id = make_instance_id(200 + i);
        let wk = make_wait_key(&format!("key-{i}"));
        for j in 0..3 {
            buf.buffer_signal(
                id.clone(),
                wk.clone(),
                make_signal(&format!("sig-{j}")),
                BufferPolicy::BufferMany,
            );
        }
    }

    assert_eq!(buf.total_buffered_count(), 300);
    assert_eq!(buf.num_keys_with_signals(), 100);

    let id0 = make_instance_id(200);
    let wk0 = make_wait_key("key-0");
    buf.clear(&id0, &wk0);
    assert_eq!(buf.total_buffered_count(), 297);
    assert_eq!(buf.num_keys_with_signals(), 99);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 7: BATCH — calculate_batch_size with u32::MAX boundary
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: calculate_batch_size with max_per_cycle=u32::MAX and huge current_batch.
/// Should saturate to 0, not overflow.
#[test]
fn attack_batch_calculate_batch_size_u32_max_no_overflow() {
    assert_eq!(calculate_batch_size(100, u32::MAX, 0), 100);
    assert_eq!(calculate_batch_size(0, u32::MAX, 0), 0);
    assert_eq!(
        calculate_batch_size(100, u32::MAX, u32::MAX as usize - 50),
        0,
        "BUG: saturating_sub should prevent underflow"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 8: BATCH — calculate_batch_size with usize::MAX remaining
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Remaining timers at usize::MAX should be clamped by budget.
#[test]
fn attack_batch_calculate_batch_size_usize_max_remaining() {
    assert_eq!(
        calculate_batch_size(usize::MAX, 10, 0),
        10,
        "BUG: usize::MAX remaining should be clamped to max_per_cycle"
    );
    assert_eq!(
        calculate_batch_size(usize::MAX, 10, 5),
        5,
        "BUG: should respect budget remaining"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 9: BATCH — calculate_batch_size with zero inputs
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: All-zero inputs should return 0.
#[test]
fn attack_batch_calculate_batch_size_all_zeros() {
    assert_eq!(calculate_batch_size(0, 0, 0), 0);
    assert_eq!(calculate_batch_size(0, 100, 0), 0);
    assert_eq!(calculate_batch_size(100, 0, 0), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 10: BATCH — validate_timer_record with corrupt fire_at_ms=0
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Timer with fire_at_ms=0 is corrupt (invariant: fire time must be nonzero).
#[test]
fn attack_batch_validate_timer_fire_at_zero_rejects() {
    let record = make_timer_record(0, 1000, "inst-1");
    let result = validate_timer_record(&record);
    assert!(
        result.is_err(),
        "BUG: fire_at_ms=0 should be rejected as corrupt"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 11: BATCH — validate_timer_record with scheduled_at_ms=0
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Timer with scheduled_at_ms=0 is corrupt.
#[test]
fn attack_batch_validate_timer_scheduled_at_zero_rejects() {
    let record = make_timer_record(2000, 0, "inst-1");
    let result = validate_timer_record(&record);
    assert!(
        result.is_err(),
        "BUG: scheduled_at_ms=0 should be rejected as corrupt"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 12: BATCH — validate_timer_record fire_before_scheduled
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Timer where fire_at_ms < scheduled_at_ms is a time-travel corruption.
#[test]
fn attack_batch_validate_timer_fire_before_scheduled_rejects() {
    let record = make_timer_record(500, 1000, "inst-1");
    let result = validate_timer_record(&record);
    assert!(
        result.is_err(),
        "BUG: fire_at_ms < scheduled_at_ms should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 13: BATCH — validate_timer_record both fields zero
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Both timestamps zero should fail on fire_at_ms check first.
#[test]
fn attack_batch_validate_timer_both_zero_rejects() {
    let record = make_timer_record(0, 0, "inst-1");
    let result = validate_timer_record(&record);
    assert!(result.is_err(), "both-zero timer should be rejected");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 14: BATCH — validate_timer_record u64::MAX boundary
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Maximum u64 timestamp values should be valid.
#[test]
fn attack_batch_validate_timer_u64_max_boundary() {
    let record = make_timer_record(u64::MAX, u64::MAX - 1, "inst-1");
    let result = validate_timer_record(&record);
    assert!(result.is_ok(), "u64::MAX timestamps should be valid");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 16: BATCH — validate_timer_record fire_at equals scheduled_at
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: fire_at == scheduled_at is valid (instant timer).
#[test]
fn attack_batch_validate_timer_fire_equals_scheduled_valid() {
    let record = make_timer_record(1000, 1000, "inst-1");
    let result = validate_timer_record(&record);
    assert!(
        result.is_ok(),
        "fire_at == scheduled_at (instant timer) should be valid"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 17: SWITCH — BufferOne→BufferMany→BufferOne roundtrip
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: BufferOne (creates Single), then BufferMany (promotes to Many),
/// then BufferOne again (overwrites entire Many entry).
#[test]
fn attack_switch_buffer_one_to_many_to_one_roundtrip() {
    let config = SignalBufferConfig::new(10);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(300);
    let wk = make_wait_key("roundtrip");

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("first"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(buf.buffered_count(&id, &wk), 1);

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("second"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(buf.buffered_count(&id, &wk), 2);

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("overwrite"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(
        buf.buffered_count(&id, &wk),
        1,
        "BUG: BufferOne after BufferMany should overwrite to 1 signal"
    );

    let popped = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(
        popped.signal_id, "overwrite",
        "BUG: BufferOne should have replaced with latest signal"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 18: SWITCH — Many→Reject→Many transition preserves buffer
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: BufferMany fills buffer, then Reject policy is applied,
/// then BufferMany again. Reject should not clear existing buffer.
#[test]
fn attack_switch_many_reject_many_preserves_buffer() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(301);
    let wk = make_wait_key("mrm");

    for i in 0..3 {
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i}")),
            BufferPolicy::BufferMany,
        );
    }
    assert_eq!(buf.buffered_count(&id, &wk), 3);

    let reject_result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("rejected"),
        BufferPolicy::Reject,
    );
    assert_eq!(reject_result, BufferResult::Rejected);
    assert_eq!(
        buf.buffered_count(&id, &wk),
        3,
        "Reject should not modify buffer"
    );

    let buffer_result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-3"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(buffer_result, BufferResult::Buffered);
    assert_eq!(buf.buffered_count(&id, &wk), 4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 19: SWITCH — Single→Many promotion at capacity overflow
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: BufferOne creates Single, then BufferMany fills to capacity.
/// When transitioning from Single (1 signal) to Many, if capacity is 2,
/// adding the 2nd signal via Many creates [old, new]. Adding a 3rd should drop.
#[test]
fn attack_switch_single_to_many_at_capacity_overflow() {
    let config = SignalBufferConfig::new(2);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(302);
    let wk = make_wait_key("cap-overflow");

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("first"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(buf.buffered_count(&id, &wk), 1);

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("second"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(
        buf.buffered_count(&id, &wk),
        2,
        "BUG: promoted Many should have both signals"
    );

    let overflow = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("third"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(
        overflow,
        BufferResult::Dropped,
        "BUG: should drop at capacity"
    );
    assert_eq!(buf.buffered_count(&id, &wk), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 20: SWITCH — Rapid policy cycling (50 iterations)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Cycle through BufferOne/BufferMany/Reject 50 times rapidly.
/// Final state should be consistent with last applied policy.
#[test]
fn attack_switch_rapid_policy_cycling_50_iterations() {
    let config = SignalBufferConfig::new(10);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(303);
    let wk = make_wait_key("rapid-cycle");

    let policies = [
        BufferPolicy::BufferOne,
        BufferPolicy::BufferMany,
        BufferPolicy::Reject,
    ];

    for i in 0..50 {
        let policy = policies[i % 3];
        let result = buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("cycle-{i}")),
            policy,
        );
        match policy {
            BufferPolicy::Reject => assert_eq!(result, BufferResult::Rejected),
            BufferPolicy::BufferOne => assert_eq!(result, BufferResult::Buffered),
            BufferPolicy::BufferMany => assert_eq!(result, BufferResult::Buffered),
        }
    }

    // 50 iterations: 17 Reject + 17 BufferOne + 16 BufferMany
    // BufferOne overwrites each time, BufferMany adds
    // After 50 cycles: the exact count depends on promotion logic
    // But the buffer should be in a valid state
    let count = buf.buffered_count(&id, &wk);
    assert!(
        count > 0,
        "buffer should have at least 1 signal after 50 cycles"
    );
    assert!(
        count <= config.max_buffered_per_key,
        "BUG: count {count} exceeds max {max}",
        max = config.max_buffered_per_key
    );

    // All signals should be poppable in FIFO order
    while buf.buffered_count(&id, &wk) > 0 {
        let popped = buf.pop_buffered(&id, &wk);
        assert!(
            popped.is_some(),
            "pop should succeed while buffer has signals"
        );
    }
    assert_eq!(buf.buffered_count(&id, &wk), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 21: SWITCH — Cross-key policy isolation
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Different keys can have different policies simultaneously.
/// Policy on key A must not affect policy on key B.
#[test]
fn attack_switch_cross_key_policy_isolation() {
    let config = SignalBufferConfig::new(3);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(304);
    let wk_reject = make_wait_key("reject-key");
    let wk_buffer = make_wait_key("buffer-key");

    let result = buf.buffer_signal(
        id.clone(),
        wk_reject.clone(),
        make_signal("r-1"),
        BufferPolicy::Reject,
    );
    assert_eq!(result, BufferResult::Rejected);
    assert_eq!(buf.buffered_count(&id, &wk_reject), 0);

    let result = buf.buffer_signal(
        id.clone(),
        wk_buffer.clone(),
        make_signal("b-1"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Buffered);
    assert_eq!(buf.buffered_count(&id, &wk_buffer), 1);

    let result = buf.buffer_signal(
        id.clone(),
        wk_reject.clone(),
        make_signal("r-2"),
        BufferPolicy::Reject,
    );
    assert_eq!(result, BufferResult::Rejected);
    assert_eq!(buf.buffered_count(&id, &wk_reject), 0);

    let result = buf.buffer_signal(
        id.clone(),
        wk_buffer.clone(),
        make_signal("b-2"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Buffered);
    assert_eq!(buf.buffered_count(&id, &wk_buffer), 2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 22: SWITCH — apply_policy has_existing_buffer unused but tested
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: The _has_existing_buffer parameter is unused in apply_policy.
/// This test verifies the behavior is correct regardless of its value.
#[test]
fn attack_switch_apply_policy_existing_buffer_unused_consistency() {
    for policy in [
        BufferPolicy::Reject,
        BufferPolicy::BufferOne,
        BufferPolicy::BufferMany,
    ] {
        let (d1, b1) = apply_policy(policy, false, false);
        let (d2, b2) = apply_policy(policy, false, true);
        assert_eq!(
            d1, d2,
            "BUG: apply_policy result differs by has_existing_buffer for {policy:?}"
        );
        assert_eq!(
            b1, b2,
            "BUG: apply_policy buffer_result differs by has_existing_buffer for {policy:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 23: WAIT — BufferedSignal with maximum-size payload
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Signal with payload at exactly 64 KiB limit should work.
#[test]
fn attack_wait_max_payload_signal_buffers() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(305);
    let wk = make_wait_key("max-payload");

    let max_payload = vec![0xFF; 65536];
    let signal = make_signal_with_payload("max-sig", max_payload);

    let result = buf.buffer_signal(id.clone(), wk.clone(), signal, BufferPolicy::BufferMany);
    assert_eq!(result, BufferResult::Buffered);

    let popped = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(popped.signal_id, "max-sig");
    assert_eq!(popped.payload.as_bytes().len(), 65536);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 24: BATCH — calculate_batch_size current_batch exceeds max
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: current_batch already exceeds max_per_cycle — should return 0.
#[test]
fn attack_batch_calculate_batch_size_current_exceeds_max() {
    assert_eq!(calculate_batch_size(100, 10, 20), 0);
    assert_eq!(calculate_batch_size(100, 10, 100), 0);
    assert_eq!(calculate_batch_size(1, 1, 1), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 25: WAIT — num_keys_with_signals tracks unique (instance, key) pairs
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Buffering the same key twice should NOT create two entries.
#[test]
fn attack_wait_num_keys_no_duplicates_on_same_key() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(306);
    let wk = make_wait_key("unique-key");

    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("s1"),
        BufferPolicy::BufferMany,
    );
    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("s2"),
        BufferPolicy::BufferMany,
    );
    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("s3"),
        BufferPolicy::BufferMany,
    );

    assert_eq!(
        buf.num_keys_with_signals(),
        1,
        "same key should count as 1 entry"
    );
    assert_eq!(buf.buffered_count(&id, &wk), 3);
}
