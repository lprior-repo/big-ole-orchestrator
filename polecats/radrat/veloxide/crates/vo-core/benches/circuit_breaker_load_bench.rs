use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;
use vo_core::circuit_breaker::{
    evaluate_registration, record_failure, CircuitBreakerConfig, CircuitBreakerState,
    RegistrationRequest,
};
use vo_types::{BinaryHash, WorkflowName};

fn make_config() -> CircuitBreakerConfig {
    CircuitBreakerConfig::new(
        std::time::Duration::from_secs(60),
        std::time::Duration::from_secs(600),
        5,
    )
    .unwrap()
}

fn make_workflow_name(id: u64) -> WorkflowName {
    WorkflowName::parse(&format!("wf-load-{id}")).unwrap()
}

fn make_binary_hash(id: u64) -> BinaryHash {
    let hex = format!("{id:064x}");
    BinaryHash::parse(&hex).unwrap()
}

fn bench_concurrent_registration(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_cb_concurrent_registration");
    group.throughput(Throughput::Elements(1));

    for num_threads in [4, 8, 16, 32, 64] {
        group.bench_function(format!("{}_threads_distinct", num_threads), |b| {
            b.iter_batched(
                || {
                    let state = Arc::new(CircuitBreakerState::new());
                    let config = make_config();
                    (state, config)
                },
                |(state, config)| {
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let state = Arc::clone(&state);
                            let config = config.clone();
                            let barrier = Arc::clone(&barrier);
                            thread::spawn(move || {
                                barrier.wait();
                                let mut allowed = 0u64;
                                for i in 0..100u64 {
                                    let wf = make_workflow_name(t as u64 * 100 + i);
                                    let hash = make_binary_hash(t as u64 * 100 + i);
                                    let req = RegistrationRequest {
                                        workflow_name: wf,
                                        binary_hash: hash,
                                        force: false,
                                    };
                                    let now = Instant::now();
                                    match evaluate_registration(&req, &config, &state, now).unwrap()
                                    {
                                        vo_core::circuit_breaker::RegistrationOutcome::Allowed => {
                                            allowed += 1
                                        }
                                        _ => {}
                                    }
                                }
                                allowed
                            })
                        })
                        .collect();
                    let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                    black_box(total)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_concurrent_rate_limited(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_cb_rate_limited");
    group.throughput(Throughput::Elements(1));

    for num_threads in [4, 8, 16, 32] {
        group.bench_function(format!("{}_threads_same_wf", num_threads), |b| {
            b.iter_batched(
                || {
                    let state = Arc::new(CircuitBreakerState::new());
                    let config = make_config();
                    (state, config)
                },
                |(state, config)| {
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_t| {
                            let state = Arc::clone(&state);
                            let config = config.clone();
                            let barrier = Arc::clone(&barrier);
                            thread::spawn(move || {
                                barrier.wait();
                                let mut allowed = 0u64;
                                let mut rate_limited = 0u64;
                                for i in 0..100u64 {
                                    let wf =
                                        WorkflowName::parse(&format!("shared-wf-{i}")).unwrap();
                                    let hash = make_binary_hash(i);
                                    let req = RegistrationRequest {
                                        workflow_name: wf,
                                        binary_hash: hash,
                                        force: false,
                                    };
                                    let now = Instant::now();
                                    match evaluate_registration(&req, &config, &state, now).unwrap()
                                    {
                                        vo_core::circuit_breaker::RegistrationOutcome::Allowed => {
                                            allowed += 1
                                        }
                                        vo_core::circuit_breaker::RegistrationOutcome::RateLimited {
                                            ..
                                        } => rate_limited += 1,
                                        _ => {}
                                    }
                                }
                                (allowed, rate_limited)
                            })
                        })
                        .collect();
                    let results: Vec<(u64, u64)> =
                        handles.into_iter().map(|h| h.join().unwrap()).collect();
                    let total_allowed: u64 = results.iter().map(|(a, _)| *a).sum();
                    let total_limited: u64 = results.iter().map(|(_, l)| *l).sum();
                    black_box((total_allowed, total_limited))
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_concurrent_failure_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_cb_failure_recording");
    group.throughput(Throughput::Elements(1));

    for num_threads in [4, 8, 16, 32] {
        group.bench_function(format!("{}_threads", num_threads), |b| {
            b.iter_batched(
                || {
                    let state = Arc::new(CircuitBreakerState::new());
                    let config = make_config();
                    (state, config)
                },
                |(state, config)| {
                    let barrier = Arc::new(Barrier::new(num_threads));
                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let state = Arc::clone(&state);
                            let config = config.clone();
                            let barrier = Arc::clone(&barrier);
                            thread::spawn(move || {
                                barrier.wait();
                                let mut recorded = 0u64;
                                for i in 0..100u64 {
                                    let wf = make_workflow_name(t as u64 * 100 + i);
                                    let hash = make_binary_hash(i * 1000 + t as u64);
                                    let now = Instant::now();
                                    let result =
                                        record_failure(&wf, &hash, &config, &state, now).unwrap();
                                    if result.is_some() {
                                        recorded += 1;
                                    }
                                }
                                recorded
                            })
                        })
                        .collect();
                    let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                    black_box(total)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_mixed_registration_and_failure(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_cb_mixed_ops");
    group.throughput(Throughput::Elements(1));

    group.bench_function("16_threads_mixed_ops", |b| {
        b.iter_batched(
            || {
                let state = Arc::new(CircuitBreakerState::new());
                let config = make_config();
                (state, config)
            },
            |(state, config)| {
                let barrier = Arc::new(Barrier::new(16));
                let handles: Vec<_> = (0..16)
                    .map(|t| {
                        let state = Arc::clone(&state);
                        let config = config.clone();
                        let barrier = Arc::clone(&barrier);
                        thread::spawn(move || {
                            barrier.wait();
                            let mut allowed = 0u64;
                            let mut failures = 0u64;
                            for i in 0..200u64 {
                                let now = Instant::now();
                                if i % 4 == 0 {
                                    let wf = make_workflow_name(t as u64 * 200 + i);
                                    let hash = make_binary_hash(i);
                                    let req = RegistrationRequest {
                                        workflow_name: wf,
                                        binary_hash: hash,
                                        force: false,
                                    };
                                    if matches!(
                                        evaluate_registration(&req, &config, &state, now).unwrap(),
                                        vo_core::circuit_breaker::RegistrationOutcome::Allowed
                                    ) {
                                        allowed += 1;
                                    }
                                } else {
                                    let wf = make_workflow_name(t as u64);
                                    let hash = make_binary_hash(i * 1000 + t as u64);
                                    let result =
                                        record_failure(&wf, &hash, &config, &state, now).unwrap();
                                    if result.is_some() {
                                        failures += 1;
                                    }
                                }
                            }
                            (allowed, failures)
                        })
                    })
                    .collect();
                let results: Vec<(u64, u64)> =
                    handles.into_iter().map(|h| h.join().unwrap()).collect();
                let total_allowed: u64 = results.iter().map(|(a, _)| *a).sum();
                let total_failures: u64 = results.iter().map(|(_, f)| *f).sum();
                black_box((total_allowed, total_failures))
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_dashmap_status_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_cb_state_contention");
    group.throughput(Throughput::Elements(1));

    for num_threads in [4, 8, 16, 32, 64] {
        group.bench_function(format!("{}_threads_get_status", num_threads), |b| {
            let state = Arc::new(CircuitBreakerState::new());
            for i in 0..100u64 {
                let wf = make_workflow_name(i);
                state.set_status(wf, vo_core::circuit_breaker::RegistrationStatus::Active);
            }
            b.iter(|| {
                let barrier = Arc::new(Barrier::new(num_threads));
                let handles: Vec<_> = (0..num_threads)
                    .map(|_t| {
                        let state = Arc::clone(&state);
                        let barrier = Arc::clone(&barrier);
                        thread::spawn(move || {
                            barrier.wait();
                            let mut ops = 0u64;
                            for i in 0..10_000u64 {
                                let wf = make_workflow_name(i % 100);
                                black_box(state.get_status(&wf));
                                ops += 1;
                            }
                            ops
                        })
                    })
                    .collect();
                let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
                black_box(total)
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_registration,
    bench_concurrent_rate_limited,
    bench_concurrent_failure_recording,
    bench_mixed_registration_and_failure,
    bench_dashmap_status_contention,
);
criterion_main!(benches);
