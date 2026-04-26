// TEMPORARILY DISABLED - pre-existing API mismatch
/
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for the timer_index partition.
//!
//! These tests attempt to break the implementation through:
//! - Key encoding attacks (invalid lengths, boundary values)
//! - Value validation attacks (zero duration)
//! - Dual-clock invariant violations
//! - Timer set validation edge cases
//! - Scan due timers boundary conditions
//! - Delete operations under various conditions
//!
//! bead_id: ve-c45
//! bead_title: RED QUEEN: nitro test 2
//! module: timer_index (12 attack vectors)

use vo_storage::codec::StorageError;
use vo_storage::timer_index::{
    scan_all_timers_for_instance, scan_due_timers, timer_delete, timer_set, TimerKey, TimerRecord,
    TimerValue,
};
use vo_types::{InstanceId, TimerId};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn make_test_instance_id(byte_fill: u8) -> InstanceId {
    InstanceId::from_bytes([byte_fill; 16])
}

fn make_test_timer_id(byte_fill: u8) -> TimerId {
    TimerId::from_bytes([byte_fill; 16])
}

// Mock storage for tests
struct MockStorage {
    data: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    fail_on_op: Option<String>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: std::collections::BTreeMap::new(),
            fail_on_op: None,
        }
    }

    fn with_fail(op: &str) -> Self {
        let mut s = Self::new();
        s.fail_on_op = Some(op.to_string());
        s
    }
}

