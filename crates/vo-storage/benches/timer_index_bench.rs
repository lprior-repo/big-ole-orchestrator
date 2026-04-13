use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use tempfile::tempdir;
use vo_types::{InstanceId, TimerId};

use vo_storage::codec::StorageError;
use vo_storage::timer_index::{scan_due_timers, timer_delete, timer_set, Storage, TimerKey};

struct FjallStorage {
    keyspace: fjall::Keyspace,
}

impl FjallStorage {
    fn new(keyspace: fjall::Keyspace) -> Self {
        Self { keyspace }
    }

    fn partition(&self) -> fjall::Partition {
        self.keyspace
            .open_partition("timers", fjall::PartitionCreateOptions::default())
            .unwrap()
    }
}

impl Storage for FjallStorage {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.partition()
            .insert(key, value)
            .map_err(StorageError::from)
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        self.partition()
            .get(key)
            .map(|opt| opt.map(|v| v.to_vec()))
            .map_err(StorageError::from)
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.partition().remove(key).map_err(StorageError::from)
    }

    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, StorageError> {
        let partition = self.partition();
        let mut results = Vec::new();
        let mut cursor = partition.range(start..end);
        while let Some(item) = cursor.next() {
            let (k, v) = item.map_err(StorageError::from)?;
            results.push((k.to_vec(), v.to_vec()));
        }
        Ok(results)
    }
}

fn make_instance_id(index: u8) -> InstanceId {
    let mut bytes = [0x01u8; 16];
    bytes[0] = index;
    InstanceId::from_bytes(bytes)
}

fn make_timer_id(index: u8) -> TimerId {
    let mut bytes = [0x02u8; 16];
    bytes[0] = index;
    TimerId::from_bytes(bytes)
}

