use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;
use vo_types::{InstanceId, InstanceStatus, TimestampMs};

use vo_storage::instance_index::{
    encode_instance_index_key, instance_index_upsert, scan_all_instances, scan_by_status,
    InstanceIndexEntry,
};

fn create_test_keyspace() -> fjall::Keyspace {
    let dir = tempdir().unwrap();
    fjall::Config::new(dir.path()).open().unwrap()
}

fn make_instance_id(index: u8) -> InstanceId {
    let mut bytes = [0x01u8; 16];
    bytes[0] = index;
    InstanceId::from_bytes(bytes)
}

fn make_timestamp(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn seed_instances(keyspace: &fjall::Keyspace, count: usize, status: InstanceStatus) {
    for i in 0..count {
        let id = make_instance_id(i as u8);
        let ts = make_timestamp(i as u64 * 1000);
        instance_index_upsert(keyspace, &id, status, ts, None).unwrap();
    }
}

fn collect_entries(
    iter: impl Iterator<Item = Result<InstanceIndexEntry, vo_storage::codec::StorageError>>,
) -> Vec<InstanceIndexEntry> {
    iter.map(|r| r.expect("expected Ok entry"))
        .collect::<Vec<_>>()
}

fn bench_instance_index_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_insert");

    for &count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(&format!("insert_{count}_entries"), |b| {
            b.iter_batched(
                || create_test_keyspace(),
                |keyspace| {
                    for i in 0..count {
                        let id = make_instance_id(i as u8);
                        let ts = make_timestamp(i as u64 * 1000);
                        black_box(instance_index_upsert(
                            &keyspace,
                            &id,
                            InstanceStatus::Running,
                            ts,
                            None,
                        ))
                        .unwrap();
                    }
                },
                criterion::BatchSize::NumIterations(1),
            );
        });

        group.bench_function(&format!("insert_{count}_entries_with_transition"), |b| {
            b.iter_batched(
                || create_test_keyspace(),
                |keyspace| {
                    for i in 0..count {
                        let id = make_instance_id(i as u8);
                        let ts = make_timestamp(i as u64 * 1000);
                        instance_index_upsert(&keyspace, &id, InstanceStatus::Pending, ts, None)
                            .unwrap();
                        black_box(instance_index_upsert(
                            &keyspace,
                            &id,
                            InstanceStatus::Running,
                            ts,
                            Some(InstanceStatus::Pending),
                        ))
                        .unwrap();
                    }
                },
                criterion::BatchSize::NumIterations(1),
            );
        });
    }

    group.finish();
}

