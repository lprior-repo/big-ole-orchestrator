use std::hint::black_box;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::time::{Duration, Instant};
use vo_core::circuit_breaker::rate_limiter::{
    check_rate_limit, TokenBucketConfig, TokenBucketRateLimiter,
};
use vo_core::workflow_version::WorkflowVersion;
use vo_core::workload_class::{WorkloadBudget, WorkloadClass};
use vo_core::write_class::{WriteBudget, WriteClass};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

fn bench_check_rate_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_rate_limit");
    group.throughput(Throughput::Elements(1));

    let now = Instant::now();
    let window = Duration::from_secs(60);

    group.bench_function("no_last_registration", |b| {
        b.iter(|| {
            black_box(check_rate_limit(
                black_box(None),
                black_box(window),
                black_box(now),
            ))
        })
    });

    let last = now - Duration::from_secs(30);
    group.bench_function("within_window", |b| {
        b.iter(|| {
            black_box(check_rate_limit(
                black_box(Some(last)),
                black_box(window),
                black_box(now),
            ))
        })
    });

    let old = now - Duration::from_secs(120);
    group.bench_function("outside_window", |b| {
        b.iter(|| {
            black_box(check_rate_limit(
                black_box(Some(old)),
                black_box(window),
                black_box(now),
            ))
        })
    });

    group.finish();
}

