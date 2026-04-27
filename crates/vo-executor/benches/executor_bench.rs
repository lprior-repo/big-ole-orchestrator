use std::hint::black_box;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use vo_executor::scheduler::PriorityQueue;
use vo_executor::scheduler::{Job, JobId, Schedule};
use vo_executor::types::RetryPolicy;

fn make_job(id: u64, fire_at_ms: u64) -> Job {
    Job::new(
        JobId::new(id),
        format!("payload-{id}"),
        Schedule::OneShot { fire_at_ms },
    )
}

fn bench_retry_policy(c: &mut Criterion) {
    let mut group = c.benchmark_group("retry_policy");
    group.throughput(Throughput::Elements(1));

    let policy = RetryPolicy::new(5, 100, 2.0).unwrap();

    group.bench_function("calculate_backoff_attempt_1", |b| {
        b.iter(|| black_box(policy.calculate_backoff_delay(black_box(1))))
    });

    group.bench_function("calculate_backoff_attempt_5", |b| {
        b.iter(|| black_box(policy.calculate_backoff_delay(black_box(5))))
    });

    group.bench_function("calculate_backoff_attempt_20", |b| {
        b.iter(|| black_box(policy.calculate_backoff_delay(black_box(20))))
    });

    group.bench_function("policy_creation", |b| {
        b.iter(|| {
            black_box(RetryPolicy::new(
                black_box(5),
                black_box(100),
                black_box(2.0),
            ))
        })
    });

    group.bench_function("backoff_sequence_10", |b| {
        b.iter(|| {
            let mut total = 0u64;
            for attempt in 1..=10 {
                total += policy.calculate_backoff_delay(attempt);
            }
            black_box(total)
        })
    });

    group.finish();
}

fn bench_priority_queue(c: &mut Criterion) {
    let mut group = c.benchmark_group("priority_queue");
    group.throughput(Throughput::Elements(1));

    group.bench_function("push_1k", |b| {
        b.iter_batched(
            PriorityQueue::new,
            |mut pq| {
                for i in 0..1000u64 {
                    pq.push(make_job(i, i * 100), i * 100);
                }
                pq
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("push_pop_1k", |b| {
        b.iter_batched(
            || {
                let mut pq = PriorityQueue::new();
                for i in 0..1000u64 {
                    pq.push(make_job(i, i * 100), i * 100);
                }
                pq
            },
            |mut pq| {
                for _ in 0..1000 {
                    black_box(pq.pop());
                }
                pq
            },
            BatchSize::SmallInput,
        )
    });

    let populated: PriorityQueue = {
        let mut pq = PriorityQueue::new();
        for i in 0..1000u64 {
            pq.push(make_job(i, i * 100), i * 100);
        }
        pq
    };

    group.bench_function("due_jobs_1k_at_50000", |b| {
        b.iter(|| black_box(populated.due_jobs(black_box(50_000), black_box(100))))
    });

    group.bench_function("due_jobs_1k_all_due", |b| {
        b.iter(|| black_box(populated.due_jobs(black_box(u64::MAX), black_box(1000))))
    });

    group.bench_function("due_jobs_1k_none_due", |b| {
        b.iter(|| black_box(populated.due_jobs(black_box(0), black_box(100))))
    });

    group.bench_function("remove_existing", |b| {
        b.iter_batched(
            || {
                let mut pq = PriorityQueue::new();
                for i in 0..100u64 {
                    pq.push(make_job(i, i * 100), i * 100);
                }
                pq
            },
            |mut pq| {
                for i in 0..50u64 {
                    black_box(pq.remove(&JobId::new(i)));
                }
                pq
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, bench_retry_policy, bench_priority_queue,);
criterion_main!(benches);
