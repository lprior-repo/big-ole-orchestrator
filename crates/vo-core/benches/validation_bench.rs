use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use vo_core::validate_effect_kinds;
use vo_core::validation::{
    validate_inline_size, validate_workflow_sinks, KnownSinks, WorkflowSinkValidator,
};
use vo_types::EffectKind;
use vo_types::INLINED_MAX_BYTES;

fn bench_validate_inline_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_inline_size");

    let small = vec![0u8; 256];
    group.bench_function("small_256b_pass", |b| {
        b.iter(|| black_box(validate_inline_size(black_box(&small))))
    });

    let at_limit = vec![0u8; INLINED_MAX_BYTES];
    group.bench_function("at_limit_4096b_pass", |b| {
        b.iter(|| black_box(validate_inline_size(black_box(&at_limit))))
    });

    let over_limit = vec![0u8; INLINED_MAX_BYTES + 1];
    group.bench_function("over_limit_4097b_fail", |b| {
        b.iter(|| black_box(validate_inline_size(black_box(&over_limit))))
    });

    let large = vec![0u8; 1_000_000];
    group.bench_function("large_1mb_fail", |b| {
        b.iter(|| black_box(validate_inline_size(black_box(&large))))
    });

    group.finish();
}

fn bench_known_sinks(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_known_sinks");

    let sinks = KnownSinks::default_sinks();
    group.bench_function("default_sinks_contains_hit", |b| {
        b.iter(|| black_box(sinks.contains(black_box("blob"))))
    });
    group.bench_function("default_sinks_contains_miss", |b| {
        b.iter(|| black_box(sinks.contains(black_box("nonexistent"))))
    });

    let many_sinks: Vec<String> = (0..100).map(|i| format!("sink-{i}")).collect();
    let large_sinks = KnownSinks::new(many_sinks.iter().map(|s| s.as_str()));
    group.bench_function("large_100_sinks_contains_hit", |b| {
        b.iter(|| black_box(large_sinks.contains(black_box("sink-99"))))
    });
    group.bench_function("large_100_sinks_contains_miss", |b| {
        b.iter(|| black_box(large_sinks.contains(black_box("nonexistent"))))
    });

    group.finish();
}

fn bench_validate_workflow_sinks(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_workflow_sinks");

    let valid_sinks = vec!["blob", "http", "sql"];
    group.bench_function("valid_3_sinks", |b| {
        b.iter(|| {
            black_box(validate_workflow_sinks(black_box(
                valid_sinks.iter().copied(),
            )))
        })
    });

    let many_valid: Vec<&str> = (0..50)
        .flat_map(|i| match i % 3 {
            0 => Some("blob"),
            1 => Some("http"),
            _ => Some("sql"),
        })
        .collect();
    group.bench_function("valid_50_sinks", |b| {
        b.iter(|| {
            black_box(validate_workflow_sinks(black_box(
                many_valid.iter().copied(),
            )))
        })
    });

    let with_invalid = vec!["blob", "unknown-sink", "sql"];
    group.bench_function("invalid_3_sinks", |b| {
        b.iter(|| {
            black_box(validate_workflow_sinks(black_box(
                with_invalid.iter().copied(),
            )))
        })
    });

    group.finish();
}

fn bench_validate_effect_kinds(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_effect_kinds");

    let all_kinds = [
        EffectKind::HttpCall,
        EffectKind::SqlQuery,
        EffectKind::BlobWrite,
    ];
    group.bench_function("all_kinds_pass", |b| {
        b.iter(|| black_box(validate_effect_kinds(black_box(all_kinds.iter().copied()))))
    });

    let many_kinds: Vec<EffectKind> = (0..100)
        .map(|i| match i % 3 {
            0 => EffectKind::HttpCall,
            1 => EffectKind::SqlQuery,
            _ => EffectKind::BlobWrite,
        })
        .collect();
    group.bench_function("many_100_kinds", |b| {
        b.iter(|| black_box(validate_effect_kinds(black_box(many_kinds.iter().copied()))))
    });

    group.finish();
}

fn bench_sink_validator(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_sink_validator");

    let validator = WorkflowSinkValidator::new();
    group.bench_function("validate_single_known", |b| {
        b.iter(|| black_box(validator.validate_sink(black_box("blob"))))
    });

    let many_known: Vec<&str> = (0..50).flat_map(|_| ["blob", "http", "sql"]).collect();
    group.bench_function("validate_many_150_known", |b| {
        b.iter(|| black_box(validator.validate_sinks(black_box(many_known.iter().copied()))))
    });

    let many_sinks: Vec<String> = (0..100).map(|i| format!("sink-{i}")).collect();
    let custom_validator = WorkflowSinkValidator::with_sinks(KnownSinks::new(many_sinks.iter()));
    group.bench_function("custom_100_sinks_lookup", |b| {
        b.iter(|| black_box(custom_validator.validate_sink(black_box("sink-99"))))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_validate_inline_size,
    bench_known_sinks,
    bench_validate_workflow_sinks,
    bench_validate_effect_kinds,
    bench_sink_validator,
);
criterion_main!(benches);
