//! Red Queen tests: Rapid and interleaved transitions across multiple instances.

use vo_storage::instance_index::{scan_by_status, InstanceStatus};

use crate::instance_index_red_queen::helpers::*;

// ---------------------------------------------------------------------------
// RQ-IV04: Rapid transitions on many instances — no orphaned keys
// ---------------------------------------------------------------------------

#[test]
fn rq_rapid_transitions_on_50_instances_leave_exactly_50_keys() {
    use vo_storage::instance_index::instance_index_upsert;

    let (_dir, database) = make_test_keyspace();

    (0u16..50).for_each(|i| {
        let id = make_unique_instance_id(i);
        seed_instance(
            &database,
            &id,
            InstanceStatus::Pending,
            make_test_timestamp(u64::from(i)),
        );
    });

    (0u16..50).for_each(|i| {
        let id = make_unique_instance_id(i);
        let ts = make_test_timestamp(u64::from(i));

        instance_index_upsert(
            &database,
            &id,
            InstanceStatus::Running,
            ts,
            Some(InstanceStatus::Pending),
        )
        .unwrap();
        instance_index_upsert(
            &database,
            &id,
            InstanceStatus::Completed,
            ts,
            Some(InstanceStatus::Running),
        )
        .unwrap();
    });

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(
        all.len(),
        50,
        "After rapid transitions, exactly 50 keys should exist"
    );

    all.iter().for_each(|entry| {
        assert_eq!(entry.status, InstanceStatus::Completed);
    });

    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        0
    );
}

// ---------------------------------------------------------------------------
// RQ-IV05: Interleaved transitions across instances — no cross-contamination
// ---------------------------------------------------------------------------

#[test]
fn rq_interleaved_transitions_do_not_cross_contaminate_instances() {
    use vo_storage::instance_index::instance_index_upsert;

    let (_dir, database) = make_test_keyspace();

    let id_a = make_unique_instance_id(1);
    let id_b = make_unique_instance_id(2);
    let ts_a = make_test_timestamp(100);
    let ts_b = make_test_timestamp(200);

    seed_instance(&database, &id_a, InstanceStatus::Pending, ts_a);
    seed_instance(&database, &id_b, InstanceStatus::Pending, ts_b);

    instance_index_upsert(
        &database,
        &id_a,
        InstanceStatus::Running,
        ts_a,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &database,
        &id_b,
        InstanceStatus::Failed,
        ts_b,
        Some(InstanceStatus::Pending),
    )
    .unwrap();
    instance_index_upsert(
        &database,
        &id_a,
        InstanceStatus::Completed,
        ts_a,
        Some(InstanceStatus::Running),
    )
    .unwrap();

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 2);

    let completed = collect_scan_ok(scan_by_status(&database, InstanceStatus::Completed));
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].instance_id, id_a);

    let failed = collect_scan_ok(scan_by_status(&database, InstanceStatus::Failed));
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].instance_id, id_b);

    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Pending)).len(),
        0
    );
    assert_eq!(
        collect_scan_ok(scan_by_status(&database, InstanceStatus::Running)).len(),
        0
    );
}