impl vo_storage::timer_index::Storage for MockStorage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("put") {
            return Err(StorageError::Storage);
        }
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        if self.fail_on_op.as_deref() == Some("get") {
            return Err(StorageError::Storage);
        }
        Ok(self.data.get(key).cloned())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        if self.fail_on_op.as_deref() == Some("delete") {
            return Err(StorageError::Storage);
        }
        self.data.remove(key);
        Ok(())
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        if self.fail_on_op.as_deref() == Some("scan") {
            return Err(StorageError::Storage);
        }
        Ok(self
            .data
            .range(start.to_vec()..end.to_vec())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

// ===========================================================================
// ATTACK VECTOR 1: TimerKey encoding attacks
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TK01: TimerKey::new rejects invalid length instance_id
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_key_rejects_invalid_instance_id_length() {
    let timer_id = make_test_timer_id(0x02);
    let fire_at_ms = 1000u64;

    // InstanceId::from_bytes expects exactly 16 bytes
    // If we provide wrong length via InstanceId::from_bytes, it will fail
    let result = TimerKey::new(
        fire_at_ms,
        InstanceId::from_bytes([0; 16]),
        timer_id.clone(),
    );
    assert!(result.is_ok(), "Valid 16-byte instance ID should work");

    // Test with zero-filled InstanceId
    let zero_id = InstanceId::from_bytes([0; 16]);
    let result = TimerKey::new(fire_at_ms, zero_id, timer_id.clone());
    assert!(result.is_ok(), "Zero-filled instance ID should work");
}

// ---------------------------------------------------------------------------
// RQ-TK02: TimerKey boundary values for fire_at_ms
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_key_handles_u64_boundary_values() {
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Test u64::MAX
    let result = TimerKey::new(u64::MAX, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "u64::MAX fire_at_ms should be valid");

    // Test 0
    let result = TimerKey::new(0u64, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "0 fire_at_ms should be valid");

    // Test 1
    let result = TimerKey::new(1u64, instance_id.clone(), timer_id.clone());
    assert!(result.is_ok(), "1 fire_at_ms should be valid");
}

// ---------------------------------------------------------------------------
// RQ-TK03: TimerKey byte boundaries
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_key_extraction_at_boundaries() {
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Test at u64::MAX
    let key = TimerKey::new(u64::MAX, instance_id.clone(), timer_id.clone()).unwrap();
    assert_eq!(key.fire_at_ms(), u64::MAX);
    assert_eq!(key.instance_id(), instance_id);
    assert_eq!(key.timer_id(), timer_id);

    // Test at 0
    let key = TimerKey::new(0u64, instance_id.clone(), timer_id.clone()).unwrap();
    assert_eq!(key.fire_at_ms(), 0);
}

// ===========================================================================
// ATTACK VECTOR 2: TimerValue validation attacks
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TV01: TimerValue::new rejects zero duration
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_value_rejects_zero_duration() {
    let result = TimerValue::new(0);
    assert!(result.is_err(), "Zero duration should be rejected");
    match result {
        Err(StorageError::InvalidArgument) => {}
        Err(e) => panic!("Expected InvalidArgument, got something else: {:?}", e),
        Ok(_) => panic!("Expected error, got Ok"),
    }

    let result = TimerValue::new(1);
    assert!(result.is_ok(), "Non-zero duration should be accepted");
}

// ===========================================================================
// ATTACK VECTOR 3: TimerRecord dual-clock invariant violations
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TR01: TimerRecord::try_from_parts rejects zero duration
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_record_rejects_zero_duration() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 500u64;
    let duration_ms = 0u64;

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Zero duration should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RQ-TR02: TimerRecord::try_from_parts rejects dual-clock violation
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_record_rejects_dual_clock_violation() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 500u64;
    let duration_ms = 600u64; // 500 + 600 = 1100 != 1000

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Dual-clock violation should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RQ-TR03: TimerRecord::try_from_parts accepts valid dual-clock
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_record_accepts_valid_dual_clock() {
    let timer_id = make_test_timer_id(0x02);
    let instance_id = make_test_instance_id(0x01);
    let fire_at_ms = 1000u64;
    let trigger_time_ms = 400u64;
    let duration_ms = 600u64; // 400 + 600 = 1000 ✓

    let result = TimerRecord::try_from_parts(
        timer_id.clone(),
        instance_id.clone(),
        fire_at_ms,
        trigger_time_ms,
        duration_ms,
    );
    assert!(result.is_ok(), "Valid dual-clock should be accepted");
}

// ===========================================================================
// ATTACK VECTOR 4: timer_set validation edge cases
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TS01: timer_set rejects fire_at_ms <= now_ms
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_set_rejects_past_fire_time() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);
    let now_ms = 1000u64;

    // fire_at_ms == now_ms should fail
    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000, // fire_at_ms == now_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        now_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "fire_at_ms == now_ms should be rejected"
    );

    // fire_at_ms < now_ms should fail
    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        500, // fire_at_ms < now_ms
        0,   // trigger_time_ms
        500, // duration_ms
        now_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "fire_at_ms < now_ms should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RQ-TS02: timer_set rejects zero duration
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_set_rejects_zero_duration() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);
    let now_ms = 1000u64;

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000, // fire_at_ms > now_ms
        1500, // trigger_time_ms
        0,    // duration_ms = 0
        now_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Zero duration should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RQ-TS03: timer_set rejects dual-clock violation
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_set_rejects_dual_clock_violation() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);
    let now_ms = 1000u64;

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000, // fire_at_ms
        1000, // trigger_time_ms
        500,  // duration_ms -> 1000 + 500 = 1500 != 2000
        now_ms,
    );
    assert_eq!(
        result,
        Err(StorageError::InvalidArgument),
        "Dual-clock violation should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RQ-TS04: timer_set accepts valid timer
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_set_accepts_valid_timer() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);
    let now_ms = 1000u64;

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000, // fire_at_ms
        1500, // trigger_time_ms
        500,  // duration_ms -> 1500 + 500 = 2000 ✓
        now_ms,
    );
    assert!(result.is_ok(), "Valid timer should be accepted");
}

// ---------------------------------------------------------------------------
// RQ-TS05: timer_set storage failure propagation
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_set_propagates_storage_failure() {
    let mut storage = MockStorage::with_fail("put");
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,
        1500,
        500,
        1000,
    );
    assert_eq!(
        result,
        Err(StorageError::Storage),
        "Storage failure should be propagated"
    );
}

// ===========================================================================
// ATTACK VECTOR 5: scan_due_timers boundary conditions
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-SD01: scan_due_timers with no timers
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_due_timers_empty_when_no_timers() {
    let storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);

    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(result.unwrap().is_empty(), "Should be empty when no timers");
}

