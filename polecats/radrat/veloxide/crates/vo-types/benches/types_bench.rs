use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use vo_types::{DedupeKey, EventEnvelope, InstanceId};

fn bench_instance_id_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_id_parse");
    group.throughput(Throughput::Elements(1));

    group.bench_function("parse_valid_ulid", |b| {
        b.iter(|| black_box(InstanceId::parse(black_box("01H5JYV4XHGSR2F8KZ9BWNRFMA"))))
    });

    group.bench_function("parse_nil_rejected", |b| {
        b.iter(|| black_box(InstanceId::parse(black_box("00000000000000000000000000"))))
    });

    group.bench_function("parse_too_short_rejected", |b| {
        b.iter(|| black_box(InstanceId::parse(black_box("01H5JY"))))
    });

    group.finish();
}

fn bench_instance_id_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_id_bytes");
    group.throughput(Throughput::Elements(1));

    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

    group.bench_function("to_bytes", |b| b.iter(|| black_box(id.to_bytes())));

    let bytes = id.to_bytes().unwrap();
    group.bench_function("from_bytes", |b| {
        b.iter(|| black_box(InstanceId::from_bytes(black_box(bytes))))
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let bytes = id.to_bytes().unwrap();
            black_box(InstanceId::from_bytes(bytes))
        })
    });

    group.finish();
}

fn bench_instance_id_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_id_serde");
    group.throughput(Throughput::Elements(1));

    let id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();

    group.bench_function("serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&id)).unwrap()))
    });

    let json = serde_json::to_string(&id).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<InstanceId>(black_box(&json)).unwrap()))
    });

    group.finish();
}

fn bench_dedupe_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedupe_key");
    group.throughput(Throughput::Elements(1));

    group.bench_function("parse_valid", |b| {
        b.iter(|| black_box(DedupeKey::parse(black_box("order-workflow-12345"))))
    });

    group.bench_function("parse_empty_rejected", |b| {
        b.iter(|| black_box(DedupeKey::parse(black_box(""))))
    });

    group.bench_function("as_str", |b| {
        let key = DedupeKey::parse("order-workflow-12345").unwrap();
        b.iter(|| black_box(key.as_str()))
    });

    group.bench_function("serde_roundtrip", |b| {
        let key = DedupeKey::parse("order-workflow-12345").unwrap();
        b.iter(|| {
            let json = serde_json::to_string(&key).unwrap();
            black_box(serde_json::from_str::<DedupeKey>(&json).unwrap())
        })
    });

    group.finish();
}

fn bench_event_envelope_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_envelope");
    group.throughput(Throughput::Elements(1));

    let envelope = EventEnvelope {
        schema_version: 1,
        instance_id: "01H5JYV4XHGSR2F8KZ9BWNRFMA".to_string(),
        sequence: 42,
        timestamp_ms: 1_715_000_000_000,
        payload: serde_json::json!({"type": "WorkflowStarted", "workflow_id": "wf-123"}),
        metadata: Default::default(),
    };

    group.bench_function("serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&envelope)).unwrap()))
    });

    let json = serde_json::to_string(&envelope).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<EventEnvelope>(black_box(&json)).unwrap()))
    });

    let compat_json = serde_json::json!({
        "version": 1,
        "instance_id": "01H5JYV4XHGSR2F8KZ9BWNRFMA",
        "sequence": 42,
        "timestamp_ms": 1_715_000_000_000u64,
        "payload": {"type": "WorkflowStarted", "workflow_id": "wf-123"}
    })
    .to_string();
    group.bench_function("from_bytes_compat", |b| {
        let bytes = compat_json.as_bytes();
        b.iter(|| black_box(EventEnvelope::from_bytes(black_box(bytes)).unwrap()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_instance_id_parse,
    bench_instance_id_bytes,
    bench_instance_id_serde,
    bench_dedupe_key,
    bench_event_envelope_serde,
);
criterion_main!(benches);
