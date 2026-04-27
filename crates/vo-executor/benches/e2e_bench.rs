use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_error_count, get_execution_status,
    get_state_count, reset_all_state, RetryPolicy, StepId,
};

fn rt() -> Runtime {
    Runtime::new().expect("create runtime for benchmark")
}

fn bench_execute_step_success_latency(c: &mut Criterion) {
    let runtime = rt();
    let step_id = StepId::new("step-1".to_string());

    c.bench_function("execute_step_success_latency", |b| {
        b.to_async(&runtime).iter(|| async {
            let start = std::time::Instant::now();
            for _ in 0..100 {
                black_box(execute_step(step_id.clone(), 5000).await).unwrap();
            }
            start.elapsed()
        })
    });
}

fn bench_execute_step_concurrent_throughput(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_concurrent_throughput");
    group.throughput(Throughput::Elements(1));

    for num_tasks in [10, 50, 100, 500] {
        group.bench_function(format!("{}_tasks", num_tasks), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_tasks);
                for t in 0..num_tasks {
                    let step_id = StepId::new(format!("workflow-step-{}", t % 10));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move { execute_step(step_id, 5000).await.unwrap() }
                    }));
                }
                for handle in handles {
                    black_box(handle.await.expect("task join"));
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_execute_step_with_retry_throughput(c: &mut Criterion) {
    let runtime = rt();
    let policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let step_id = StepId::new("step-retry".to_string());

    c.bench_function("execute_step_with_retry_throughput", |b| {
        b.to_async(&runtime).iter(|| async {
            let start = std::time::Instant::now();
            for _ in 0..10 {
                black_box(execute_step_with_retry(step_id.clone(), 5000, policy.clone()).await)
                    .unwrap();
            }
            start.elapsed()
        })
    });
}

fn bench_get_execution_status_no_contention(c: &mut Criterion) {
    let runtime = rt();
    let step_id = StepId::new("step-1".to_string());

    runtime.block_on(async {
        execute_step(step_id.clone(), 5000).await.unwrap();
    });

    c.bench_function("get_execution_status_no_contention", |b| {
        b.to_async(&runtime).iter(|| async {
            black_box(get_execution_status(&step_id));
        })
    });
}

fn bench_get_execution_status_concurrent(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("get_execution_status_concurrent");

    for num_readers in [10, 50, 100] {
        group.bench_function(format!("{}_readers", num_readers), |b| {
            b.to_async(&runtime).iter(move || async move {
                let step_id = StepId::new("step-1".to_string());
                execute_step(step_id.clone(), 5000).await.unwrap();

                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_readers);
                for _ in 0..num_readers {
                    let sid = step_id.clone();
                    handles.push(tokio::spawn(async move { get_execution_status(&sid) }));
                }
                for handle in handles {
                    black_box(handle.await.expect("reader join"));
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_execute_step_mixed_workload(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_mixed_workload");
    group.throughput(Throughput::Elements(1));

    group.bench_function("mixed_100_steps", |b| {
        b.to_async(&runtime).iter(move || async move {
            let start = std::time::Instant::now();
            let mut handles = Vec::with_capacity(100);
            for i in 0..100 {
                let step_name = match i % 4 {
                    0 => "step-1",
                    1 => "step-good",
                    2 => "step-fail",
                    _ => "step-retry",
                };
                let step_id = StepId::new(format!("{}-{}", step_name, i));
                handles.push(tokio::spawn({
                    let step_id = step_id.clone();
                    async move { execute_step(step_id, 5000).await }
                }));
            }
            for handle in handles {
                let _ = black_box(handle.await.expect("task join"));
            }
            start.elapsed()
        })
    });
    group.finish();
}

fn bench_execute_step_scaling(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_scaling");
    group.throughput(Throughput::Elements(1));

    for batch_size in [1, 10, 50, 100, 200] {
        group.bench_function(format!("batch_size_{}", batch_size), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(batch_size);
                for t in 0..batch_size {
                    let step_id = StepId::new(format!("step-{}-{}", batch_size, t));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move { execute_step(step_id, 5000).await.unwrap() }
                    }));
                }
                for handle in handles {
                    black_box(handle.await.expect("batch task join"));
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_cancel_execution_no_op(c: &mut Criterion) {
    let runtime = rt();
    let step_id = StepId::new("step-1".to_string());

    runtime.block_on(async {
        execute_step(step_id.clone(), 5000).await.unwrap();
    });

    c.bench_function("cancel_execution_no_op", |b| {
        b.to_async(&runtime).iter(|| async {
            let sid = step_id.clone();
            black_box(cancel_execution(sid).await)
        })
    });
}

fn bench_reset_all_state(c: &mut Criterion) {
    let runtime = rt();

    runtime.block_on(async {
        for i in 0..1000 {
            let step_id = StepId::new(format!("step-{}", i % 10));
            execute_step(step_id, 5000).await.unwrap();
        }
    });

    c.bench_function("reset_all_state_1000_entries", |b| {
        b.iter(|| {
            reset_all_state();
        })
    });
}

fn bench_execute_step_success_sequential(c: &mut Criterion) {
    let runtime = rt();
    let step_id = StepId::new("step-1".to_string());

    c.bench_function("execute_step_success_sequential", |b| {
        b.to_async(&runtime).iter(|| async {
            let start = std::time::Instant::now();
            for _ in 0..1000 {
                black_box(execute_step(step_id.clone(), 5000).await).unwrap();
            }
            start.elapsed()
        })
    });
}

fn bench_execute_step_many_distinct_steps(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_distinct_steps");

    for num_steps in [10, 100, 1000] {
        group.bench_function(format!("{}_distinct_steps", num_steps), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = std::time::Instant::now();
                for i in 0..num_steps {
                    let step_id = StepId::new(format!("step-{}", i));
                    black_box(execute_step(step_id, 5000).await).unwrap();
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_execute_step_latency_percentiles(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_latency_percentiles");
    group.throughput(Throughput::Elements(1));

    for num_ops in [100, 500, 1000] {
        group.bench_function(format!("{}_ops", num_ops), |b| {
            b.to_async(&runtime).iter(move || async move {
                let mut latencies = Vec::with_capacity(num_ops);
                for i in 0..num_ops {
                    let step_id = StepId::new(format!("latency-step-{}", i));
                    let start = std::time::Instant::now();
                    black_box(execute_step(step_id, 5000).await).unwrap();
                    let elapsed = start.elapsed();
                    latencies.push(elapsed);
                }
                latencies.sort();
                let p50 = latencies[num_ops / 2];
                let p95 = latencies[(num_ops as f64 * 0.95) as usize];
                let p99 = latencies[(num_ops as f64 * 0.99) as usize];
                (p50, p95, p99)
            })
        });
    }
    group.finish();
}

fn bench_execute_step_concurrent_latency_percentiles(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("execute_step_concurrent_latency_percentiles");

    for num_tasks in [10, 50, 100] {
        group.bench_function(format!("{}_tasks", num_tasks), |b| {
            b.to_async(&runtime).iter(move || async move {
                let mut all_latencies = Vec::with_capacity(num_tasks * 10);
                for batch in 0..10 {
                    let start = std::time::Instant::now();
                    let mut handles = Vec::with_capacity(num_tasks);
                    for t in 0..num_tasks {
                        let step_id = StepId::new(format!("batch-{}-step-{}", batch, t));
                        handles.push(tokio::spawn({
                            let step_id = step_id.clone();
                            async move {
                                let start = std::time::Instant::now();
                                execute_step(step_id, 5000).await.unwrap();
                                start.elapsed()
                            }
                        }));
                    }
                    for handle in handles {
                        let elapsed = handle.await.expect("task join");
                        all_latencies.push(elapsed);
                    }
                }
                all_latencies.sort();
                let p50 = all_latencies[all_latencies.len() / 2];
                let p95 = all_latencies[(all_latencies.len() as f64 * 0.95) as usize];
                let p99 = all_latencies[(all_latencies.len() as f64 * 0.99) as usize];
                (p50, p95, p99)
            })
        });
    }
    group.finish();
}

fn bench_sustained_load_throughput(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("sustained_load_throughput");
    group.throughput(Throughput::Elements(1));

    group.bench_function("1000_ops_10_concurrent_sustained", |b| {
        b.to_async(&runtime).iter(move || async move {
            let total_ops = 1000;
            let concurrent = 10;
            let mut total_completed = 0u64;
            let start = std::time::Instant::now();

            while total_completed < total_ops as u64 {
                let mut handles = Vec::with_capacity(concurrent);
                for t in 0..concurrent {
                    if total_completed + t as u64 >= total_ops as u64 {
                        break;
                    }
                    let step_id =
                        StepId::new(format!("sustained-step-{}", total_completed + t as u64));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move { execute_step(step_id, 5000).await.unwrap() }
                    }));
                }
                for handle in handles {
                    black_box(handle.await.expect("task join"));
                    total_completed += 1;
                }
            }
            let elapsed = start.elapsed();
            let throughput = (total_completed as f64) / elapsed.as_secs_f64();
            (throughput, elapsed)
        })
    });
    group.finish();
}

fn bench_high_concurrency_stress(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("high_concurrency_stress");

    for num_tasks in [200, 500, 1000] {
        group.bench_function(format!("{}_tasks", num_tasks), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_tasks);
                for t in 0..num_tasks {
                    let step_id = StepId::new(format!("stress-step-{}", t));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move { execute_step(step_id, 5000).await.unwrap() }
                    }));
                }
                for handle in handles {
                    black_box(handle.await.expect("stress task join"));
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_mixed_workload_throughput(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("mixed_workload_throughput");
    group.throughput(Throughput::Elements(1));

    for num_steps in [50, 100, 200] {
        group.bench_function(format!("{}_steps_various_types", num_steps), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_steps);
                for i in 0..num_steps {
                    let (step_name, delay) = match i % 5 {
                        0 => ("step-good", 5000),
                        1 => ("step-fail", 5000),
                        2 => ("step-retry", 5000),
                        3 => ("step-timeout", 100),
                        _ => ("step-1", 5000),
                    };
                    let step_id = StepId::new(format!("{}-{}", step_name, i));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move {
                            let result = execute_step(step_id, delay).await;
                            black_box(result)
                        }
                    }));
                }
                let mut successes = 0u32;
                let mut failures = 0u32;
                for handle in handles {
                    match handle.await.expect("mixed workload join") {
                        Ok(_) => successes += 1,
                        Err(_) => failures += 1,
                    }
                }
                (successes, failures, start.elapsed())
            })
        });
    }
    group.finish();
}

fn bench_memory_leak_state_growth(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("memory_leak_state");

    for num_distinct_steps in [100, 500, 1000, 5000] {
        group.bench_function(
            format!("{}_distinct_steps_growth", num_distinct_steps),
            |b| {
                b.to_async(&runtime).iter(|| async move {
                    let initial_count = get_state_count();
                    for i in 0..num_distinct_steps {
                        let step_id = StepId::new(format!("leak-step-{}", i));
                        execute_step(step_id, 5000).await.unwrap();
                    }
                    let final_count = get_state_count();
                    let growth = final_count.saturating_sub(initial_count);
                    reset_all_state();
                    let after_reset = get_state_count();
                    (growth, after_reset)
                })
            },
        );
    }
    group.finish();
}

fn bench_memory_leak_error_accumulation(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("memory_leak_error");

    for num_errors in [100, 500, 1000] {
        group.bench_function(format!("{}_transient_errors", num_errors), |b| {
            b.to_async(&runtime).iter(|| async move {
                let initial_error_count = get_error_count();
                let initial_state_count = get_state_count();
                for i in 0..num_errors {
                    let step_id = StepId::new(format!("transient-step-{}", i));
                    let _ = execute_step(step_id, 5000).await;
                }
                let final_error_count = get_error_count();
                let final_state_count = get_state_count();
                let error_growth = final_error_count.saturating_sub(initial_error_count);
                let state_growth = final_state_count.saturating_sub(initial_state_count);
                reset_all_state();
                (error_growth, state_growth)
            })
        });
    }
    group.finish();
}

fn bench_memory_leak_sustained_load(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("memory_leak_sustained");

    group.bench_function("10000_ops_distinct_steps", |b| {
        b.to_async(&runtime).iter(|| async move {
            let initial_count = get_state_count();
            let mut total_growth = 0usize;
            for batch in 0..10 {
                for i in 0..1000 {
                    let step_id = StepId::new(format!("sustained-{}-{}", batch, i));
                    execute_step(step_id, 5000).await.unwrap();
                }
                let current_count = get_state_count();
                total_growth = current_count.saturating_sub(initial_count);
            }
            let final_count = get_state_count();
            reset_all_state();
            (total_growth, final_count)
        })
    });
    group.finish();
}

fn bench_memory_leak_concurrent_distinct(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("memory_leak_concurrent_distinct");

    for num_tasks in [50, 100, 200] {
        group.bench_function(format!("{}_concurrent_distinct", num_tasks), |b| {
            b.to_async(&runtime).iter(|| async move {
                let initial_count = get_state_count();
                let mut handles = Vec::with_capacity(num_tasks);
                for t in 0..num_tasks {
                    let step_id = StepId::new(format!("concurrent-leak-{}", t));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move { execute_step(step_id, 5000).await.unwrap() }
                    }));
                }
                for handle in handles {
                    black_box(handle.await.expect("task join"));
                }
                let final_count = get_state_count();
                let growth = final_count.saturating_sub(initial_count);
                reset_all_state();
                growth
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_execute_step_success_latency,
    bench_execute_step_concurrent_throughput,
    bench_execute_step_with_retry_throughput,
    bench_get_execution_status_no_contention,
    bench_get_execution_status_concurrent,
    bench_execute_step_mixed_workload,
    bench_execute_step_scaling,
    bench_cancel_execution_no_op,
    bench_reset_all_state,
    bench_execute_step_success_sequential,
    bench_execute_step_many_distinct_steps,
    bench_memory_leak_state_growth,
    bench_memory_leak_error_accumulation,
    bench_memory_leak_sustained_load,
    bench_memory_leak_concurrent_distinct,
);
criterion_main!(benches);
