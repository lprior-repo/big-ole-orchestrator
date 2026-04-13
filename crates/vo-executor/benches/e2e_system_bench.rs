use criterion::{
    black_box, criterion_group, criterion_main, Criterion, Throughput,
};
use std::time::Instant;
use tokio::runtime::Runtime;
use vo_executor::{
    execute_step, execute_step_with_retry, get_execution_status, get_state_count,
    reset_all_state, RetryPolicy, StepId,
};

fn rt() -> Runtime {
    Runtime::new().expect("create runtime for benchmark")
}

fn fresh_step_id() -> StepId {
    StepId::new(format!("step-{}", uuid::Uuid::new_v4()))
}

fn bench_e2e_throughput_single_step(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_throughput_single_step");
    group.throughput(Throughput::Elements(1));

    for num_concurrent in [1, 10, 50, 100, 200] {
        group.bench_function(format!("{}_concurrent", num_concurrent), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = Instant::now();
                let mut handles = Vec::with_capacity(num_concurrent);
                for _ in 0..num_concurrent {
                    let step_id = fresh_step_id();
                    handles.push(tokio::spawn(async move {
                        execute_step(step_id, 5000).await
                    }));
                }
                let mut successes = 0u64;
                let mut failures = 0u64;
                for handle in handles {
                    match handle.await {
                        Ok(Ok(_)) => successes += 1,
                        Ok(Err(_)) => failures += 1,
                        Err(_) => failures += 1,
                    }
                }
                let elapsed = start.elapsed();
                let throughput = (successes as f64) / elapsed.as_secs_f64();
                (throughput, successes, failures, elapsed)
            })
        });
    }
    group.finish();
}

fn bench_e2e_throughput_mixed_workload(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_throughput_mixed_workload");
    group.throughput(Throughput::Elements(1));

    for batch_size in [50, 100, 200, 500] {
        group.bench_function(format!("batch_{}", batch_size), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = Instant::now();
                let mut handles = Vec::with_capacity(batch_size);
                for i in 0..batch_size {
                    let (step_name, delay) = match i % 5 {
                        0 => ("step-good", 5000),
                        1 => ("step-fail", 5000),
                        2 => ("step-retry", 5000),
                        3 => ("step-timeout", 100),
                        _ => ("step-normal", 5000),
                    };
                    let step_id = StepId::new(format!("{}-{}", step_name, uuid::Uuid::new_v4()));
                    handles.push(tokio::spawn(async move {
                        execute_step(step_id, delay).await
                    }));
                }
                let mut successes = 0u64;
                let mut failures = 0u64;
                for handle in handles {
                    match handle.await {
                        Ok(Ok(_)) => successes += 1,
                        Ok(Err(_)) => failures += 1,
                        Err(_) => failures += 1,
                    }
                }
                let elapsed = start.elapsed();
                let throughput = (batch_size as f64) / elapsed.as_secs_f64();
                (throughput, successes, failures)
            })
        });
    }
    group.finish();
}

fn bench_e2e_throughput_sustained_load(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_throughput_sustained");
    group.throughput(Throughput::Elements(1));

    let test_config = [
        (1000, 10),
        (5000, 50),
        (10000, 100),
    ];

    for (total_ops, concurrent) in test_config {
        group.bench_function(format!("{}_ops_{}_concurrent", total_ops, concurrent), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = Instant::now();
                let mut total_completed = 0u64;
                let mut total_successes = 0u64;
                let mut total_failures = 0u64;

                while total_completed < total_ops as u64 {
                    let batch_size = concurrent.min(total_ops as usize - total_completed as usize);
                    let mut handles = Vec::with_capacity(batch_size);
                    for _ in 0..batch_size {
                        let step_id = fresh_step_id();
                        handles.push(tokio::spawn(async move {
                            execute_step(step_id, 5000).await
                        }));
                    }
                    for handle in handles {
                        total_completed += 1;
                        match handle.await {
                            Ok(Ok(_)) => total_successes += 1,
                            Ok(Err(_)) => total_failures += 1,
                            Err(_) => total_failures += 1,
                        }
                    }
                }
                let elapsed = start.elapsed();
                let throughput = (total_completed as f64) / elapsed.as_secs_f64();
                (throughput, total_successes, total_failures, elapsed)
            })
        });
    }
    group.finish();
}