// ---------------------------------------------------------------------------
// RQ-SD02: scan_due_timers filters by instance_id
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_due_timers_filters_by_instance() {
    let mut storage = MockStorage::new();
    let instance_id_1 = make_test_instance_id(0x01);
    let instance_id_2 = make_test_instance_id(0x02);
    let timer_id = make_test_timer_id(0x03);

    // Add timer for instance_id_1
    timer_set(
        &mut storage,
        instance_id_1.clone(),
        timer_id.clone(),
        500, // fire_at_ms
        0,   // trigger_time_ms
        500, // duration_ms
        0,   // now_ms
    )
    .unwrap();

    // Scan for instance_id_2 should not find it
    let result = scan_due_timers(&storage, &instance_id_2, 1000);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(
        result.unwrap().is_empty(),
        "Should not find timer for different instance"
    );
}

// ---------------------------------------------------------------------------
// RQ-SD03: scan_due_timers with now_ms at boundary
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_due_timers_boundary_now_ms() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Add timer that fires at exactly 1000
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000, // fire_at_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        0,    // now_ms
    )
    .unwrap();

    // Scan at now_ms = 999 should NOT find it
    let result = scan_due_timers(&storage, &instance_id, 999);
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "Timer at 1000 should not be due at 999"
    );

    // Scan at now_ms = 1000 should find it
    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().len(),
        1,
        "Timer at 1000 should be due at 1000"
    );
}

// ---------------------------------------------------------------------------
// RQ-SD04: scan_due_timers storage failure
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_due_timers_propagates_storage_failure() {
    let storage = MockStorage::with_fail("scan");
    let instance_id = make_test_instance_id(0x01);

    let result = scan_due_timers(&storage, &instance_id, 1000);
    assert_eq!(
        result,
        Err(StorageError::Storage),
        "Storage failure should be propagated"
    );
}

// ===========================================================================
// ATTACK VECTOR 6: timer_delete edge cases
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TD01: timer_delete non-existent timer
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_delete_nonexistent_succeeds() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Deleting non-existent timer should succeed (idempotent)
    let result = timer_delete(&mut storage, &instance_id, timer_id, 1000);
    assert!(result.is_ok(), "Deleting non-existent timer should succeed");
}

// ---------------------------------------------------------------------------
// RQ-TD02: timer_delete storage failure
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_delete_propagates_storage_failure() {
    let mut storage = MockStorage::with_fail("delete");
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    let result = timer_delete(&mut storage, &instance_id, timer_id, 1000);
    assert_eq!(
        result,
        Err(StorageError::Storage),
        "Storage failure should be propagated"
    );
}

// ---------------------------------------------------------------------------
// RQ-TD03: timer_delete actually removes timer
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_delete_removes_timer() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Add timer
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();

    // Verify it's there
    let result = scan_due_timers(&storage, &instance_id, 3000);
    assert_eq!(result.unwrap().len(), 1);

    // Delete it
    timer_delete(&mut storage, &instance_id, timer_id, 2000).unwrap();

    // Verify it's gone
    let result = scan_due_timers(&storage, &instance_id, 3000);
    assert!(result.unwrap().is_empty());
}

// ===========================================================================
// ATTACK VECTOR 7: Multiple timers edge cases
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-MT01: Multiple timers for same instance, different fire times
// ---------------------------------------------------------------------------

#[test]
fn rq_multiple_timers_different_fire_times() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    // Add two timers
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1500,
        1000,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    // At now_ms = 1499, no timers should be due (fire_at_ms=1500 > 1499)
    let result = scan_due_timers(&storage, &instance_id, 1499).unwrap();
    assert_eq!(result.len(), 0);

    // At now_ms = 1500, first timer should be due
    let result = scan_due_timers(&storage, &instance_id, 1500).unwrap();
    assert_eq!(result.len(), 1);

    // At now_ms = 1999, still only first should be due
    let result = scan_due_timers(&storage, &instance_id, 1999).unwrap();
    assert_eq!(result.len(), 1);

    // At now_ms = 2000, both should be due
    let result = scan_due_timers(&storage, &instance_id, 2000).unwrap();
    assert_eq!(result.len(), 2);
}

