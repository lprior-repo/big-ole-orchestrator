use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;
use vo_types::DedupeKey;

use vo_storage::dedupe_partition::{DedupeStore, FjallDedupeStore};

fn create_test_keyspace() -> fjall::Keyspace {
    let dir = tempdir().unwrap();
    fjall::Config::new(dir.path()).open().unwrap()
}

fn sample_instance_id() -> vo_types::InstanceId {
    vo_types::InstanceId::from_bytes([1u8; 16])
}

fn bench_check_and_insert_admit(c: &mut Criterion) {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();

    let mut group = c.benchmark_group("dedupe_check_and_insert");
    group.throughput(Throughput::Elements(1));

    group.bench_function("admit_new_key", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let key = DedupeKey::parse(&format!("bench-admit-{counter}")).unwrap();
            black_box(store.check_and_insert(&key, &sample_instance_id(), 60_000))
        })
    });

    group.bench_function("reject_duplicate_key", |b| {
        let key = DedupeKey::parse("bench-dup-key").unwrap();
        store
            .check_and_insert(&key, &sample_instance_id(), 60_000)
            .unwrap();
        b.iter(|| black_box(store.check_and_insert(&key, &sample_instance_id(), 60_000)))
    });

    group.finish();
}

fn bench_contains(c: &mut Criterion) {
    let keyspace = create_test_keyspace();
    let store = FjallDedupeStore::open(&keyspace).unwrap();
    let key = DedupeKey::parse("bench-contains-key").unwrap();
    store
        .check_and_insert(&key, &sample_instance_id(), 60_000)
        .unwrap();

    let mut group = c.benchmark_group("dedupe_contains");
    group.throughput(Throughput::Elements(1));

    group.bench_function("hit", |b| b.iter(|| black_box(store.contains(&key))));

    let missing = DedupeKey::parse("bench-missing-key").unwrap();
    group.bench_function("miss", |b| b.iter(|| black_box(store.contains(&missing))));

    group.finish();
}

fn bench_purge_expired(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedupe_purge");

    group.bench_function("purge_100_entries", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                let store = FjallDedupeStore::open(&keyspace).unwrap();
                for i in 0..100u64 {
                    let key = DedupeKey::parse(&format!("bench-purge-{i}")).unwrap();
                    store
                        .check_and_insert(&key, &sample_instance_id(), 1)
                        .unwrap();
                }
                store
            },
            |store| black_box(store.purge_expired(u64::MAX)),
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_concurrent_check_and_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedupe_concurrent");

    group.bench_function("8_threads_distinct_keys", |b| {
        b.iter_batched(
            || {
                let keyspace = create_test_keyspace();
                let store = Arc::new(FjallDedupeStore::open(&keyspace).unwrap());
                store
            },
            |store| {
                let num_threads = 8usize;
                let barrier = Arc::new(std::sync::Barrier::new(num_threads));
                let handles: Vec<_> = (0..num_threads)
                    .map(|t| {
                        let store = Arc::clone(&store);
                        let barrier = Arc::clone(&barrier);
                        thread::spawn(move || {
                            barrier.wait();
                            let mut count = 0u32;
                            for k in 0..64u64 {
                                let key = DedupeKey::parse(&format!("bench-cc-{t}-{k}")).unwrap();
                                let iid = vo_types::InstanceId::from_bytes([t as u8; 16]);
                                if matches!(
                                    store.check_and_insert(&key, &iid, 60_000).unwrap(),
                                    vo_storage::dedupe_partition::AdmissionResult::Admitted
                                ) {
                                    count += 1;
                                }
                            }
                            count
                        })
                    })
                    .collect();
                let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                black_box(total)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_entry_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedupe_encoding");
    group.throughput(Throughput::Elements(1));

    let entry = vo_storage::dedupe_partition::DedupeEntry::new(
        "workflow-order-12345".to_string(),
        "01H2X3K4M5N6P7Q8R9S0T1U2V3".to_string(),
        1_715_000_000_000,
    )
    .unwrap();

    group.bench_function("encode_entry", |b| {
        b.iter(|| black_box(vo_storage::dedupe_partition::encode_dedupe_entry(&entry)))
    });

    let encoded = vo_storage::dedupe_partition::encode_dedupe_entry(&entry).unwrap();
    group.bench_function("decode_entry", |b| {
        b.iter(|| black_box(vo_storage::dedupe_partition::decode_dedupe_entry(&encoded)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_check_and_insert_admit,
    bench_contains,
    bench_purge_expired,
    bench_concurrent_check_and_insert,
    bench_entry_encoding,
);
criterion_main!(benches);
