use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use vo_storage::checksum::{compute_checksum, verify_checksum, Checksum, StreamingHasher};
use vo_storage::codec::{decode_event_key, encode_event_key};
use vo_types::{InstanceId, SequenceNumber};

fn bench_compute_checksum(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_checksum");

    let small_data = b"hello world";
    group.throughput(Throughput::Bytes(small_data.len() as u64));
    group.bench_function("small_11b", |b| {
        b.iter(|| black_box(compute_checksum(black_box(small_data))))
    });

    let medium_data: Vec<u8> = (0..4096).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(medium_data.len() as u64));
    group.bench_function("medium_4kb", |b| {
        b.iter(|| black_box(compute_checksum(black_box(&medium_data))))
    });

    let large_data: Vec<u8> = (0..65536).map(|b| (b % 255) as u8).collect();
    group.throughput(Throughput::Bytes(large_data.len() as u64));
    group.bench_function("large_64kb", |b| {
        b.iter(|| black_box(compute_checksum(black_box(&large_data))))
    });

    group.finish();
}

fn bench_streaming_hasher(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_hasher");

    let chunk = vec![0xABu8; 1024];

    group.throughput(Throughput::Bytes(1024));
    group.bench_function("update_1kb", |b| {
        b.iter_batched(
            StreamingHasher::new,
            |mut hasher| {
                hasher.update(black_box(&chunk));
                hasher
            },
            BatchSize::SmallInput,
        )
    });

    group.throughput(Throughput::Bytes(1024 * 100));
    group.bench_function("update_100_chunks_1kb", |b| {
        b.iter_batched(
            StreamingHasher::new,
            |mut hasher| {
                for _ in 0..100 {
                    hasher.update(black_box(&chunk));
                }
                hasher
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("finalize", |b| {
        b.iter_batched(
            || {
                let mut hasher = StreamingHasher::new();
                hasher.update(&chunk);
                hasher
            },
            |hasher| black_box(hasher.finalize()),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_verify_checksum(c: &mut Criterion) {
    let mut group = c.benchmark_group("verify_checksum");

    let data: Vec<u8> = (0..4096).map(|b| (b % 255) as u8).collect();
    let expected = compute_checksum(&data);

    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("verify_match_4kb", |b| {
        b.iter(|| black_box(verify_checksum(black_box(&data), black_box(&expected))))
    });

    let mut wrong = expected.clone();
    wrong.crc32 = wrong.crc32.wrapping_add(1);
    group.bench_function("verify_mismatch_4kb", |b| {
        b.iter(|| black_box(verify_checksum(black_box(&data), black_box(&wrong))))
    });

    group.finish();
}

fn bench_codec_event_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec_event_key");
    group.throughput(Throughput::Elements(1));

    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    let seq = SequenceNumber::try_from(1000u64).unwrap();

    group.bench_function("encode", |b| {
        b.iter(|| black_box(encode_event_key(black_box(&id), black_box(&seq))))
    });

    let encoded = encode_event_key(&id, seq).unwrap();
    group.bench_function("decode", |b| {
        b.iter(|| black_box(decode_event_key(black_box(&encoded))))
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let enc = encode_event_key(&id, seq).unwrap();
            black_box(decode_event_key(&enc))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_compute_checksum,
    bench_streaming_hasher,
    bench_verify_checksum,
    bench_codec_event_key,
);
criterion_main!(benches);