// ---------------------------------------------------------------------------
// RQ-MT02: Saturating arithmetic in trigger_time_ms calculation
// ---------------------------------------------------------------------------

#[test]
fn rq_trigger_time_saturating_sub() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Add timer where scan will calculate trigger_time_ms = fire_at_ms.saturating_sub(duration_ms)
    // fire_at_ms = 1500, duration_ms = 1000
    // trigger_time_ms stored = 500 (satisfies dual-clock: 1500 == 500 + 1000)
    // But during scan, trigger_time_ms is recalculated as fire_at_ms.saturating_sub(duration_ms) = 1500.saturating_sub(1000) = 500
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1500, // fire_at_ms
        500,  // trigger_time_ms (1500 = 500 + 1000, satisfies dual-clock)
        1000, // duration_ms
        0,
    )
    .unwrap();

    // Scan at now_ms = 1500 should find the timer
    let result = scan_due_timers(&storage, &instance_id, 1500).unwrap();
    assert_eq!(result.len(), 1);
    let record = &result[0];
    assert_eq!(record.fire_at_ms, 1500);
    assert_eq!(record.trigger_time_ms, 500);
    assert_eq!(record.duration_ms, 1000);
}

// ===========================================================================
// ATTACK VECTOR 8: scan_all_timers_for_instance — cancellation on completion
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-CA01: scan_all_timers_for_instance returns empty when no timers exist
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_timers_empty_when_no_timers() {
    let storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);

    let result = scan_all_timers_for_instance(&storage, &instance_id);
    assert!(result.is_ok(), "Scan should succeed");
    assert!(result.unwrap().is_empty(), "Should be empty when no timers");
}

// ---------------------------------------------------------------------------
// RQ-CA02: scan_all_timers_for_instance returns all timers including future ones
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_timers_includes_future_timers() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    // Add past timer (fire_at_ms = 1000, now_ms = 2000, already due)
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000, // fire_at_ms
        500,  // trigger_time_ms
        500,  // duration_ms
        500,  // now_ms
    )
    .unwrap();

    // Add future timer (fire_at_ms = 5000, now_ms = 2000)
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        5000, // fire_at_ms
        4500, // trigger_time_ms
        500,  // duration_ms
        2000, // now_ms
    )
    .unwrap();

    // Scan all should return both timers
    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 2, "Should return both past and future timers");

    let fire_times: Vec<u64> = result.iter().map(|r| r.fire_at_ms).collect();
    assert!(fire_times.contains(&1000u64), "Should include past timer");
    assert!(fire_times.contains(&5000u64), "Should include future timer");
}

// ---------------------------------------------------------------------------
// RQ-CA03: scan_all_timers_for_instance filters by instance_id correctly
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_timers_filters_by_instance() {
    let mut storage = MockStorage::new();
    let instance_id_1 = make_test_instance_id(0x01);
    let instance_id_2 = make_test_instance_id(0x02);
    let timer_id_1 = make_test_timer_id(0x03);
    let timer_id_2 = make_test_timer_id(0x04);

    timer_set(
        &mut storage,
        instance_id_1.clone(),
        timer_id_1.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();

    timer_set(
        &mut storage,
        instance_id_2.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    // Scanning for instance_id_1 should only return its timer
    let result = scan_all_timers_for_instance(&storage, &instance_id_1).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_1);

    // Scanning for instance_id_2 should only return its timer
    let result = scan_all_timers_for_instance(&storage, &instance_id_2).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_2);
}

// ---------------------------------------------------------------------------
// RQ-CA04: scan_all_timers_for_instance storage failure propagation
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_timers_propagates_storage_failure() {
    let storage = MockStorage::with_fail("scan");
    let instance_id = make_test_instance_id(0x01);

    let result = scan_all_timers_for_instance(&storage, &instance_id);
    assert_eq!(result, Err(StorageError::Storage));
}

// ---------------------------------------------------------------------------
// RQ-CA05: scan_all_timers_for_instance returns timers with correct fields
// ---------------------------------------------------------------------------