fn bench_instance_index_scan_by_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_scan_by_status");

    for &count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(&format!("scan_{count}_entries"), |b| {
            b.iter_batched(
                || {
                    let keyspace = create_test_keyspace();
                    seed_instances(&keyspace, count, InstanceStatus::Running);
                    keyspace
                },
                |keyspace| {
                    let entries =
                        collect_entries(scan_by_status(&keyspace, InstanceStatus::Running));
                    black_box(entries.len())
                },
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function(&format!("scan_{count}_entries_prefix"), |b| {
            b.iter_batched(
                || {
                    let keyspace = create_test_keyspace();
                    seed_instances(&keyspace, count, InstanceStatus::Running);
                    keyspace
                },
                |keyspace| {
                    let partition = keyspace
                        .open_partition("instances", fjall::PartitionCreateOptions::default())
                        .unwrap();
                    let prefix = [InstanceStatus::Running.to_byte()];
                    let count = partition.prefix(prefix).count();
                    black_box(count)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_instance_index_scan_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_scan_all");

    for &count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(&format!("full_scan_{count}_entries"), |b| {
            b.iter_batched(
                || {
                    let keyspace = create_test_keyspace();
                    seed_instances(&keyspace, count / 3, InstanceStatus::Pending);
                    seed_instances(&keyspace, count / 3, InstanceStatus::Running);
                    seed_instances(&keyspace, count / 3, InstanceStatus::Completed);
                    keyspace
                },
                |keyspace| {
                    let entries = collect_entries(scan_all_instances(&keyspace));
                    black_box(entries.len())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_instance_index_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_mixed");

    group.throughput(Throughput::Elements(100));

    group.bench_function("80_20_read_write_ratio", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                seed_instances(&keyspace, 1000, InstanceStatus::Running);
                keyspace
            },
            |keyspace| {
                for i in 0..100u64 {
                    if i % 5 < 4 {
                        let entries =
                            collect_entries(scan_by_status(&keyspace, InstanceStatus::Running));
                        black_box(entries.len());
                    } else {
                        let id = make_instance_id((i % 200) as u8);
                        let ts = make_timestamp(i * 1000);
                        instance_index_upsert(
                            &keyspace,
                            &id,
                            InstanceStatus::Running,
                            ts,
                            Some(InstanceStatus::Running),
                        )
                        .unwrap();
                    }
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("50_50_read_write_ratio", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                seed_instances(&keyspace, 1000, InstanceStatus::Running);
                keyspace
            },
            |keyspace| {
                for i in 0..100u64 {
                    if i % 2 == 0 {
                        let entries =
                            collect_entries(scan_by_status(&keyspace, InstanceStatus::Running));
                        black_box(entries.len());
                    } else {
                        let id = make_instance_id((i % 200) as u8);
                        let ts = make_timestamp(i * 1000);
                        instance_index_upsert(
                            &keyspace,
                            &id,
                            InstanceStatus::Running,
                            ts,
                            Some(InstanceStatus::Running),
                        )
                        .unwrap();
                    }
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_instance_index_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_concurrent");

    group.throughput(Throughput::Elements(512));

    group.bench_function("8_threads_insert_distinct", |b| {
        b.iter_batched(
            || {
                let keyspace = Arc::new(create_test_keyspace());
                (0..8u64)
                    .map(|t| {
                        let ks = Arc::clone(&keyspace);
                        thread::spawn(move || {
                            for i in 0..64u64 {
                                let id = make_instance_id(((t * 100) + i) as u8);
                                let ts = make_timestamp(i * 1000 + t * 100000);
                                instance_index_upsert(&ks, &id, InstanceStatus::Running, ts, None)
                                    .unwrap();
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            },
            |handles| {
                let total: usize = handles
                    .into_iter()
                    .map(|h| {
                        h.join().unwrap();
                        64
                    })
                    .sum();
                black_box(total)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("8_threads_mixed_read_write", |b| {
        b.iter_batched(
            || {
                let keyspace = Arc::new(create_test_keyspace());
                for i in 0..1000u64 {
                    let id = make_instance_id(i as u8);
                    let ts = make_timestamp(i * 1000);
                    instance_index_upsert(&keyspace, &id, InstanceStatus::Running, ts, None)
                        .unwrap();
                }
                let ks = Arc::clone(&keyspace);
                (0..8u64)
                    .map(|t| {
                        let ks = Arc::clone(&ks);
                        thread::spawn(move || {
                            for i in 0..64u64 {
                                let idx = (t * 100 + i) % 1000;
                                if i % 2 == 0 {
                                    let entries = collect_entries(scan_by_status(
                                        &ks,
                                        InstanceStatus::Running,
                                    ));
                                    black_box(entries.len());
                                } else {
                                    let id = make_instance_id(idx as u8);
                                    let ts = make_timestamp(idx as u64 * 1000);
                                    instance_index_upsert(
                                        &ks,
                                        &id,
                                        InstanceStatus::Running,
                                        ts,
                                        Some(InstanceStatus::Running),
                                    )
                                    .unwrap();
                                }
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            },
            |handles| {
                handles.into_iter().for_each(|h| h.join().unwrap());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_instance_index_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_encoding");
    group.throughput(Throughput::Elements(1));

    let id = InstanceId::from_bytes([0x42; 16]);
    let ts = TimestampMs::try_from(1000u64).unwrap();

    group.bench_function("encode_key_all_statuses", |b| {
        b.iter(|| {
            for status in [
                InstanceStatus::Pending,
                InstanceStatus::Running,
                InstanceStatus::Paused,
                InstanceStatus::Completed,
                InstanceStatus::Failed,
                InstanceStatus::Cancelled,
            ] {
                black_box(encode_instance_index_key(status, ts, &id)).unwrap();
            }
        });
    });

    group.finish();
}

fn bench_instance_index_range_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_index_range_scan");

    group.throughput(Throughput::Elements(100));

    group.bench_function("scan_by_status_with_limit", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                for i in 0..10000u64 {
                    let id = make_instance_id((i % 256) as u8);
                    let ts = make_timestamp(i * 1000);
                    let status = match i % 3 {
                        0 => InstanceStatus::Pending,
                        1 => InstanceStatus::Running,
                        _ => InstanceStatus::Completed,
                    };
                    instance_index_upsert(&keyspace, &id, status, ts, None).unwrap();
                }
                keyspace
            },
            |keyspace| {
                let entries = collect_entries(scan_by_status(&keyspace, InstanceStatus::Running));
                let total: u64 = entries.iter().map(|e| e.created_at.as_u64()).sum();
                black_box(total)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_instance_index_insert,
    bench_instance_index_scan_by_status,
    bench_instance_index_scan_all,
    bench_instance_index_mixed_workload,
    bench_instance_index_concurrent,
    bench_instance_index_encoding,
    bench_instance_index_range_scan,
);
criterion_main!(benches);