fn bench_e2e_latency_percentiles_single_step(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_latency_percentiles_single");
    group.throughput(Throughput::Elements(1));

    for num_ops in [100, 500, 1000] {
        group.bench_function(format!("{}_ops", num_ops), |b| {
            b.to_async(&runtime).iter(move || async move {
                let mut latencies = Vec::with_capacity(num_ops);
                for _ in 0..num_ops {
                    let step_id = fresh_step_id();
                    let start = Instant::now();
                    let _ = execute_step(step_id, 5000).await;
                    let elapsed = start.elapsed();
                    latencies.push(elapsed);
                }
                latencies.sort();
                let p50 = latencies[num_ops / 2];
                let p95 = latencies[(num_ops as f64 * 0.95) as usize];
                let p99 = latencies[(num_ops as f64 * 0.99) as usize];
                let p999 = latencies[(num_ops as f64 * 0.999) as usize];
                (p50, p95, p99, p999)
            })
        });
    }
    group.finish();
}

fn bench_e2e_latency_percentiles_concurrent(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_latency_percentiles_concurrent");
    group.throughput(Throughput::Elements(1));

    for num_tasks in [10, 50, 100] {
        group.bench_function(format!("{}_tasks", num_tasks), |b| {
            b.to_async(&runtime).iter(move || async move {
                let mut all_latencies = Vec::with_capacity(num_tasks * 10);
                for batch in 0..10 {
                    let mut handles = Vec::with_capacity(num_tasks);
                    for t in 0..num_tasks {
                        let step_id = StepId::new(format!("batch-{}-task-{}-{}", batch, t, uuid::Uuid::new_v4()));
                        handles.push(tokio::spawn({
                            let step_id = step_id.clone();
                            async move {
                                let start = Instant::now();
                                execute_step(step_id, 5000).await?;
                                Ok::<_, vo_executor::ExecuteNodeError>(start.elapsed())
                            }
                        }));
                    }
                    for handle in handles {
                        if let Ok(Ok(elapsed)) = handle.await {
                            all_latencies.push(elapsed);
                        }
                    }
                }
                all_latencies.sort();
                let len = all_latencies.len();
                if len == 0 {
                    return (std::time::Duration::ZERO, std::time::Duration::ZERO, std::time::Duration::ZERO, std::time::Duration::ZERO);
                }
                let p50 = all_latencies[len / 2];
                let p95 = all_latencies[((len as f64) * 0.95) as usize].min(all_latencies[len - 1]);
                let p99 = all_latencies[((len as f64) * 0.99) as usize].min(all_latencies[len - 1]);
                let p999 = all_latencies[((len as f64) * 0.999) as usize].min(all_latencies[len - 1]);
                (p50, p95, p99, p999)
            })
        });
    }
    group.finish();
}