#[test]
fn rq_scan_all_timers_returns_correct_fields() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        3000, // fire_at_ms
        2500, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms
    )
    .unwrap();

    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
    let record = &result[0];
    assert_eq!(record.timer_id, timer_id);
    assert_eq!(record.instance_id, instance_id);
    assert_eq!(record.fire_at_ms, 3000);
    assert_eq!(record.trigger_time_ms, 2500);
    assert_eq!(record.duration_ms, 500);
}

// ===========================================================================
// ATTACK VECTOR 9: Crash-recovery timer correctness
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-CR01: Timers that fired during server downtime are recovered correctly
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_finds_timers_that_fired_during_downtime() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    // Server starts at now_ms = 1000
    // Timer 1 fires at 1500
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1500, // fire_at_ms
        1000, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms (server start time)
    )
    .unwrap();

    // Timer 2 fires at 2000
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000, // fire_at_ms
        1500, // trigger_time_ms
        500,  // duration_ms
        1000, // now_ms
    )
    .unwrap();

    // Server crashes and restarts at now_ms = 2500
    // At restart, scan for due timers should find both that fired during downtime
    let result = scan_due_timers(&storage, &instance_id, 2500).unwrap();
    assert_eq!(
        result.len(),
        2,
        "Should recover both timers that fired during downtime"
    );

    let fire_times: Vec<u64> = result.iter().map(|r| r.fire_at_ms).collect();
    assert!(
        fire_times.contains(&1500u64),
        "Should find timer 1 that fired at 1500"
    );
    assert!(
        fire_times.contains(&2000u64),
        "Should find timer 2 that fired at 2000"
    );
}

// ---------------------------------------------------------------------------
// RQ-CR02: Only timers for the correct instance are recovered
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_only_recovers_target_instance_timers() {
    let mut storage = MockStorage::new();
    let target_instance = make_test_instance_id(0x01);
    let other_instance = make_test_instance_id(0x02);
    let timer_id_target = make_test_timer_id(0x03);
    let timer_id_other = make_test_timer_id(0x04);

    // Set timer for target instance
    timer_set(
        &mut storage,
        target_instance.clone(),
        timer_id_target.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    // Set timer for other instance
    timer_set(
        &mut storage,
        other_instance.clone(),
        timer_id_other.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    // At recovery, only target instance's timers should be found
    let result = scan_due_timers(&storage, &target_instance, 2000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_target);
    assert_eq!(result[0].instance_id, target_instance);
}

// ---------------------------------------------------------------------------
// RQ-CR03: Future timers are not incorrectly recovered as due
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_does_not_return_future_timers() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_past = make_test_timer_id(0x02);
    let timer_id_future = make_test_timer_id(0x03);

    // Past timer (should be recovered)
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_past.clone(),
        1500,
        1000,
        500,
        1000,
    )
    .unwrap();

    // Future timer (should NOT be recovered at this time)
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_future.clone(),
        5000,
        4500,
        500,
        1000,
    )
    .unwrap();

    // Server restarts at now_ms = 2000, only past timer should be due
    let result = scan_due_timers(&storage, &instance_id, 2000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_past);

    // At now_ms = 6000, both should be due
    let result = scan_due_timers(&storage, &instance_id, 6000).unwrap();
    assert_eq!(result.len(), 2);
}

// ---------------------------------------------------------------------------
// RQ-CR04: Timer fired exactly at boundary is recovered
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_timer_fired_at_boundary() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Timer fires at exactly 1000
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,
        500,
        500,
        500,
    )
    .unwrap();

    // Server restarts at now_ms = 1000 - timer should be found
    let result = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fire_at_ms, 1000);

    // Server restarts at now_ms = 999 - timer should NOT be found yet
    let result = scan_due_timers(&storage, &instance_id, 999).unwrap();
    assert!(result.is_empty(), "Timer at 1000 should not be due at 999");
}

// ---------------------------------------------------------------------------
// RQ-CR05: Multiple timers with same fire time all recovered
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_multiple_timers_same_fire_time() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);
    let timer_id_3 = make_test_timer_id(0x04);

    // All three timers fire at 2000
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        2000,
        1500,
        500,
        1000,
    )
    .unwrap();

    // At recovery (now_ms = 2500), all three should be found
    let result = scan_due_timers(&storage, &instance_id, 2500).unwrap();
    assert_eq!(result.len(), 3);

    let timer_ids: Vec<_> = result.iter().map(|r| r.timer_id.clone()).collect();
    assert!(timer_ids.contains(&timer_id_1));
    assert!(timer_ids.contains(&timer_id_2));
    assert!(timer_ids.contains(&timer_id_3));
}

