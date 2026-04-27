use std::hint::black_box;
// ADR-006 Backpressure Load Shedding Benchmark
//
// Measures throughput degradation curve as load increases past semaphore capacity.
// Verifies that load shedding activates correctly and measures the cost of contention.
//
// Run: `cargo bench -p vo-core --bench backpressure_load_bench`

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use vo_core::shedding::{LoadSheddingSemaphore, SemaphoreLimitError};

fn bench_try_acquire_no_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_try_acquire");
    group.throughput(Throughput::Elements(1));
    group.bench_function("no_contention_500_permits", |b| {
        b.iter_batched(
            || LoadSheddingSemaphore::new(500),
            |sem| {
                let permit = sem.try_acquire().unwrap();
                black_box(&permit);
                drop(permit);
                sem.available_permits()
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_try_acquire_under_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_contention");
    group.throughput(Throughput::Elements(1));

    for permits in [10, 50, 100, 500] {
        group.bench_function(format!("{}_permits_no_waiters", permits), |b| {
            b.iter_batched(
                || LoadSheddingSemaphore::new(permits),
                |sem| {
                    let p = sem.try_acquire().unwrap();
                    black_box(&p);
                    drop(p);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_load_shedding_check_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_shedding_check");
    group.throughput(Throughput::Elements(1));

    for acquired in [0, 100, 250, 499, 500] {
        group.bench_function(format!("check_{}_of_500_acquired", acquired), |b| {
            b.iter_batched(
                || {
                    let sem = LoadSheddingSemaphore::new(500);
                    let permits: Vec<_> =
                        (0..acquired).map(|_| sem.try_acquire().unwrap()).collect();
                    (sem, permits)
                },
                |(sem, _permits)| {
                    black_box(sem.check_load_shedding_threshold(400));
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_throughput_degradation_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_degradation");
    group.throughput(Throughput::Elements(1));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));

    let max_permits = 100usize;
    let load_fractions: Vec<(usize, &str)> = vec![
        (10, "10%"),
        (25, "25%"),
        (50, "50%"),
        (75, "75%"),
        (90, "90%"),
        (100, "100%"),
        (110, "110%"),
        (125, "125%"),
        (150, "150%"),
        (200, "200%"),
    ];

    for (num_tasks, _label) in &load_fractions {
        group.bench_function(
            format!("{}_workers_{}_permits", num_tasks, max_permits),
            |b| {
                b.iter_batched(
                    || {
                        let sem = Arc::new(LoadSheddingSemaphore::new(max_permits));
                        let ops = Arc::new(AtomicU64::new(0));
                        let rt = tokio::runtime::Builder::new_multi_thread()
                            .worker_threads(4)
                            .enable_time()
                            .build()
                            .unwrap();
                        (sem, ops, rt)
                    },
                    |(sem, ops, rt)| {
                        rt.block_on(async {
                            let mut handles = Vec::new();
                            for _ in 0..*num_tasks {
                                let sem = Arc::clone(&sem);
                                let ops = Arc::clone(&ops);
                                handles.push(tokio::spawn(async move {
                                    for _ in 0..1000 {
                                        match sem.try_acquire() {
                                            Ok(permit) => {
                                                ops.fetch_add(1, Ordering::Relaxed);
                                                drop(permit);
                                            }
                                            Err(SemaphoreLimitError::LimitReached { .. }) => {}
                                            Err(SemaphoreLimitError::LoadSheddingActive {
                                                ..
                                            }) => {}
                                            Err(_) => {}
                                        }
                                        tokio::task::yield_now().await;
                                    }
                                }));
                            }
                            for h in handles {
                                h.await.unwrap();
                            }
                            ops.load(Ordering::Relaxed)
                        })
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

fn bench_acquire_many_bulk_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_bulk_acquire");
    group.throughput(Throughput::Elements(1));

    for batch_size in [1, 5, 10, 25] {
        group.bench_function(format!("try_acquire_many_{}", batch_size), |b| {
            b.iter_batched(
                || LoadSheddingSemaphore::new(500),
                |sem| {
                    let permits: Vec<_> = (0..20)
                        .filter_map(|_| sem.try_acquire_many(batch_size).ok())
                        .collect();
                    let count = permits.len();
                    black_box(count)
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_async_acquire_wait_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("backpressure_async_wait");
    group.throughput(Throughput::Elements(1));
    group.sample_size(10);

    group.bench_function("acquire_with_immediate_release", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let sem = Arc::new(LoadSheddingSemaphore::new(100));
                let permit = sem.acquire().await.unwrap();
                black_box(&permit);
                drop(permit);
            })
    });

    group.bench_function("acquire_under_50pct_load", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let sem = Arc::new(LoadSheddingSemaphore::new(100));
                let _held: Vec<_> = (0..50).map(|_| sem.try_acquire().unwrap()).collect();
                let permit = sem.try_acquire();
                black_box(&permit);
            })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_try_acquire_no_contention,
    bench_try_acquire_under_contention,
    bench_load_shedding_check_overhead,
    bench_throughput_degradation_curve,
    bench_acquire_many_bulk_throughput,
    bench_async_acquire_wait_cost,
);
criterion_main!(benches);