fn bench_token_bucket_single_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_bucket_single_key");
    group.throughput(Throughput::Elements(1));

    let config = TokenBucketConfig::new(100, 10.0, 1);
    let now = Instant::now();

    group.bench_function("check_and_consume_hit", |b| {
        b.iter_batched(
            || TokenBucketRateLimiter::new(config),
            |limiter| {
                black_box(limiter.check_and_consume(black_box("key-1"), black_box(now)));
                limiter
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("peek_tokens", |b| {
        b.iter_batched(
            || {
                let limiter = TokenBucketRateLimiter::new(config);
                limiter.check_and_consume("key-1", now);
                limiter
            },
            |limiter| black_box(limiter.peek_tokens(black_box("key-1"), black_box(now))),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("burst_exhaustion_100", |b| {
        b.iter_batched(
            || TokenBucketRateLimiter::new(config),
            |limiter| {
                for _ in 0..100 {
                    black_box(limiter.check_and_consume(black_box("key-1"), black_box(now)));
                }
                limiter
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_token_bucket_multi_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_bucket_multi_key");
    group.throughput(Throughput::Elements(1));

    let config = TokenBucketConfig::new(100, 10.0, 1);
    let now = Instant::now();

    group.bench_function("100_keys", |b| {
        b.iter_batched(
            || TokenBucketRateLimiter::new(config),
            |limiter| {
                for i in 0..100u32 {
                    black_box(
                        limiter.check_and_consume(black_box(&format!("key-{i}")), black_box(now)),
                    );
                }
                limiter
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("1000_keys", |b| {
        b.iter_batched(
            || TokenBucketRateLimiter::new(config),
            |limiter| {
                for i in 0..1000u32 {
                    black_box(
                        limiter.check_and_consume(black_box(&format!("key-{i}")), black_box(now)),
                    );
                }
                limiter
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_write_budget(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_budget");
    group.throughput(Throughput::Elements(1));

    group.bench_function("reserve_success", |b| {
        b.iter_batched(
            || WriteBudget::new(1_000_000, 2_000_000, 5_000_000),
            |budget| {
                black_box(
                    budget.reserve(black_box(WriteClass::CriticalControlPlane), black_box(1024)),
                );
                budget
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("can_write", |b| {
        let budget = WriteBudget::new(1_000_000, 2_000_000, 5_000_000);
        b.iter(|| {
            black_box(budget.can_write(black_box(WriteClass::OperatorProjection), black_box(4096)))
        })
    });

    group.bench_function("remaining", |b| {
        let budget = WriteBudget::new(1_000_000, 2_000_000, 5_000_000);
        b.iter(|| black_box(budget.remaining(black_box(WriteClass::BulkBlob))))
    });

    group.bench_function("reserve_loop_1k", |b| {
        b.iter_batched(
            || WriteBudget::new(1_000_000_000, 2_000_000_000, 5_000_000_000),
            |budget| {
                for i in 0..1000u64 {
                    let _ = budget.reserve(WriteClass::CriticalControlPlane, 1024 + (i % 100));
                }
                budget
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_workload_budget(c: &mut Criterion) {
    let mut group = c.benchmark_group("workload_budget");
    group.throughput(Throughput::Elements(1));

    group.bench_function("acquire_standard", |b| {
        b.iter_batched(
            || WorkloadBudget::default_budget(),
            |budget| {
                black_box(budget.acquire(black_box(WorkloadClass::Standard)));
                budget
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("release_standard", |b| {
        b.iter_batched(
            || {
                let budget = WorkloadBudget::default_budget();
                budget.acquire(WorkloadClass::Standard).unwrap();
                budget
            },
            |budget| {
                black_box(budget.release(black_box(WorkloadClass::Standard)));
                budget
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("can_acquire", |b| {
        let budget = WorkloadBudget::default_budget();
        b.iter(|| black_box(budget.can_acquire(black_box(WorkloadClass::ExactCritical))))
    });

    group.bench_function("remaining", |b| {
        let budget = WorkloadBudget::default_budget();
        b.iter(|| black_box(budget.remaining(black_box(WorkloadClass::Standard))))
    });

    group.bench_function("acquire_release_cycle_1k", |b| {
        b.iter_batched(
            || WorkloadBudget::new(50, 1000, 500, 100),
            |budget| {
                for _ in 0..1000 {
                    let _ = budget.acquire(WorkloadClass::Standard);
                    budget.release(WorkloadClass::Standard);
                }
                budget
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_workflow_version(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_version");
    group.throughput(Throughput::Elements(1));

    let name = WorkflowName::parse("my-workflow").unwrap();
    let hash_str = "a".repeat(64);
    let hash = BinaryHash::parse(&hash_str).unwrap();
    let ts = TimestampMs::try_from(1_715_000_000_000u64).unwrap();

    group.bench_function("construction", |b| {
        b.iter(|| {
            black_box(WorkflowVersion::new(
                black_box(name.clone()),
                black_box(hash.clone()),
                black_box(ts),
                black_box(vo_types::VERSION_BASE_PATH),
            ))
        })
    });

    let version = WorkflowVersion::new(name.clone(), hash.clone(), ts, vo_types::VERSION_BASE_PATH).unwrap();
    group.bench_function("serde_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&version)).unwrap();
            black_box(serde_json::from_str::<WorkflowVersion>(black_box(&json)).unwrap())
        })
    });

    group.bench_function("serde_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&version)).unwrap()))
    });

    group.bench_function("serde_deserialize", |b| {
        let json = serde_json::to_string(&version).unwrap();
        b.iter(|| black_box(serde_json::from_str::<WorkflowVersion>(black_box(&json)).unwrap()))
    });

    group.finish();
}

fn bench_write_class_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_class_parse");
    group.throughput(Throughput::Elements(1));

    group.bench_function("parse_critical", |b| {
        b.iter(|| black_box(WriteClass::parse(black_box("critical_control_plane"))))
    });

    group.bench_function("parse_invalid", |b| {
        b.iter(|| black_box(WriteClass::parse(black_box("not_a_class"))))
    });

    group.bench_function("tier", |b| {
        b.iter(|| black_box(WriteClass::OperatorProjection.tier()))
    });

    group.bench_function("as_str", |b| {
        b.iter(|| black_box(WriteClass::BulkBlob.as_str()))
    });

    group.finish();
}

fn bench_workload_class_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("workload_class_parse");
    group.throughput(Throughput::Elements(1));

    group.bench_function("parse_standard", |b| {
        b.iter(|| black_box(WorkloadClass::parse(black_box("standard"))))
    });

    group.bench_function("parse_exact_critical", |b| {
        b.iter(|| black_box(WorkloadClass::parse(black_box("exact_critical"))))
    });

    group.bench_function("rank", |b| {
        b.iter(|| black_box(WorkloadClass::ExactCritical.rank()))
    });

    group.bench_function("serde_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&WorkloadClass::Standard).unwrap();
            black_box(serde_json::from_str::<WorkloadClass>(&json).unwrap())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_check_rate_limit,
    bench_token_bucket_single_key,
    bench_token_bucket_multi_key,
    bench_write_budget,
    bench_workload_budget,
    bench_workflow_version,
    bench_write_class_parse,
    bench_workload_class_parse,
);
criterion_main!(benches);
