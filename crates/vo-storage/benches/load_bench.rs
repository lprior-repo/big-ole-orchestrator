use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;
use vo_storage::dedupe_partition::{DedupeStore, FjallDedupeStore};
use vo_types::{DedupeKey, InstanceId};

fn create_test_keyspace() -> fjall::Keyspace {
    let dir = tempdir().unwrap();
    fjall::Config::new(dir.path()).open().unwrap()
}

fn sample_instance_id(thread_id: u8) -> InstanceId {
    InstanceId::from_bytes([thread_id; 16])
}

fn bench_high_concurrency_dedupe_admit(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_dedupe_high_concurrency");
    group.throughput(Throughput::Elements(1));

    for num_threads in [16, 32, 64, 128] {
        let keys_per_thread = 256u64;
        group.bench_function(
            format!("{}_threads_{}keys_each", num_threads, keys_per_thread),
            |b| {
                b.iter_batched(
                    || {
                        let keyspace = create_test_keyspace();
                        Arc::new(FjallDedupeStore::open(&keyspace).unwrap())
                    },
                    |store| {
                        let barrier = Arc::new(Barrier::new(num_threads));
                        let handles: Vec<_> = (0..num_threads)
                            .map(|t| {
                                let store = Arc::clone(&store);
                                let barrier = Arc::clone(&barrier);
                                let iid = sample_instance_id(t as u8);
                                thread::spawn(move || {
                                    barrier.wait();
                                    let mut admitted = 0u64;
                                    for k in 0..keys_per_thread {
                                        let key = DedupeKey::parse(&format!("load-admit-{t}-{k}"))
                                            .unwrap();
                                        if matches!(
                                            store.check_and_insert(&key, &iid, 60_000).unwrap(),
                                            vo_storage::dedupe_partition::AdmissionResult::Admitted
                                        ) {
                                            admitted += 1;
                                        }
                                    }
                                    admitted
                                })
                            })
                            .collect();
                        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                        black_box(total)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_contention_same_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_dedupe_contention");
    group.throughput(Throughput::Elements(1));

    for (num_threads, num_keys) in [(16, 16), (32, 32), (64, 64), (128, 128)] {
        group.bench_function(
            format!(
                "{}_threads_{}shared_keys",
                num_threads, num_keys
            ),
            |b| {
                b.iter_batched(
                    || {
                        let keyspace = create_test_keyspace();
                        let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
                        store
                    },
                    |store| {
                        let barrier = Arc::new(Barrier::new(num_threads));
                        let handles: Vec<_> = (0..num_threads)
                            .map(|t| {
                                let store = Arc::clone(&store);
                                let barrier = Arc::clone(&barrier);
                                let iid = sample_instance_id(t as u8);
                                thread::spawn(move || {
                                    barrier.wait();
                                    let mut admitted = 0u64;
                                    let mut dup = 0u64;
                                    for round in 0..10u64 {
                                        let key_idx = round as usize % num_keys;
                                        let key = DedupeKey::parse(&format!(
                                            "contention-{key_idx}"
                                        ))
                                        .unwrap();
                                        match store
                                            .check_and_insert(&key, &iid, 60_000)
                                            .unwrap()
                                        {
                                            vo_storage::dedupe_partition::AdmissionResult::Admitted => {
                                                admitted += 1
                                            }
                                            vo_storage::dedupe_partition::AdmissionResult::Duplicate {
                                                ..
                                            } => dup += 1,
                                        }
                                    }
                                    (admitted, dup)
                                })
                            })
                            .collect();
                        let results: Vec<(u64, u64)> =
                            handles.into_iter().map(|h| h.join().unwrap()).collect();
                        let total_admit: u64 = results.iter().map(|(a, _)| *a).sum();
                        let total_dup: u64 = results.iter().map(|(_, d)| *d).sum();
                        black_box((total_admit, total_dup))
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_mixed_admit_contains(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_dedupe_mixed_ops");
    group.throughput(Throughput::Elements(1));

    for num_threads in [16, 32, 64] {
        group.bench_function(format!("{}_threads_mixed", num_threads), |b| {
            b.iter_batched(
                || {
                    let keyspace = create_test_keyspace();
                    let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
                    for i in 0..1000u64 {
                        let key = DedupeKey::parse(&format!("mixed-seed-{i}")).unwrap();
                        let iid = InstanceId::from_bytes([0; 16]);
                        store.check_and_insert(&key, &iid, 60_000).unwrap();
                    }
                    store
                },
                |store| {
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let store = Arc::clone(&store);
                            let barrier = Arc::clone(&barrier);
                            thread::spawn(move || {
                                barrier.wait();
                                let mut ops = 0u64;
                                for i in 0..500u64 {
                                    if i % 3 == 0 {
                                        let key =
                                            DedupeKey::parse(&format!("mixed-seed-{i}")).unwrap();
                                        black_box(store.contains(&key).unwrap());
                                    } else {
                                        let key = DedupeKey::parse(&format!("mixed-new-{t}-{i}"))
                                            .unwrap();
                                        let iid = InstanceId::from_bytes([t as u8; 16]);
                                        black_box(
                                            store.check_and_insert(&key, &iid, 60_000).unwrap(),
                                        );
                                    }
                                    ops += 1;
                                }
                                ops
                            })
                        })
                        .collect();
                    let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                    black_box(total)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_sustained_write_amplification(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_dedupe_sustained");
    group.throughput(Throughput::Elements(1));

    group.bench_function("8_threads_10k_ops_each", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                Arc::new(FjallDedupeStore::open(&keyspace).unwrap())
            },
            |store| {
                let barrier = Arc::new(Barrier::new(8));
                let handles: Vec<_> = (0..8)
                    .map(|t| {
                        let store = Arc::clone(&store);
                        let barrier = Arc::clone(&barrier);
                        thread::spawn(move || {
                            barrier.wait();
                            let mut admitted = 0u64;
                            for k in 0..10_000u64 {
                                let key = DedupeKey::parse(&format!("sustained-{t}-{k}")).unwrap();
                                let iid = InstanceId::from_bytes([t as u8; 16]);
                                if matches!(
                                    store.check_and_insert(&key, &iid, 60_000).unwrap(),
                                    vo_storage::dedupe_partition::AdmissionResult::Admitted
                                ) {
                                    admitted += 1;
                                }
                            }
                            admitted
                        })
                    })
                    .collect();
                let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                black_box(total)
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_high_concurrency_dedupe_admit,
    bench_contention_same_keys,
    bench_mixed_admit_contains,
    bench_sustained_write_amplification,
);
criterion_main!(benches);
