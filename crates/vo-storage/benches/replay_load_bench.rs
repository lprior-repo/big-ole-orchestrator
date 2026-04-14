use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;
use vo_storage::query::replay_events;
use vo_types::InstanceId;

fn create_populated_keyspace(num_instances: usize, events_per_instance: u64) -> fjall::Keyspace {
    let dir = tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let partition = keyspace
        .open_partition("events", fjall::PartitionCreateOptions::default())
        .unwrap();

    for inst_idx in 0..num_instances {
        let instance_id = InstanceId::from_bytes([inst_idx as u8; 16]);
        let prefix = format!("{}", instance_id);
        for seq in 1..=events_per_instance {
            let mut key = prefix.as_bytes().to_vec();
            key.extend_from_slice(&seq.to_be_bytes());
            let envelope = serde_json::json!({
                "version": 1,
                "instance_id": prefix,
                "sequence": seq,
                "timestamp_ms": 1_715_000_000_000u64 + seq,
                "payload": {
                    "type": "StepCompleted",
                    "step_id": format!("step-{seq}"),
                    "output": format!("result-{seq}")
                }
            });
            let value = serde_json::to_vec(&envelope).unwrap();
            partition.insert(&key, &value).unwrap();
        }
    }

    keyspace
}

fn bench_replay_single_instance(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_single");
    group.throughput(Throughput::Elements(1));

    for events_per_instance in [10, 100, 1000] {
        group.bench_function(format!("{} events", events_per_instance), |b| {
            let keyspace = create_populated_keyspace(1, events_per_instance);
            let instance_id = InstanceId::from_bytes([0u8; 16]);
            b.iter(|| {
                let iter = replay_events(&keyspace, &instance_id);
                let count = iter.filter_map(|r| r.ok()).count();
                black_box(count)
            })
        });
    }
    group.finish();
}

fn bench_concurrent_replay_distinct_instances(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_concurrent_distinct");
    group.throughput(Throughput::Elements(1));

    for (num_threads, events_per) in [(4, 100), (8, 100), (16, 100), (32, 50)] {
        group.bench_function(
            format!("{}_threads_{}events_each", num_threads, events_per),
            |b| {
                b.iter_batched(
                    || Arc::new(create_populated_keyspace(num_threads, events_per)),
                    |keyspace| {
                        let barrier = Arc::new(Barrier::new(num_threads));
                        let handles: Vec<_> = (0..num_threads)
                            .map(|t| {
                                let ks = Arc::clone(&keyspace);
                                let barrier = Arc::clone(&barrier);
                                thread::spawn(move || {
                                    barrier.wait();
                                    let instance_id = InstanceId::from_bytes([t as u8; 16]);
                                    let iter = replay_events(&ks, &instance_id);
                                    let count = iter.filter_map(|r| r.ok()).count();
                                    count
                                })
                            })
                            .collect();
                        let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
                        black_box(total)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_concurrent_replay_same_instance(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_contention");
    group.throughput(Throughput::Elements(1));

    for (num_threads, events_per) in [(4, 100), (8, 100), (16, 100)] {
        group.bench_function(
            format!("{}_threads_same_instance_{}events", num_threads, events_per),
            |b| {
                b.iter_batched(
                    || Arc::new(create_populated_keyspace(1, events_per)),
                    |keyspace| {
                        let barrier = Arc::new(Barrier::new(num_threads));
                        let instance_id = InstanceId::from_bytes([0u8; 16]);
                        let handles: Vec<_> = (0..num_threads)
                            .map(|_| {
                                let ks = Arc::clone(&keyspace);
                                let barrier = Arc::clone(&barrier);
                                let iid = instance_id.clone();
                                thread::spawn(move || {
                                    barrier.wait();
                                    let iter = replay_events(&ks, &iid);
                                    let count = iter.filter_map(|r| r.ok()).count();
                                    count
                                })
                            })
                            .collect();
                        let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
                        black_box(total)
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_replay_empty_instance(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_empty");

    group.bench_function("no_events", |b| {
        let keyspace = create_populated_keyspace(0, 0);
        let instance_id = InstanceId::from_bytes([0u8; 16]);
        b.iter(|| {
            let iter = replay_events(&keyspace, &instance_id);
            let count = iter.filter_map(|r| r.ok()).count();
            black_box(count)
        })
    });
    group.finish();
}

fn bench_large_event_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_large_history");
    group.throughput(Throughput::Elements(1));

    for events_per in [1000, 5000, 10_000] {
        group.bench_function(format!("{} events", events_per), |b| {
            let keyspace = create_populated_keyspace(1, events_per);
            let instance_id = InstanceId::from_bytes([0u8; 16]);
            b.iter(|| {
                let iter = replay_events(&keyspace, &instance_id);
                let mut count = 0usize;
                for result in iter {
                    if result.is_ok() {
                        count += 1;
                    } else {
                        break;
                    }
                }
                black_box(count)
            })
        });
    }
    group.finish();
}

fn bench_mixed_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_replay_mixed_rw");
    group.throughput(Throughput::Elements(1));

    group.bench_function("4_readers_4_writers", |b| {
        b.iter_batched(
            || {
                let ks = Arc::new(create_populated_keyspace(4, 100));
                let partition = ks
                    .open_partition("events", fjall::PartitionCreateOptions::default())
                    .unwrap();
                (ks, Arc::new(partition))
            },
            |(keyspace, partition)| {
                let barrier = Arc::new(Barrier::new(8));
                let mut handles: Vec<thread::JoinHandle<usize>> = Vec::new();

                for t in 0..4 {
                    let ks = Arc::clone(&keyspace);
                    let barrier = Arc::clone(&barrier);
                    handles.push(thread::spawn(move || {
                        barrier.wait();
                        let instance_id = InstanceId::from_bytes([t as u8; 16]);
                        let iter = replay_events(&ks, &instance_id);
                        iter.filter_map(|r| r.ok()).count()
                    }));
                }

                for t in 0..4 {
                    let part = Arc::clone(&partition);
                    let barrier = Arc::clone(&barrier);
                    handles.push(thread::spawn(move || {
                        barrier.wait();
                        let mut written = 0usize;
                        for i in 0..50u64 {
                            let instance_id = InstanceId::from_bytes([t as u8; 16]);
                            let prefix = format!("{}", instance_id);
                            let seq = 1000 + i;
                            let mut key = prefix.as_bytes().to_vec();
                            key.extend_from_slice(&seq.to_be_bytes());
                            let envelope = serde_json::json!({
                                "version": 1,
                                "instance_id": prefix,
                                "sequence": seq,
                                "timestamp_ms": 1_715_000_000_000u64,
                                "payload": {"type": "NewEvent"}
                            });
                            let value = serde_json::to_vec(&envelope).unwrap();
                            part.insert(&key, &value).unwrap();
                            written += 1;
                        }
                        written
                    }));
                }

                let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
                black_box(results)
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_replay_single_instance,
    bench_concurrent_replay_distinct_instances,
    bench_concurrent_replay_same_instance,
    bench_replay_empty_instance,
    bench_large_event_history,
    bench_mixed_read_write,
);
criterion_main!(benches);