fn bench_timer_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_set");

    for &count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(&format!("insert_{count}_timers"), |b| {
            b.iter_batched(
                || {
                    let dir = tempfile::tempdir().unwrap();
                    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                    FjallStorage::new(keyspace)
                },
                |mut storage| {
                    let now_ms = 1_000_000u64;
                    for i in 0..count {
                        let instance_id = make_instance_id((i % 10) as u8);
                        let timer_id = make_timer_id(i as u8);
                        let fire_at_ms = now_ms + 1 + (i as u64 * 1000);
                        let trigger_time_ms = fire_at_ms - 100;
                        let duration_ms = 100;
                        black_box(timer_set(
                            &mut storage,
                            instance_id,
                            timer_id,
                            fire_at_ms,
                            trigger_time_ms,
                            duration_ms,
                            now_ms,
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

fn bench_scan_due_timers(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_scan_due");

    for &count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(&format!("scan_{count}_due_timers"), |b| {
            b.iter_batched(
                || {
                    let dir = tempfile::tempdir().unwrap();
                    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                    let mut storage = FjallStorage::new(keyspace);
                    let now_ms = 500_000u64;
                    for i in 0..count {
                        let instance_id = make_instance_id(0);
                        let timer_id = make_timer_id(i as u8);
                        let fire_at_ms = now_ms + 1000 + (i as u64);
                        let trigger_time_ms = fire_at_ms - 100;
                        let duration_ms = 100;
                        timer_set(
                            &mut storage,
                            instance_id,
                            timer_id,
                            fire_at_ms,
                            trigger_time_ms,
                            duration_ms,
                            now_ms,
                        )
                        .unwrap();
                    }
                    let scan_now_ms = now_ms + 1500;
                    (storage, make_instance_id(0), scan_now_ms)
                },
                |(storage, instance_id, scan_now_ms)| {
                    let timers =
                        black_box(scan_due_timers(&storage, &instance_id, scan_now_ms)).unwrap();
                    black_box(timers.len())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_timer_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_delete");

    group.throughput(Throughput::Elements(1));

    group.bench_function("delete_single_timer", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                let mut storage = FjallStorage::new(keyspace);
                let now_ms = 1_000_000u64;
                let instance_id = make_instance_id(0);
                let timer_id = make_timer_id(42);
                let fire_at_ms = now_ms + 1000;
                let trigger_time_ms = fire_at_ms - 100;
                let duration_ms = 100;
                timer_set(
                    &mut storage,
                    instance_id.clone(),
                    timer_id.clone(),
                    fire_at_ms,
                    trigger_time_ms,
                    duration_ms,
                    now_ms,
                )
                .unwrap();
                (storage, instance_id, timer_id, fire_at_ms)
            },
            |(mut storage, instance_id, timer_id, fire_at_ms)| {
                black_box(timer_delete(
                    &mut storage,
                    &instance_id,
                    timer_id,
                    fire_at_ms,
                ))
                .unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_timer_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_mixed");

    group.throughput(Throughput::Elements(100));

    group.bench_function("80_20_read_write_ratio", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                let mut storage = FjallStorage::new(keyspace);
                let now_ms = 1_000_000u64;
                for i in 0..1000u64 {
                    let instance_id = make_instance_id((i % 10) as u8);
                    let timer_id = make_timer_id(i as u8);
                    let fire_at_ms = now_ms + 1 + (i * 1000);
                    let trigger_time_ms = fire_at_ms - 100;
                    let duration_ms = 100;
                    timer_set(
                        &mut storage,
                        instance_id,
                        timer_id,
                        fire_at_ms,
                        trigger_time_ms,
                        duration_ms,
                        now_ms,
                    )
                    .unwrap();
                }
                (storage, now_ms)
            },
            |(mut storage, now_ms)| {
                for i in 0..100u64 {
                    if i % 5 < 4 {
                        let instance_id = make_instance_id((i % 10) as u8);
                        black_box(scan_due_timers(&storage, &instance_id, now_ms + i * 100))
                            .unwrap();
                    } else {
                        let instance_id = make_instance_id((i % 10) as u8);
                        let timer_id = make_timer_id((i + 1000) as u8);
                        let fire_at_ms = now_ms + ((i + 1000) * 1000);
                        let trigger_time_ms = fire_at_ms - 100;
                        let duration_ms = 100;
                        timer_set(
                            &mut storage,
                            instance_id,
                            timer_id,
                            fire_at_ms,
                            trigger_time_ms,
                            duration_ms,
                            now_ms,
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

fn bench_timer_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_concurrent");

    group.throughput(Throughput::Elements(512));

    group.bench_function("sequential_512_inserts", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                (dir, FjallStorage::new(keyspace))
            },
            |(_dir, mut storage)| {
                let now_ms = 1_000_000u64;
                for i in 0..512u64 {
                    let instance_id = make_instance_id((i % 10) as u8);
                    let timer_id = make_timer_id(i as u8);
                    let fire_at_ms = now_ms + 1 + (i * 1000);
                    let trigger_time_ms = fire_at_ms - 100;
                    let duration_ms = 100;
                    timer_set(
                        &mut storage,
                        instance_id,
                        timer_id,
                        fire_at_ms,
                        trigger_time_ms,
                        duration_ms,
                        now_ms,
                    )
                    .unwrap();
                }
                black_box(())
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_timer_key_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_key_encoding");
    group.throughput(Throughput::Elements(1));

    let instance_id = InstanceId::from_bytes([0x42; 16]);
    let timer_id = TimerId::from_bytes([0x43; 16]);
    let fire_at_ms = 1_000_000u64;

    group.bench_function("encode_timer_key", |b| {
        b.iter(|| {
            black_box(TimerKey::new(
                fire_at_ms,
                instance_id.clone(),
                timer_id.clone(),
            ))
            .unwrap();
        });
    });

    group.finish();
}

fn bench_timer_range_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_range_scan");

    group.throughput(Throughput::Elements(100));

    group.bench_function("scan_due_timers_with_varying_timespans", |b| {
        b.iter_batched(
            || {
                let dir = tempfile::tempdir().unwrap();
                let keyspace = fjall::Config::new(dir.path()).open().unwrap();
                let mut storage = FjallStorage::new(keyspace);
                let base_now_ms = 500_000u64;
                for i in 0..10000u64 {
                    let instance_id = make_instance_id(0);
                    let timer_id = make_timer_id(i as u8);
                    let fire_at_ms = base_now_ms + 1000 + i;
                    let trigger_time_ms = fire_at_ms - 100;
                    let duration_ms = 100;
                    timer_set(
                        &mut storage,
                        instance_id,
                        timer_id,
                        fire_at_ms,
                        trigger_time_ms,
                        duration_ms,
                        base_now_ms,
                    )
                    .unwrap();
                }
                let scan_now_ms = base_now_ms + 1500;
                (storage, make_instance_id(0), scan_now_ms)
            },
            |(storage, instance_id, scan_now_ms)| {
                let timers =
                    black_box(scan_due_timers(&storage, &instance_id, scan_now_ms)).unwrap();
                let total: u64 = timers.iter().map(|t| t.fire_at_ms).sum();
                black_box(total)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_timer_set,
    bench_scan_due_timers,
    bench_timer_delete,
    bench_timer_mixed_workload,
    bench_timer_concurrent,
    bench_timer_key_encoding,
    bench_timer_range_scan,
);
criterion_main!(benches);
