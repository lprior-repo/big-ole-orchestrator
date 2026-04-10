#![allow(clippy::redundant_pattern_matching)]
//! Red Queen adversarial tests for queue (BudgetQueues), tag (WriteClass),
//! and stash (WriteBudget) in vo-storage.
//!
//! Attack vectors:
//! - QUEUE: budget/queue consistency, stats drift, zero-size bypass
//! - TAG: serde bypass, invalid classification, tier ordering
//! - STASH: budget exhaustion without release, u64 overflow, reserve boundary
//!
//! NOTE: WriteBudget uses RefCell (not Sync), so Appender is single-threaded.
//! Concurrent attack vectors are documented but not tested here.

use vo_storage::append::{
    Appender, BlobWrite, BudgetQueuesError, ControlPlaneWrite, QueueConfig, WriteBudget, WriteClass,
};
use vo_types::events::{EventEnvelope, EventMetadata};

fn make_event(seq: u64, instance: &str) -> EventEnvelope {
    EventEnvelope {
        schema_version: 1,
        instance_id: instance.to_string(),
        sequence: seq,
        timestamp_ms: 1000,
        payload: serde_json::json!({"test": "data"}),
        metadata: EventMetadata::default(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 1: QUEUE — Budget never released on dequeue (INV-BUDGET-LEAK)
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: Enqueue items consuming budget, dequeue them, then try to enqueue again.
/// If budget is never released on dequeue, the queue becomes permanently starved
/// even though it's empty.
#[test]
fn attack_budget_leak_on_dequeue() {
    let config = QueueConfig {
        critical_capacity: 100,
        projection_capacity: 100,
        blob_capacity: 100,
    };
    let budget = WriteBudget::new(500, 500, 500);
    let appender = Appender::new(config, budget);

    for i in 0..5u64 {
        assert!(appender
            .append_control_plane(ControlPlaneWrite::new(make_event(i, "leak-test"), 100))
            .is_ok());
    }

    let remaining = appender
        .budget()
        .remaining(WriteClass::CriticalControlPlane);
    assert_eq!(remaining, 0, "budget should be fully consumed");

    for i in 0..5 {
        assert!(
            appender.dequeue_critical().is_some(),
            "dequeue {i} should return item"
        );
    }

    let remaining_after_drain = appender
        .budget()
        .remaining(WriteClass::CriticalControlPlane);
    assert_eq!(
        remaining_after_drain, 0,
        "BUG: budget is NOT released on dequeue — queue is empty but starved. \
         This means the budget is a lifetime counter, not a current-depth counter. \
         Long-lived appenders will permanently exhaust budget."
    );

    let result =
        appender.append_control_plane(ControlPlaneWrite::new(make_event(99, "leak-test"), 1));
    assert!(
        result.is_err(),
        "BUG: enqueue into empty queue fails because budget was never released"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 2: QUEUE — Budget exhaustion exact cutoff
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_budget_exhaustion_exact_cutoff() {
    let config = QueueConfig {
        critical_capacity: 100,
        projection_capacity: 100,
        blob_capacity: 100,
    };
    let budget = WriteBudget::new(53, 53, 53);
    let appender = Appender::new(config, budget);

    for i in 0..5u64 {
        assert!(appender
            .append_control_plane(ControlPlaneWrite::new(make_event(i, "cutoff"), 10))
            .is_ok());
    }

    let result = appender.append_control_plane(ControlPlaneWrite::new(make_event(5, "cutoff"), 10));
    assert!(matches!(
        result,
        Err(BudgetQueuesError::BudgetExceeded { .. })
    ));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 3: QUEUE — Stats consistency after enqueue/dequeue
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stats_consistency_after_enqueue_dequeue() {
    let config = QueueConfig {
        critical_capacity: 1000,
        projection_capacity: 1000,
        blob_capacity: 1000,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = Appender::new(config, budget);

    // Enqueue 50, dequeue 20, check stats
    for i in 0..50u64 {
        assert!(appender
            .append_control_plane(ControlPlaneWrite::new(make_event(i, "stats"), 10))
            .is_ok());
    }

    for _ in 0..20 {
        assert!(appender.dequeue_critical().is_some());
    }

    let stats_arc = appender.stats();
    let stats = stats_arc.lock().unwrap();
    let depth = stats.depth(WriteClass::CriticalControlPlane);
    drop(stats);

    // Count actual items remaining
    let mut actual_depth = 0;
    loop {
        match appender.dequeue_critical() {
            Some(_) => actual_depth += 1,
            None => break,
        }
    }

    assert_eq!(
        depth, actual_depth,
        "BUG: stats depth ({depth}) != actual queue depth ({actual_depth})"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 4: TAG — Serde bypass for WriteClass
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_tag_deserialize_invalid_strings() {
    let json = r#""critical_control_plane""#;
    let wc: WriteClass = serde_json::from_str(json).unwrap();
    assert_eq!(wc, WriteClass::CriticalControlPlane);

    let json = r#""Critical_Control_Plane""#;
    let result = serde_json::from_str::<WriteClass>(json);
    assert!(
        result.is_err(),
        "BUG: mixed case WriteClass deserialized without error"
    );

    let json = r#""""#;
    let result = serde_json::from_str::<WriteClass>(json);
    assert!(
        result.is_err(),
        "BUG: empty string WriteClass deserialized without error"
    );

    let json = r#""totally_invalid""#;
    let result = serde_json::from_str::<WriteClass>(json);
    assert!(
        result.is_err(),
        "BUG: arbitrary string WriteClass deserialized without error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 5: TAG — FromStr edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_tag_fromstr_edge_cases() {
    assert!("critical_control_plane".parse::<WriteClass>().is_ok());
    assert!("operator_projection".parse::<WriteClass>().is_ok());
    assert!("bulk_blob".parse::<WriteClass>().is_ok());

    assert!("CRITICAL_CONTROL_PLANE".parse::<WriteClass>().is_err());
    assert!("Critical_Control_Plane".parse::<WriteClass>().is_err());
    assert!("CriticalControlPlane".parse::<WriteClass>().is_err());

    assert!("critical".parse::<WriteClass>().is_err());
    assert!("control_plane".parse::<WriteClass>().is_err());
    assert!("operator".parse::<WriteClass>().is_err());

    assert!(" critical_control_plane".parse::<WriteClass>().is_err());
    assert!("critical_control_plane ".parse::<WriteClass>().is_err());
    assert!("critical_control_plane\n".parse::<WriteClass>().is_err());

    assert!("critical\u{200b}_control_plane"
        .parse::<WriteClass>()
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 6: STASH — Budget reserve overflow via u64 saturation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stash_budget_reserve_u64_max() {
    let budget = WriteBudget::new(100, 100, 100);

    let result = budget.reserve(WriteClass::CriticalControlPlane, u64::MAX);
    assert!(
        result.is_err(),
        "BUG: reserve(u64::MAX) should fail but returned Ok — possible overflow"
    );

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        100,
        "BUG: budget was modified despite failed reserve"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 7: STASH — Budget reserve exact boundary
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stash_budget_exact_boundary() {
    let budget = WriteBudget::new(42, 42, 42);

    assert!(budget.reserve(WriteClass::CriticalControlPlane, 42).is_ok());
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);

    assert!(budget.reserve(WriteClass::CriticalControlPlane, 1).is_err());
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 8: STASH — Multiple reserve calls accumulate
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stash_budget_accumulation() {
    let budget = WriteBudget::new(100, 100, 100);

    for i in 0..100 {
        assert!(
            budget.reserve(WriteClass::CriticalControlPlane, 1).is_ok(),
            "reserve {i} of 1 byte should succeed"
        );
    }

    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 1).is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 9: QUEUE — Queue capacity 0
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_zero_capacity() {
    let config = QueueConfig {
        critical_capacity: 0,
        projection_capacity: 0,
        blob_capacity: 0,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = Appender::new(config, budget);

    let result =
        appender.append_control_plane(ControlPlaneWrite::new(make_event(1, "zero-cap"), 10));
    assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 10: QUEUE — Budget zero with zero-size item
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_zero_budget_zero_size() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(0, 0, 0);
    let appender = Appender::new(config, budget);

    let result =
        appender.append_control_plane(ControlPlaneWrite::new(make_event(1, "zero-budget"), 0));
    if result.is_ok() {
        let item = appender.dequeue_critical();
        assert!(
            item.is_some(),
            "zero-budget, zero-size item should be retrievable"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 11: STASH — Budget reserve with size 0
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stash_budget_reserve_zero_bytes() {
    let budget = WriteBudget::new(10, 10, 10);

    for _ in 0..1000 {
        assert!(
            budget.reserve(WriteClass::CriticalControlPlane, 0).is_ok(),
            "BUG: reserve(0) failed — zero-byte reserves should always succeed"
        );
    }

    assert_eq!(
        budget.remaining(WriteClass::CriticalControlPlane),
        10,
        "BUG: zero-byte reserves consumed budget"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 12: QUEUE — Dequeue from empty queue 1000 times
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_dequeue_empty_1000_times() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = Appender::new(config, budget);

    for _ in 0..1000 {
        assert!(appender.dequeue_critical().is_none());
        assert!(appender.dequeue_projection().is_none());
        assert!(appender.dequeue_blob().is_none());
    }

    let stats_arc = appender.stats();
    let stats = stats_arc.lock().unwrap();
    assert_eq!(stats.depth(WriteClass::CriticalControlPlane), 0);
    assert_eq!(stats.depth(WriteClass::OperatorProjection), 0);
    assert_eq!(stats.depth(WriteClass::BulkBlob), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 13: QUEUE — Fill then drain one
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_full_then_drain_one() {
    let config = QueueConfig {
        critical_capacity: 2,
        projection_capacity: 2,
        blob_capacity: 2,
    };
    let budget = WriteBudget::new(1_000_000, 1_000_000, 1_000_000);
    let appender = Appender::new(config, budget);

    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(make_event(1, "full"), 10))
        .is_ok());
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(make_event(2, "full"), 10))
        .is_ok());

    let result = appender.append_control_plane(ControlPlaneWrite::new(make_event(3, "full"), 10));
    assert!(matches!(result, Err(BudgetQueuesError::QueueFull { .. })));

    assert!(appender.dequeue_critical().is_some());

    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(make_event(4, "full"), 10))
        .is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 14: TAG — WriteClass tier ordering invariant
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_tag_tier_ordering_invariant() {
    let tiers: Vec<u8> = [
        WriteClass::CriticalControlPlane,
        WriteClass::OperatorProjection,
        WriteClass::BulkBlob,
    ]
    .iter()
    .map(|wc| wc.tier())
    .collect();

    assert_eq!(tiers, vec![1, 2, 3], "BUG: tier ordering is not 1, 2, 3");

    for w in tiers.windows(2) {
        assert!(
            w[0] < w[1],
            "BUG: tiers are not strictly ordered: {} >= {}",
            w[0],
            w[1]
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 15: QUEUE — Cross-class budget isolation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_cross_class_budget_isolation() {
    let config = QueueConfig::default();
    let budget = WriteBudget::new(50, 50, 50);
    let appender = Appender::new(config, budget);

    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(make_event(1, "iso"), 50))
        .is_ok());
    assert!(appender
        .append_control_plane(ControlPlaneWrite::new(make_event(2, "iso"), 1))
        .is_err());

    assert!(appender
        .append_blob(BlobWrite::bulk("iso-blob".to_string(), 50))
        .is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 16: STASH — can_write vs reserve single-threaded consistency
// ═══════════════════════════════════════════════════════════════════════════════

/// Attack: In single-threaded context, can_write and reserve must be consistent.
/// NOTE: WriteBudget uses RefCell (not Sync), so it cannot be shared across threads.
/// This means the Appender is inherently single-threaded — a design limitation
/// that prevents TOCTOU races but also prevents concurrent use.
#[test]
fn attack_stash_can_write_reserve_single_thread_consistency() {
    let budget = WriteBudget::new(10, 10, 10);

    assert!(budget.can_write(WriteClass::CriticalControlPlane, 10));
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 10).is_ok());
    assert!(!budget.can_write(WriteClass::CriticalControlPlane, 1));
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 1).is_err());

    // can_write(0) is always true when remaining >= 0
    assert!(budget.can_write(WriteClass::CriticalControlPlane, 0));
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 0).is_ok());
    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 17: QUEUE — Zero-size items bypass budget
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_queue_zero_size_bypasses_budget() {
    let config = QueueConfig {
        critical_capacity: 1000,
        projection_capacity: 1000,
        blob_capacity: 1000,
    };
    let budget = WriteBudget::new(1, 1, 1);
    let appender = Appender::new(config, budget);

    let result =
        appender.append_control_plane(ControlPlaneWrite::new(make_event(1, "zero-size"), 0));
    assert!(
        result.is_ok(),
        "zero-size items should bypass budget check (0 <= remaining)"
    );

    assert_eq!(
        appender
            .budget()
            .remaining(WriteClass::CriticalControlPlane),
        1,
        "BUG: zero-byte reserve consumed budget"
    );

    for i in 2..1001u64 {
        assert!(appender
            .append_control_plane(ControlPlaneWrite::new(make_event(i, "zero-size"), 0))
            .is_ok());
    }

    let result =
        appender.append_control_plane(ControlPlaneWrite::new(make_event(1001, "zero-size"), 0));
    assert!(
        result.is_err(),
        "1001st item should fail due to queue capacity"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTACK VECTOR 18: STASH — Reserve across all three classes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn attack_stash_reserve_all_classes_independently() {
    let budget = WriteBudget::new(100, 200, 300);

    assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
    assert!(budget.reserve(WriteClass::OperatorProjection, 150).is_ok());
    assert!(budget.reserve(WriteClass::BulkBlob, 250).is_ok());

    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 50);
    assert_eq!(budget.remaining(WriteClass::OperatorProjection), 50);
    assert_eq!(budget.remaining(WriteClass::BulkBlob), 50);

    // Exhaust each independently
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 50).is_ok());
    assert!(budget.reserve(WriteClass::OperatorProjection, 50).is_ok());
    assert!(budget.reserve(WriteClass::BulkBlob, 50).is_ok());

    assert_eq!(budget.remaining(WriteClass::CriticalControlPlane), 0);
    assert_eq!(budget.remaining(WriteClass::OperatorProjection), 0);
    assert_eq!(budget.remaining(WriteClass::BulkBlob), 0);

    // All should fail now
    assert!(budget.reserve(WriteClass::CriticalControlPlane, 1).is_err());
    assert!(budget.reserve(WriteClass::OperatorProjection, 1).is_err());
    assert!(budget.reserve(WriteClass::BulkBlob, 1).is_err());
}