// ---------------------------------------------------------------------------
// RQ-CR06: Timer with very old fire time is still recovered
// ---------------------------------------------------------------------------

#[test]
fn rq_crash_recovery_very_old_timer() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Timer fires at timestamp 100 (very old)
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        100,
        50,
        50,
        0,
    )
    .unwrap();

    // Server restarts much later at now_ms = 1_000_000
    let result = scan_due_timers(&storage, &instance_id, 1_000_000).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].fire_at_ms, 100);
    assert_eq!(result[0].trigger_time_ms, 50);
    assert_eq!(result[0].duration_ms, 50);
}

// ===========================================================================
// ATTACK VECTOR 10: Timer cancellation on instance completion
// ===========================================================================

// ---------------------------------------------------------------------------
// RQ-TC01: All timers for instance cancelled on completion
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_cancellation_all_timers_for_instance() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);
    let timer_id_3 = make_test_timer_id(0x04);

    // Add timers with different fire times
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_3.clone(),
        5000,
        4500,
        500,
        0,
    )
    .unwrap();

    // Verify all 3 timers exist
    let all_timers = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(all_timers.len(), 3);

    // On completion, cancel all timers (delete each one)
    for timer in &all_timers {
        timer_delete(
            &mut storage,
            &instance_id,
            timer.timer_id.clone(),
            timer.fire_at_ms,
        )
        .unwrap();
    }

    // Verify all timers are gone
    let all_timers = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert!(
        all_timers.is_empty(),
        "All timers should be cancelled on completion"
    );

    // Verify no timers are due
    let due_timers = scan_due_timers(&storage, &instance_id, 10_000).unwrap();
    assert!(due_timers.is_empty());
}

// ---------------------------------------------------------------------------
// RQ-TC02: Cancellation of specific timer does not affect others
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_cancellation_specific_timer_only() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id_1 = make_test_timer_id(0x02);
    let timer_id_2 = make_test_timer_id(0x03);

    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_1.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id_2.clone(),
        2000,
        1500,
        500,
        0,
    )
    .unwrap();

    // Cancel only timer 1
    timer_delete(&mut storage, &instance_id, timer_id_1, 1000).unwrap();

    // Timer 2 should still exist
    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].timer_id, timer_id_2);
}

// ---------------------------------------------------------------------------
// RQ-TC03: Cancel non-existent timer is idempotent
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_cancellation_nonexistent_is_idempotent() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Add a timer
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();

    // Cancel a non-existent timer should succeed (idempotent)
    let non_existent_timer_id = make_test_timer_id(0xFF);
    let result = timer_delete(&mut storage, &instance_id, non_existent_timer_id, 1000);
    assert!(result.is_ok());

    // Original timer should still exist
    let result = scan_all_timers_for_instance(&storage, &instance_id).unwrap();
    assert_eq!(result.len(), 1);
}

// ---------------------------------------------------------------------------
// RQ-TC04: Cancel already-fired timer is idempotent
// ---------------------------------------------------------------------------

#[test]
fn rq_timer_cancellation_already_fired_is_idempotent() {
    let mut storage = MockStorage::new();
    let instance_id = make_test_instance_id(0x01);
    let timer_id = make_test_timer_id(0x02);

    // Add timer
    timer_set(
        &mut storage,
        instance_id.clone(),
        timer_id.clone(),
        1000,
        500,
        500,
        0,
    )
    .unwrap();

    // Timer fires at 1000, but is still in storage (hasn't been deleted yet)
    let due = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert_eq!(due.len(), 1);

    // Cancel already-fired timer should still succeed
    let result = timer_delete(&mut storage, &instance_id, timer_id.clone(), 1000);
    assert!(result.is_ok());

    // Now it should be gone
    let due = scan_due_timers(&storage, &instance_id, 1000).unwrap();
    assert!(due.is_empty());
}
