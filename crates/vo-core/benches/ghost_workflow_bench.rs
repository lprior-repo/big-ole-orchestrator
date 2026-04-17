use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use vo_core::ghost_workflow::{GhostLifecycle, WorkflowRegistration};
use vo_types::{BinaryHash, TimestampMs, WorkflowName};

fn make_hash() -> BinaryHash {
    BinaryHash::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap()
}

fn make_name(s: &str) -> WorkflowName {
    WorkflowName::parse(s).unwrap()
}

fn make_ts(ms: u64) -> TimestampMs {
    TimestampMs::try_from(ms).unwrap()
}

fn make_registration(name: &str) -> WorkflowRegistration {
    WorkflowRegistration::new(make_name(name), make_hash(), make_ts(1000))
}

fn bench_register(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_register");

    for count in [10, 50, 100] {
        group.bench_function(format!("register_{}_workflows", count), |b| {
            b.iter_batched(
                || GhostLifecycle::new(),
                |mut lc| {
                    for i in 0..count {
                        lc.register(make_registration(&format!("wf-{i}")));
                    }
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_check_trigger(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_check_trigger");

    let mut lc = GhostLifecycle::new();
    lc.register(make_registration("wf-active"));
    let name = make_name("wf-active");

    group.bench_function("trigger_active", |b| {
        b.iter(|| black_box(lc.check_trigger(black_box(&name))))
    });

    let name_missing = make_name("wf-nonexistent");
    group.bench_function("trigger_missing", |b| {
        b.iter(|| black_box(lc.check_trigger(black_box(&name_missing))))
    });

    group.finish();
}

fn bench_deactivate(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_deactivate");

    let mut lc = GhostLifecycle::new();
    for i in 0..50 {
        lc.register(make_registration(&format!("wf-{i}")));
    }
    let name = make_name("wf-25");

    group.bench_function("deactivate_active", |b| {
        b.iter_batched(
            || {
                let mut lc = GhostLifecycle::new();
                for i in 0..50 {
                    lc.register(make_registration(&format!("wf-{i}")));
                }
                lc
            },
            |mut lc| black_box(lc.deactivate(black_box(&name), black_box(make_ts(2000)))),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_reap(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_reap");

    for (total, reapable) in [(10, 5), (50, 25), (100, 50)] {
        group.bench_function(format!("reap_{}_total_{}_reapable", total, reapable), |b| {
            b.iter_batched(
                || {
                    let mut lc = GhostLifecycle::new();
                    for i in 0..total {
                        lc.register(make_registration(&format!("wf-{i}")));
                    }
                    for i in 0..reapable {
                        let name = make_name(&format!("wf-{i}"));
                        lc.deactivate(&name, make_ts(2000)).unwrap();
                    }
                    lc
                },
                |mut lc| black_box(lc.reap()),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_full_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_lifecycle");
    group.bench_function("register_deactivate_reap", |b| {
        b.iter_batched(
            || {
                let mut lc = GhostLifecycle::new();
                lc.register(make_registration("wf-bench"));
                lc
            },
            |mut lc| {
                let name = make_name("wf-bench");
                lc.deactivate(&name, make_ts(2000)).unwrap();
                black_box(lc.reap())
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_instance_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("ghost_instance_tracking");

    let mut lc = GhostLifecycle::new();
    lc.register(make_registration("wf-bench"));
    let name = make_name("wf-bench");

    group.bench_function("instance_started_completed", |b| {
        b.iter(|| {
            black_box(lc.instance_started(black_box(&name)));
            black_box(lc.instance_completed(black_box(&name)));
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_register,
    bench_check_trigger,
    bench_deactivate,
    bench_reap,
    bench_full_lifecycle,
    bench_instance_tracking,
);
criterion_main!(benches);
