#![allow(clippy::unwrap_used)]

use vo_storage::instance_index::{
    encode_instance_index_key, instance_index_upsert, scan_all_instances, scan_by_status,
};

use super::helpers::*;

#[test]
fn rq_phantom_entries_detectable_via_scan_count() {
    let (_dir, database) = make_test_keyspace();
    let partition = database
        .keyspace("instances", || fjall::KeyspaceCreateOptions::default())
        .unwrap();

    let id = make_test_instance_id(0x42);
    let ts = make_test_timestamp(1000);

    let key_pending = encode_instance_index_key(InstanceStatus::Pending, ts, &id).unwrap();
    let key_running = encode_instance_index_key(InstanceStatus::Running, ts, &id).unwrap();

    partition.insert(key_pending, &[] as &[u8]).unwrap();
    partition.insert(key_running, &[] as &[u8]).unwrap();

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 2, "Should detect 2 phantom entries");

    let ids: Vec<_> = all.iter().map(|e| &e.instance_id).collect();
    assert_eq!(ids[0], ids[1], "Both entries should have the same instance_id");
    assert_ne!(all[0].status, all[1].status, "But different statuses (phantom)");
}

#[test]
fn rq_1000_instances_across_all_statuses_scan_returns_correct_counts() {
    let (_dir, database) = make_test_keyspace();
    let statuses = InstanceStatus::all_variants();

    (0u16..1000).for_each(|i| {
        let id = make_unique_instance_id(i);
        let status = statuses[(i as usize) % statuses.len()];
        let ts = make_test_timestamp(u64::from(i));
        seed_instance(&database, &id, status, ts);
    });

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), 1000, "Total should be 1000");

    let mut per_status_total = 0usize;
    (statuses.iter().enumerate())
        .into_iter()
        .for_each(|(idx, status)| {
            let entries = collect_scan_ok(scan_by_status(&database, *status));
            let expected = if idx < 4 { 167 } else { 166 };
            assert_eq!(
                entries.len(),
                expected,
                "Status {:?} should have {expected} entries, found {}",
                status,
                entries.len()
            );
            per_status_total += entries.len();

            entries.iter().for_each(|entry| {
                assert_eq!(entry.status, *status);
            });

            entries.windows(2).for_each(|pair| {
                assert!(
                    pair[0].created_at.as_u64() <= pair[1].created_at.as_u64(),
                    "Within {:?}: entries not ordered by created_at",
                    status
                );
            });
        });

    assert_eq!(per_status_total, 1000, "Sum of per-status counts should be 1000");
}

#[test]
fn rq_scan_all_returns_globally_ordered_by_status_byte_then_created_at() {
    let (_dir, database) = make_test_keyspace();

    let data: &[(u16, InstanceStatus, u64)] = &[
        (10, InstanceStatus::Cancelled, 999),
        (1, InstanceStatus::Pending, 100),
        (5, InstanceStatus::Running, 50),
        (2, InstanceStatus::Pending, 200),
        (6, InstanceStatus::Paused, 300),
        (3, InstanceStatus::Running, 10),
        (9, InstanceStatus::Failed, 400),
        (7, InstanceStatus::Completed, 500),
        (4, InstanceStatus::Running, 75),
        (8, InstanceStatus::Completed, 250),
    ];

    (data).into_iter().for_each(|(idx, status, ts)| {
        let id = make_unique_instance_id(*idx);
        seed_instance(&database, &id, *status, make_test_timestamp(*ts));
    });

    let all = collect_scan_ok(scan_all_instances(&database));
    assert_eq!(all.len(), data.len());

    all.windows(2).for_each(|pair| {
        let a_status = pair[0].status.to_byte();
        let b_status = pair[1].status.to_byte();
        let a_ts = pair[0].created_at.as_u64();
        let b_ts = pair[1].created_at.as_u64();

        assert!(
            (a_status, a_ts) <= (b_status, b_ts),
            "Global ordering violated: ({a_status:#04x}, {a_ts}) > ({b_status:#04x}, {b_ts})"
        );
    });
}

#[test]
fn rq_rapid_transitions_on_50_instances_leave_exactly_50_keys() {
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

#[test]
fn rq_interleaved_transitions_do_not_cross_contaminate_instances() {
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