fn bench_e2e_latency_percentiles_mixed_workload(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_latency_percentiles_mixed");
    group.throughput(Throughput::Elements(1));

    for num_ops in [100, 500] {
        group.bench_function(format!("{}_ops", num_ops), |b| {
            b.to_async(&runtime).iter(move || async move {
                let mut latencies = Vec::with_capacity(num_ops);
                for i in 0..num_ops {
                    let (delay, expect_success) = match i % 5 {
                        0 => (5000, true),
                        1 => (5000, false),
                        2 => (5000, true),
                        3 => (100, false),
                        _ => (5000, true),
                    };
                    let step_id = StepId::new(format!("mixed-{}-{}", i, uuid::Uuid::new_v4()));
                    let start = Instant::now();
                    let result = execute_step(step_id, delay).await;
                    let elapsed = start.elapsed();
                    if expect_success {
                        let _ = result;
                    }
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

fn bench_e2e_retry_throughput(c: &mut Criterion) {
    let runtime = rt();
    let base_policy = RetryPolicy::new(3, 10, 2.0).unwrap();
    let mut group = c.benchmark_group("e2e_throughput_with_retry");
    group.throughput(Throughput::Elements(1));

    for batch_size in [10, 50, 100] {
        group.bench_function(format!("batch_{}", batch_size), |b| {
            b.to_async(&runtime).iter({
                let policy = base_policy.clone();
                move || {
                    let policy = policy.clone();
                    async move {
                        let start = Instant::now();
                        let mut handles = Vec::with_capacity(batch_size);
                        for i in 0..batch_size {
                            let step_id = StepId::new(format!("retry-{}-{}", i, uuid::Uuid::new_v4()));
                            let policy = policy.clone();
                            handles.push(tokio::spawn({
                                let step_id = step_id.clone();
                                async move {
                                    execute_step_with_retry(step_id, 5000, policy).await
                                }
                            }));
                        }
                        let mut successes = 0u64;
                        let mut failures = 0u64;
                        for handle in handles {
                            match handle.await {
                                Ok(Ok(_)) => successes += 1,
                                Ok(Err(_)) => failures += 1,
                                Err(_) => failures += 1,
                            }
                        }
                        let elapsed = start.elapsed();
                        let throughput = (successes as f64) / elapsed.as_secs_f64();
                        (throughput, successes, failures, elapsed)
                    }
                }
            })
        });
    }
    group.finish();
}

fn bench_e2e_state_read_throughput(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_throughput_state_read");
    group.throughput(Throughput::Elements(1));

    for num_readers in [1, 10, 50, 100] {
        group.bench_function(format!("{}_readers", num_readers), |b| {
            b.to_async(&runtime).iter(move || async move {
                for i in 0..100 {
                    let step_id = StepId::new(format!("bench-state-read-{}", i));
                    execute_step(step_id, 5000).await.unwrap();
                }

                let start = Instant::now();
                let mut handles = Vec::with_capacity(num_readers);
                for i in 0..num_readers {
                    let step_id = StepId::new(format!("bench-state-read-{}", i % 100));
                    handles.push(tokio::spawn(async move {
                        get_execution_status(&step_id)
                    }));
                }
                for handle in handles {
                    let _ = black_box(handle.await);
                }
                reset_all_state();
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_e2e_concurrent_read_write(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_concurrent_read_write");
    group.throughput(Throughput::Elements(1));

    let test_mix = [
        (10, 10),
        (50, 50),
        (100, 100),
    ];

    for (num_writers, num_readers) in test_mix {
        group.bench_function(format!("{}_writes_{}_reads", num_writers, num_readers), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = Instant::now();

                let mut write_handles = Vec::with_capacity(num_writers);
                for i in 0..num_writers {
                    let step_id = StepId::new(format!("write-{}-{}", i, uuid::Uuid::new_v4()));
                    write_handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move {
                            execute_step(step_id, 5000).await
                        }
                    }));
                }

                let mut read_handles = Vec::with_capacity(num_readers);
                for i in 0..num_readers {
                    let step_id = StepId::new(format!("read-{}", i));
                    read_handles.push(tokio::spawn(async move {
                        get_execution_status(&step_id)
                    }));
                }

                let mut write_successes = 0u64;
                let mut write_failures = 0u64;
                for handle in write_handles {
                    match handle.await {
                        Ok(Ok(_)) => write_successes += 1,
                        Ok(Err(_)) => write_failures += 1,
                        Err(_) => write_failures += 1,
                    }
                }

                let mut read_count = 0u64;
                for handle in read_handles {
                    let _ = black_box(handle.await);
                    read_count += 1;
                }

                let elapsed = start.elapsed();
                let total_ops = (write_successes + write_failures + read_count) as f64;
                let throughput = total_ops / elapsed.as_secs_f64();
                (throughput, write_successes, write_failures, read_count, elapsed)
            })
        });
    }
    group.finish();
}

fn bench_e2e_memory_state_growth(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_resource_state_growth");

    for num_steps in [100, 500, 1000, 5000] {
        group.bench_function(format!("{}_steps", num_steps), |b| {
            b.to_async(&runtime).iter(|| async move {
                let initial_count = get_state_count();
                for i in 0..num_steps {
                    let step_id = StepId::new(format!("growth-{}-{}", i, uuid::Uuid::new_v4()));
                    execute_step(step_id, 5000).await.unwrap();
                }
                let final_count = get_state_count();
                let growth = final_count.saturating_sub(initial_count);
                reset_all_state();
                let after_reset = get_state_count();
                (growth, final_count, after_reset)
            })
        });
    }
    group.finish();
}

fn bench_e2e_memory_sustained_operations(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_resource_sustained_memory");

    group.bench_function("10000_ops_memory_profile", |b| {
        b.to_async(&runtime).iter(|| async move {
            let initial_count = get_state_count();
            let mut peaks = Vec::with_capacity(10);
            
            for batch in 0..10 {
                for i in 0..1000 {
                    let step_id = StepId::new(format!("sustained-{}-{}", batch, i));
                    execute_step(step_id, 5000).await.unwrap();
                }
                let current_count = get_state_count();
                peaks.push(current_count);
            }
            
            let final_count = get_state_count();
            let max_peak = peaks.into_iter().max().unwrap_or(0);
            reset_all_state();
            
            (initial_count, final_count, max_peak, final_count.saturating_sub(initial_count))
        })
    });
    group.finish();
}

fn bench_e2e_error_accumulation(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_resource_error_accumulation");

    for num_errors in [100, 500, 1000] {
        group.bench_function(format!("{}_errors", num_errors), |b| {
            b.to_async(&runtime).iter(|| async move {
                let initial_error_count = vo_executor::get_error_count();
                let initial_state_count = get_state_count();
                
                for i in 0..num_errors {
                    let step_id = StepId::new(format!("error-{}-{}", i, uuid::Uuid::new_v4()));
                    let _ = execute_step(step_id, 5000).await;
                }
                
                let final_error_count = vo_executor::get_error_count();
                let final_state_count = get_state_count();
                
                let error_growth = final_error_count.saturating_sub(initial_error_count);
                let state_growth = final_state_count.saturating_sub(initial_state_count);
                
                reset_all_state();
                
                (error_growth, state_growth, final_error_count, final_state_count)
            })
        });
    }
    group.finish();
}

fn bench_e2e_high_contention(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_high_contention");

    for num_tasks in [100, 200, 500, 1000] {
        group.bench_function(format!("{}_tasks_same_step", num_tasks), |b| {
            b.to_async(&runtime).iter(move || async move {
                let shared_step_id = StepId::new("contested-step".to_string());
                let start = Instant::now();
                let mut handles = Vec::with_capacity(num_tasks);
                
                for _ in 0..num_tasks {
                    let step_id = shared_step_id.clone();
                    handles.push(tokio::spawn(async move {
                        execute_step(step_id, 5000).await
                    }));
                }
                
                let mut successes = 0u64;
                let mut failures = 0u64;
                for handle in handles {
                    match handle.await {
                        Ok(Ok(_)) => successes += 1,
                        Ok(Err(_)) => failures += 1,
                        Err(_) => failures += 1,
                    }
                }
                
                let elapsed = start.elapsed();
                let throughput = (successes + failures) as f64 / elapsed.as_secs_f64();
                (throughput, successes, failures, elapsed)
            })
        });
    }
    group.finish();
}

fn bench_e2e_burst_load(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_burst_load");

    let burst_sizes = [50, 100, 200, 500];
    let num_bursts = 10;

    for burst_size in burst_sizes {
        group.bench_function(format!("{}_bursts_of_{}", num_bursts, burst_size), |b| {
            b.to_async(&runtime).iter(move || async move {
                let start = Instant::now();
                let mut total_successes = 0u64;
                let mut total_failures = 0u64;

                for _ in 0..num_bursts {
                    let mut handles = Vec::with_capacity(burst_size);
                    for _ in 0..burst_size {
                        let step_id = fresh_step_id();
                        handles.push(tokio::spawn(async move {
                            execute_step(step_id, 5000).await
                        }));
                    }

                    for handle in handles {
                        match handle.await {
                            Ok(Ok(_)) => total_successes += 1,
                            Ok(Err(_)) => total_failures += 1,
                            Err(_) => total_failures += 1,
                        }
                    }
                }

                let elapsed = start.elapsed();
                let total_ops = total_successes + total_failures;
                let throughput = (total_ops as f64) / elapsed.as_secs_f64();
                (throughput, total_successes, total_failures, elapsed)
            })
        });
    }
    group.finish();
}

fn bench_e2e_cold_start_latency(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_cold_start_latency");

    group.bench_function("single_fresh_step", |b| {
        b.to_async(&runtime).iter(|| async move {
            reset_all_state();
            let step_id = fresh_step_id();
            let start = Instant::now();
            let _ = execute_step(step_id, 5000).await;
            start.elapsed()
        })
    });

    for num_steps in [10, 50, 100] {
        group.bench_function(format!("{}_fresh_steps_after_reset", num_steps), |b| {
            b.to_async(&runtime).iter(move || async move {
                reset_all_state();
                let start = Instant::now();
                for i in 0..num_steps {
                    let step_id = StepId::new(format!("cold-start-{}-{}", i, uuid::Uuid::new_v4()));
                    let _ = execute_step(step_id, 5000).await;
                }
                start.elapsed()
            })
        });
    }
    group.finish();
}

fn bench_e2e_warm_state_latency(c: &mut Criterion) {
    let runtime = rt();
    let mut group = c.benchmark_group("e2e_warm_state_latency");

    runtime.block_on(async {
        for i in 0..1000 {
            let step_id = StepId::new(format!("warm-{}-{}", i, uuid::Uuid::new_v4()));
            execute_step(step_id, 5000).await.unwrap();
        }
    });

    group.bench_function("1000_steps_warm_state", |b| {
        b.to_async(&runtime).iter(move || async move {
            let start = Instant::now();
            for i in 0..1000 {
                let step_id = StepId::new(format!("warm-{}-{}", i, uuid::Uuid::new_v4()));
                execute_step(step_id, 5000).await.unwrap();
            }
            start.elapsed()
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_e2e_throughput_single_step,
    bench_e2e_throughput_mixed_workload,
    bench_e2e_throughput_sustained_load,
    bench_e2e_latency_percentiles_single_step,
    bench_e2e_latency_percentiles_concurrent,
    bench_e2e_latency_percentiles_mixed_workload,
    bench_e2e_retry_throughput,
    bench_e2e_state_read_throughput,
    bench_e2e_concurrent_read_write,
    bench_e2e_memory_state_growth,
    bench_e2e_memory_sustained_operations,
    bench_e2e_error_accumulation,
    bench_e2e_high_contention,
    bench_e2e_burst_load,
    bench_e2e_cold_start_latency,
    bench_e2e_warm_state_latency,
);
criterion_main!(benches);