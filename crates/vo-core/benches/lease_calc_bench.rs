use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use vo_core::lease_calc::{apply, LeaseState, LeaseTransition};
use vo_types::NodeName;

fn node(name: &str) -> NodeName {
    NodeName::parse(name).expect("valid node name")
}

fn bench_acquire_vacant(c: &mut Criterion) {
    let state = LeaseState::Vacant;

    let mut group = c.benchmark_group("lease_acquire");
    group.bench_function("acquire_vacant", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&state),
                LeaseTransition::Acquire {
                    requester: node("node-a"),
                    ttl_ms: 5000,
                    now_ms: 1000,
                },
            ))
        })
    });

    let expired = LeaseState::Expired {
        last_holder: node("node-a"),
    };
    group.bench_function("acquire_expired", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&expired),
                LeaseTransition::Acquire {
                    requester: node("node-b"),
                    ttl_ms: 5000,
                    now_ms: 10000,
                },
            ))
        })
    });

    group.finish();
}

fn bench_renew(c: &mut Criterion) {
    let state = LeaseState::Held {
        holder: node("node-a"),
        expires_at_ms: 6000,
    };

    let mut group = c.benchmark_group("lease_renew");
    group.bench_function("renew_same_node", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&state),
                LeaseTransition::Renew {
                    requester: node("node-a"),
                    ttl_ms: 3000,
                    now_ms: 4000,
                },
            ))
        })
    });

    group.finish();
}

fn bench_tick(c: &mut Criterion) {
    let held = LeaseState::Held {
        holder: node("node-a"),
        expires_at_ms: 6000,
    };
    let expired = LeaseState::Expired {
        last_holder: node("node-a"),
    };

    let mut group = c.benchmark_group("lease_tick");
    group.bench_function("tick_before_expiry", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&held),
                LeaseTransition::Tick { now_ms: 5000 },
            ))
        })
    });
    group.bench_function("tick_at_expiry", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&held),
                LeaseTransition::Tick { now_ms: 6000 },
            ))
        })
    });
    group.bench_function("tick_already_expired", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&expired),
                LeaseTransition::Tick { now_ms: 20000 },
            ))
        })
    });

    group.finish();
}

fn bench_release(c: &mut Criterion) {
    let state = LeaseState::Held {
        holder: node("node-a"),
        expires_at_ms: 6000,
    };

    let mut group = c.benchmark_group("lease_release");
    group.bench_function("release_by_holder", |b| {
        b.iter(|| {
            black_box(apply(
                black_box(&state),
                LeaseTransition::Release {
                    requester: node("node-a"),
                },
            ))
        })
    });

    group.finish();
}

fn bench_full_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("lease_lifecycle");
    group.bench_function("acquire_renew_tick_release", |b| {
        b.iter(|| {
            let s0 = LeaseState::Vacant;
            let s1 = apply(
                &s0,
                LeaseTransition::Acquire {
                    requester: node("node-a"),
                    ttl_ms: 5000,
                    now_ms: 1000,
                },
            )
            .unwrap();
            let s2 = apply(
                &s1,
                LeaseTransition::Renew {
                    requester: node("node-a"),
                    ttl_ms: 5000,
                    now_ms: 3000,
                },
            )
            .unwrap();
            let s3 = apply(&s2, LeaseTransition::Tick { now_ms: 7000 }).unwrap();
            let s4 = apply(
                &s3,
                LeaseTransition::Acquire {
                    requester: node("node-b"),
                    ttl_ms: 3000,
                    now_ms: 9000,
                },
            )
            .unwrap();
            black_box(apply(
                &s4,
                LeaseTransition::Release {
                    requester: node("node-b"),
                },
            ))
        })
    });

    group.finish();
}

fn bench_contention_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("lease_contention");
    group.bench_function("failed_acquire_by_different_node", |b| {
        let state = LeaseState::Held {
            holder: node("node-a"),
            expires_at_ms: 6000,
        };
        b.iter(|| {
            black_box(apply(
                black_box(&state),
                LeaseTransition::Acquire {
                    requester: node("node-b"),
                    ttl_ms: 5000,
                    now_ms: 1000,
                },
            ))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_acquire_vacant,
    bench_renew,
    bench_tick,
    bench_release,
    bench_full_lifecycle,
    bench_contention_simulation,
);
criterion_main!(benches);
