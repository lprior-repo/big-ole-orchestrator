use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;
use vo_executor::{
    cancel_execution, execute_step, execute_step_with_retry, get_execution_status,
    reset_all_state, RetryPolicy, StepId,
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
            b.to_async(&runtime).iter(move |_| async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_tasks);
                for t in 0..num_tasks {
                    let step_id = StepId::new(format!("workflow-step-{}", t % 10));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move {
                            execute_step(step_id, 5000).await.unwrap()
                        }
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
        b.to_async(&runtime).iter(|_| async move {
            let start = std::time::Instant::now();
            for _ in 0..10 {
                black_box(
                    execute_step_with_retry(step_id.clone(), 5000, policy.clone()).await,
                )
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
            b.to_async(&runtime).iter(move |_| async move {
                let step_id = StepId::new("step-1".to_string());
                execute_step(step_id.clone(), 5000).await.unwrap();

                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(num_readers);
                for _ in 0..num_readers {
                    let sid = step_id.clone();
                    handles.push(tokio::spawn(async move {
                        get_execution_status(&sid)
                    }));
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
        b.to_async(&runtime).iter(|_| async move {
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
                    async move {
                        execute_step(step_id, 5000).await
                    }
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
            b.to_async(&runtime).iter(move |_| async move {
                let start = std::time::Instant::now();
                let mut handles = Vec::with_capacity(batch_size);
                for t in 0..batch_size {
                    let step_id = StepId::new(format!("step-{}-{}", batch_size, t));
                    handles.push(tokio::spawn({
                        let step_id = step_id.clone();
                        async move {
                            execute_step(step_id, 5000).await.unwrap()
                        }
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
            let step_id = StepId::new(format!("step-{}", i));
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
            b.to_async(&runtime).iter(move |_| async move {
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
);
criterion_main!(benches);
