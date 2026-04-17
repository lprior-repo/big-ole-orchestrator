use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vo_core::effects::{
    can_commit, can_rollback, commit_effect, is_terminal, rollback_effect,
    validate_commit_precondition,
};
use vo_types::{EffectIntent, EffectKind, EffectRecord, TimestampMs};

fn make_prepared_effect(kind: EffectKind) -> EffectRecord {
    EffectRecord::new(
        format!("intent-{kind:?}"),
        kind,
        serde_json::json!({"key": "value"}),
        EffectIntent::Prepared,
        None,
    )
    .unwrap()
}

fn make_committed_effect() -> EffectRecord {
    EffectRecord::new(
        "intent-committed".to_string(),
        EffectKind::HttpCall,
        serde_json::json!({}),
        EffectIntent::Committed,
        Some(TimestampMs::try_from(1000).unwrap()),
    )
    .unwrap()
}

fn make_rolledback_effect() -> EffectRecord {
    EffectRecord::new(
        "intent-rolledback".to_string(),
        EffectKind::SqlQuery,
        serde_json::json!({}),
        EffectIntent::RolledBack,
        None,
    )
    .unwrap()
}

fn bench_can_commit(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::HttpCall);
    let committed = make_committed_effect();

    let mut group = c.benchmark_group("effects_can_commit");
    group.bench_function("prepared", |b| {
        b.iter(|| black_box(can_commit(black_box(&prepared))))
    });
    group.bench_function("committed", |b| {
        b.iter(|| black_box(can_commit(black_box(&committed))))
    });
    group.finish();
}

fn bench_can_rollback(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::SqlQuery);
    let rolledback = make_rolledback_effect();

    let mut group = c.benchmark_group("effects_can_rollback");
    group.bench_function("prepared", |b| {
        b.iter(|| black_box(can_rollback(black_box(&prepared))))
    });
    group.bench_function("rolledback", |b| {
        b.iter(|| black_box(can_rollback(black_box(&rolledback))))
    });
    group.finish();
}

fn bench_is_terminal(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::BlobWrite);
    let committed = make_committed_effect();
    let rolledback = make_rolledback_effect();

    let mut group = c.benchmark_group("effects_is_terminal");
    group.bench_function("prepared", |b| {
        b.iter(|| black_box(is_terminal(black_box(&prepared))))
    });
    group.bench_function("committed", |b| {
        b.iter(|| black_box(is_terminal(black_box(&committed))))
    });
    group.bench_function("rolledback", |b| {
        b.iter(|| black_box(is_terminal(black_box(&rolledback))))
    });
    group.finish();
}

fn bench_validate_commit_precondition(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::HttpCall);
    let committed = make_committed_effect();

    let mut group = c.benchmark_group("effects_validate_commit_precondition");
    group.bench_function("prepared_ok", |b| {
        b.iter(|| black_box(validate_commit_precondition(black_box(&prepared))))
    });
    group.bench_function("committed_err", |b| {
        b.iter(|| black_box(validate_commit_precondition(black_box(&committed))))
    });
    group.finish();
}

fn bench_commit_effect(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::HttpCall);
    let now = TimestampMs::try_from(5000).unwrap();

    let mut group = c.benchmark_group("effects_commit");
    group.bench_function("commit_prepared", |b| {
        b.iter(|| black_box(commit_effect(black_box(&prepared), black_box(now)).ok()))
    });
    group.finish();
}

fn bench_rollback_effect(c: &mut Criterion) {
    let prepared = make_prepared_effect(EffectKind::SqlQuery);

    let mut group = c.benchmark_group("effects_rollback");
    group.bench_function("rollback_prepared", |b| {
        b.iter(|| black_box(rollback_effect(black_box(&prepared)).ok()))
    });
    group.finish();
}

fn bench_effect_record_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("effects_record_new");
    group.bench_function("new_valid", |b| {
        b.iter(|| {
            black_box(
                EffectRecord::new(
                    "intent-bench".to_string(),
                    EffectKind::HttpCall,
                    serde_json::json!({"key": "value"}),
                    EffectIntent::Prepared,
                    None,
                )
                .unwrap(),
            )
        })
    });
    group.bench_function("new_empty_intent", |b| {
        b.iter(|| {
            black_box(EffectRecord::new(
                String::new(),
                EffectKind::HttpCall,
                serde_json::json!({}),
                EffectIntent::Prepared,
                None,
            ))
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_can_commit,
    bench_can_rollback,
    bench_is_terminal,
    bench_validate_commit_precondition,
    bench_commit_effect,
    bench_rollback_effect,
    bench_effect_record_new,
);
criterion_main!(benches);
