use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vo_actor::reanimator::{
    calculate_batch_size, check_resume_budget, filter_timers_by_fairness, validate_timer_record,
    FairnessBudget, TimerRecord,
};
use vo_types::{InstanceId, TimestampMs};

fn make_timer(id_str: &str, fire_at: u64, scheduled_at: u64) -> TimerRecord {
    TimerRecord::new(
        InstanceId::parse(id_str).unwrap(),
        TimestampMs::try_from(fire_at).unwrap(),
        None,
        TimestampMs::try_from(scheduled_at).unwrap(),
    )
}

fn make_timers(count: usize) -> Vec<TimerRecord> {
    (0..count)
        .map(|i| {
            let id = format!("01H5JYV4XHGSR2F8KZ9BWNRFM{:02X}", i % 256);
            make_timer(&id, 1000 + i as u64, 500)
        })
        .collect()
}

fn bench_filter_timers_by_fairness(c: &mut Criterion) {
    let timers = make_timers(100);
    let budget = FairnessBudget::with_limits(5, 50);
    c.bench_function("filter_timers_by_fairness_100", |b| {
        b.iter(|| {
            black_box(filter_timers_by_fairness(
                timers.clone(),
                black_box(&budget),
            ))
        })
    });
}

fn bench_calculate_batch_size(c: &mut Criterion) {
    c.bench_function("calculate_batch_size", |b| {
        b.iter(|| {
            black_box(calculate_batch_size(
                black_box(50),
                black_box(100),
                black_box(30),
            ))
        })
    });
}

fn bench_validate_timer_record(c: &mut Criterion) {
    let record = make_timer("01H5JYV4XHGSR2F8KZ9BWNRFMA", 1000, 500);
    c.bench_function("validate_timer_record_valid", |b| {
        b.iter(|| black_box(validate_timer_record(black_box(&record))))
    });
}

fn bench_check_resume_budget(c: &mut Criterion) {
    let budget = FairnessBudget::with_limits(5, 50);
    let instance_id = InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap();
    c.bench_function("check_resume_budget", |b| {
        b.iter(|| {
            black_box(check_resume_budget(
                black_box(&instance_id),
                black_box(&budget),
            ))
        })
    });
}

criterion_group!(
    benches,
    bench_filter_timers_by_fairness,
    bench_calculate_batch_size,
    bench_validate_timer_record,
    bench_check_resume_budget,
);
criterion_main!(benches);
