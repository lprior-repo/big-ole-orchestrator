#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for session (InstanceRegistry) and
//! sync (SignalBuffer) in vo-actor.
//!
//! Attack vectors:
//! - SESSION: concurrent registration, INV-1..INV-5 violations, stop_fn races
//! - SYNC: buffer policy transitions, boundary conditions, policy bypass

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vo_actor::instance_registry::{
    InstanceActorHandle, InstanceRegistry, RegistryConfig, RegistryError,
};
use vo_actor::signal_buffer::{
    apply_policy, can_buffer, BufferResult, BufferedSignal, SignalBuffer, SignalBufferConfig,
};
use vo_actor::{SignalPayload, WaitKey};
use vo_types::{BufferPolicy, InstanceId, SignalDelivery, TimestampMs};

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

fn test_handle(id: u64) -> InstanceActorHandle {
    InstanceActorHandle::test(id)
}

fn make_signal(id: &str) -> BufferedSignal {
    BufferedSignal::new(id.to_string(), SignalPayload::empty(), TimestampMs::now())
}

fn make_wait_key(s: &str) -> WaitKey {
    WaitKey::parse(s).expect("test wait key should be valid")
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 1: SESSION — INV-1 single-active via concurrent registration
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Two threads register the same InstanceId concurrently.
/// INV-1 says at most one handle per InstanceId at any time.
/// The register method uses &mut self, so true concurrency is impossible
/// with a single registry reference. But we can test stop_fn races.
#[test]
fn attack_session_inv1_single_active_stop_fn_race() {
    let config = RegistryConfig {
        stop_timeout: Duration::from_millis(100),
    };
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(1);

    let handle1 = test_handle(1);
    let stop_count = Arc::new(AtomicUsize::new(0));

    // Register handle1
    let result = registry.register(id.clone(), handle1, |_h| Ok(()));
    assert!(result.is_ok());
    assert!(registry.is_active(&id));
    assert_eq!(registry.active_count(), 1);

    // Register handle2 for the same id — should stop handle1 first
    let sc = Arc::clone(&stop_count);
    let handle2 = test_handle(2);
    let result = registry.register(id.clone(), handle2, move |_h| {
        sc.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    assert!(
        result.is_ok(),
        "stop_fn succeeded, handle2 should be registered"
    );

    // Only handle2 should be active
    assert!(registry.is_active(&id));
    assert_eq!(registry.active_count(), 1);

    let current = registry.lookup(&id).unwrap();
    assert_eq!(
        current.handle_id(),
        2,
        "handle2 should be the active handle"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 2: SESSION — INV-5 no partial mutations on stop failure
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: When stop_fn fails, the old handle should remain active.
/// INV-5 says: on error, registry state is unchanged.
#[test]
fn attack_session_inv5_no_partial_mutation_on_stop_failure() {
    let config = RegistryConfig {
        stop_timeout: Duration::from_millis(100),
    };
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(2);

    let handle1 = test_handle(42);
    registry.register(id.clone(), handle1, |_h| Ok(())).unwrap();

    // Register handle2 with a failing stop_fn
    let handle2 = test_handle(99);
    let result = registry.register(id.clone(), handle2, |_h| {
        Err("simulated stop failure".to_string())
    });

    assert!(
        matches!(result, Err(RegistryError::StopFailed { .. })),
        "stop failure should return StopFailed"
    );

    // INV-5: original handle should still be active
    assert!(registry.is_active(&id));
    let current = registry.lookup(&id).unwrap();
    assert_eq!(
        current.handle_id(),
        42,
        "BUG: original handle was replaced despite stop failure — INV-5 violated"
    );
    assert_eq!(registry.active_count(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 3: SESSION — INV-5 no partial mutation on timeout
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: When stop_fn exceeds timeout, the old handle should remain active.
#[test]
fn attack_session_inv5_no_partial_mutation_on_timeout() {
    let config = RegistryConfig {
        stop_timeout: Duration::from_millis(10),
    };
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(3);

    let handle1 = test_handle(1);
    registry.register(id.clone(), handle1, |_h| Ok(())).unwrap();

    // Register handle2 with a slow stop_fn (sleeps longer than timeout)
    let handle2 = test_handle(2);
    let result = registry.register(id.clone(), handle2, |_h| {
        thread::sleep(Duration::from_millis(500));
        Ok(())
    });

    assert!(
        matches!(result, Err(RegistryError::StopTimeout { .. })),
        "slow stop_fn should trigger StopTimeout"
    );

    // INV-5: original handle should still be active
    assert!(registry.is_active(&id));
    let current = registry.lookup(&id).unwrap();
    assert_eq!(
        current.handle_id(),
        1,
        "BUG: original handle was replaced despite timeout — INV-5 violated"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 4: SESSION — Deregister non-existent instance
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Deregister an InstanceId that was never registered.
#[test]
fn attack_session_deregister_nonexistent() {
    let config = RegistryConfig::default();
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(4);

    let result = registry.deregister(&id);
    assert!(
        matches!(result, Err(RegistryError::NotRegistered { .. })),
        "deregistering nonexistent id should return NotRegistered"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 5: SESSION — Double deregister
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Register, deregister, then deregister again.
#[test]
fn attack_session_double_deregister() {
    let config = RegistryConfig::default();
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(5);

    registry
        .register(id.clone(), test_handle(1), |_h| Ok(()))
        .unwrap();
    assert!(registry.deregister(&id).is_ok());
    assert!(!registry.is_active(&id));

    let result = registry.deregister(&id);
    assert!(
        matches!(result, Err(RegistryError::NotRegistered { .. })),
        "second deregister should return NotRegistered"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 6: SESSION — INV-3 count consistency after many ops
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Register and deregister many instances, verify count stays consistent.
#[test]
fn attack_session_inv3_count_consistency() {
    let config = RegistryConfig::default();
    let mut registry = InstanceRegistry::new(config);

    for i in 0..100u64 {
        let id = make_instance_id(100 + i);
        assert!(registry
            .register(id.clone(), test_handle(i), |_h| Ok(()))
            .is_ok());
        assert_eq!(
            registry.active_count(),
            (i + 1) as usize,
            "BUG: count mismatch after register {i}"
        );
    }

    for i in 0..100u64 {
        let id = make_instance_id(100 + i);
        assert!(registry.deregister(&id).is_ok());
        assert_eq!(
            registry.active_count(),
            (99 - i) as usize,
            "BUG: count mismatch after deregister {i}"
        );
    }

    assert_eq!(registry.active_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 7: SESSION — Replace chain (stop-before-replace N times)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Register the same InstanceId 50 times with different handles.
/// Each time, the old handle should be stopped and the new one should replace it.
#[test]
fn attack_session_replace_chain_50() {
    let config = RegistryConfig::default();
    let mut registry = InstanceRegistry::new(config);
    let id = make_instance_id(6);

    for i in 0..50u64 {
        let result = registry.register(id.clone(), test_handle(i), |_h| Ok(()));
        assert!(result.is_ok(), "replace {i} should succeed");
        assert_eq!(registry.active_count(), 1);
        let current = registry.lookup(&id).unwrap();
        assert_eq!(current.handle_id(), i, "handle {i} should be active");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 8: SYNC — apply_policy with Reject policy
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Reject policy should reject even when there's no matching wait.
#[test]
fn attack_sync_apply_policy_reject() {
    let (delivery, buffer_result) = apply_policy(BufferPolicy::Reject, false, false);
    assert_eq!(delivery, SignalDelivery::Rejected);
    assert_eq!(buffer_result, Some(BufferResult::Rejected));

    // Even with existing buffer, Reject should still reject
    let (delivery, buffer_result) = apply_policy(BufferPolicy::Reject, false, true);
    assert_eq!(delivery, SignalDelivery::Rejected);
    assert_eq!(buffer_result, Some(BufferResult::Rejected));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 9: SYNC — apply_policy with matching wait
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: When there's a matching wait, ALL policies should accept immediately.
#[test]
fn attack_sync_apply_policy_matching_wait_overrides() {
    for policy in [
        BufferPolicy::Reject,
        BufferPolicy::BufferOne,
        BufferPolicy::BufferMany,
    ] {
        let (delivery, buffer_result) = apply_policy(policy, true, false);
        assert_eq!(
            delivery,
            SignalDelivery::Accepted,
            "BUG: matching wait should override {policy:?} policy"
        );
        assert_eq!(
            buffer_result, None,
            "BUG: matching wait should not buffer for {policy:?} policy"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 10: SYNC — can_buffer boundary at max_buffered_per_key
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: can_buffer should return false at exactly max_buffered_per_key.
#[test]
fn attack_sync_can_buffer_at_boundary() {
    let config = SignalBufferConfig::new(5);

    // BufferMany: should allow up to 5 (exclusive)
    assert!(can_buffer(BufferPolicy::BufferMany, false, 0, &config));
    assert!(can_buffer(BufferPolicy::BufferMany, false, 4, &config));
    assert!(
        !can_buffer(BufferPolicy::BufferMany, false, 5, &config),
        "BUG: can_buffer at exact max should return false"
    );
    assert!(
        !can_buffer(BufferPolicy::BufferMany, false, 100, &config),
        "BUG: can_buffer way over max should return false"
    );

    // BufferOne: always returns true (overwrites)
    assert!(can_buffer(BufferPolicy::BufferOne, false, 0, &config));
    assert!(can_buffer(BufferPolicy::BufferOne, false, 100, &config));

    // Reject: always returns false
    assert!(!can_buffer(BufferPolicy::Reject, false, 0, &config));
    assert!(!can_buffer(BufferPolicy::Reject, false, 100, &config));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 11: SYNC — BufferMany overflow beyond max_buffered_per_key
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Buffer exactly max_buffered_per_key signals, then try one more.
#[test]
fn attack_sync_buffer_many_exact_overflow() {
    let config = SignalBufferConfig::new(3);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(10);
    let wk = make_wait_key("timer_fired");

    // Buffer 3 signals
    for i in 0..3 {
        let result = buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i}")),
            BufferPolicy::BufferMany,
        );
        assert_eq!(result, BufferResult::Buffered, "signal {i} should buffer");
    }

    // 4th should be dropped
    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-overflow"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(
        result,
        BufferResult::Dropped,
        "BUG: signal beyond max should be dropped, got {result:?}"
    );

    // Count should be 3
    assert_eq!(buf.buffered_count(&id, &wk), 3);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 12: SYNC — BufferOne overwrites previous signal
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: BufferOne should always overwrite the previous signal.
#[test]
fn attack_sync_buffer_one_overwrite() {
    let config = SignalBufferConfig::new(100);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(11);
    let wk = make_wait_key("step_completed");

    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-A"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(result, BufferResult::Buffered);

    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-B"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(result, BufferResult::Buffered);

    // Should only have 1 signal (the latest)
    assert_eq!(buf.buffered_count(&id, &wk), 1);

    // Pop should return sig-B
    let popped = buf.pop_buffered(&id, &wk).unwrap();
    assert_eq!(
        popped.signal_id, "sig-B",
        "BUG: BufferOne didn't overwrite — got old signal"
    );

    // Buffer should be empty now
    assert_eq!(buf.buffered_count(&id, &wk), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 13: SYNC — Pop from empty buffer
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Pop from a buffer that was never written to.
#[test]
fn attack_sync_pop_from_empty() {
    let mut buf = SignalBuffer::with_default_config();
    let id = make_instance_id(12);
    let wk = make_wait_key("nonexistent");

    let result = buf.pop_buffered(&id, &wk);
    assert!(result.is_none(), "pop from empty buffer should return None");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 14: SYNC — BufferMany transition from Single to Many
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Start with BufferOne (creates Single), then switch to BufferMany.
/// The Single should be promoted to Many with both signals preserved.
#[test]
fn attack_sync_buffer_one_to_many_transition() {
    let config = SignalBufferConfig::new(5);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(13);
    let wk = make_wait_key("transition");

    // BufferOne creates a Single entry
    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("first"),
        BufferPolicy::BufferOne,
    );
    assert_eq!(buf.buffered_count(&id, &wk), 1);

    // BufferMany on top of Single should promote to Many
    buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("second"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(
        buf.buffered_count(&id, &wk),
        2,
        "BUG: BufferMany after BufferOne should have 2 signals (Single promoted to Many)"
    );

    // Both signals should be retrievable
    let all = buf.peek_all(&id, &wk);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].signal_id, "first");
    assert_eq!(all[1].signal_id, "second");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 15: SYNC — Clear removes all buffered signals
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Buffer signals, clear, verify empty.
#[test]
fn attack_sync_clear_empties_buffer() {
    let config = SignalBufferConfig::new(10);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(14);
    let wk = make_wait_key("clear-test");

    for i in 0..5 {
        buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i}")),
            BufferPolicy::BufferMany,
        );
    }

    assert_eq!(buf.buffered_count(&id, &wk), 5);

    buf.clear(&id, &wk);

    assert_eq!(buf.buffered_count(&id, &wk), 0);
    assert!(buf.pop_buffered(&id, &wk).is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 16: SYNC — Multiple instance IDs are isolated
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Signals for different instance IDs should not interfere.
#[test]
fn attack_sync_instance_isolation() {
    let config = SignalBufferConfig::new(3);
    let mut buf = SignalBuffer::new(config);
    let id1 = make_instance_id(20);
    let id2 = make_instance_id(21);
    let wk = make_wait_key("shared-key");

    // Fill id1's buffer
    for i in 0..3 {
        buf.buffer_signal(
            id1.clone(),
            wk.clone(),
            make_signal(&format!("id1-sig-{i}")),
            BufferPolicy::BufferMany,
        );
    }

    // id1 should be full
    let result = buf.buffer_signal(
        id1.clone(),
        wk.clone(),
        make_signal("id1-overflow"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Dropped);

    // id2 should still accept signals
    let result = buf.buffer_signal(
        id2.clone(),
        wk.clone(),
        make_signal("id2-sig-0"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Buffered);
    assert_eq!(buf.buffered_count(&id2, &wk), 1);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 17: SESSION — Register with zero timeout panics
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: RegistryConfig with zero timeout should panic on construction.
/// This is a documented invariant.
#[test]
#[should_panic(expected = "stop_timeout must be greater than zero")]
fn attack_session_zero_timeout_panics() {
    let config = RegistryConfig {
        stop_timeout: Duration::ZERO,
    };
    let _registry = InstanceRegistry::new(config);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 18: SYNC — SignalBufferConfig max_buffered_per_key=1
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Config with max_buffered_per_key=1 means BufferMany can only hold 1.
#[test]
fn attack_sync_config_max_one() {
    let config = SignalBufferConfig::new(1);
    assert_eq!(config.max_buffered_per_key, 1);

    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(15);
    let wk = make_wait_key("max-one");

    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-1"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Buffered);

    // Second should be dropped (max is 1)
    let result = buf.buffer_signal(
        id.clone(),
        wk.clone(),
        make_signal("sig-2"),
        BufferPolicy::BufferMany,
    );
    assert_eq!(result, BufferResult::Dropped);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 19: SESSION — Many distinct instances, verify lookup isolation
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Register 1000 distinct instances, verify each lookup returns correct handle.
#[test]
fn attack_session_1000_distinct_lookups() {
    let config = RegistryConfig::default();
    let mut registry = InstanceRegistry::new(config);

    for i in 0..1000u64 {
        let id = make_instance_id(200 + i);
        registry
            .register(id.clone(), test_handle(i), |_h| Ok(()))
            .unwrap();
    }

    assert_eq!(registry.active_count(), 1000);

    for i in [0, 42, 500, 999] {
        let id = make_instance_id(200 + i);
        let handle = registry.lookup(&id).unwrap();
        assert_eq!(handle.handle_id(), i);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 20: SYNC — Reject policy never buffers regardless of config
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Reject policy should always return Rejected, even with huge config.
#[test]
fn attack_sync_reject_never_buffers() {
    let config = SignalBufferConfig::new(1_000_000);
    let mut buf = SignalBuffer::new(config);
    let id = make_instance_id(16);
    let wk = make_wait_key("reject-test");

    for i in 0..100 {
        let result = buf.buffer_signal(
            id.clone(),
            wk.clone(),
            make_signal(&format!("sig-{i}")),
            BufferPolicy::Reject,
        );
        assert_eq!(
            result,
            BufferResult::Rejected,
            "BUG: Reject policy buffered signal {i}"
        );
    }

    assert_eq!(buf.buffered_count(&id, &wk), 0);
    assert_eq!(buf.total_buffered_count(), 0);
}
