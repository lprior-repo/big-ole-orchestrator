use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vo_core::saga::state::{
    apply_transition, CompensationState, CompensationTransitionError, CompensationTransitionEvent,
};

fn bench_apply_transition(c: &mut Criterion) {
    let mut group = c.benchmark_group("saga_transition");

    group.bench_function("pending_to_executing", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Pending),
                black_box(CompensationTransitionEvent::Start),
            ))
        })
    });

    group.bench_function("executing_to_completed", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Executing),
                black_box(CompensationTransitionEvent::Complete),
            ))
        })
    });

    group.bench_function("executing_to_failed", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Executing),
                black_box(CompensationTransitionEvent::Fail),
            ))
        })
    });

    group.bench_function("pending_to_failed", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Pending),
                black_box(CompensationTransitionEvent::Fail),
            ))
        })
    });

    group.bench_function("terminal_completed_rejects", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Completed),
                black_box(CompensationTransitionEvent::Start),
            ))
        })
    });

    group.bench_function("invalid_executing_start", |b| {
        b.iter(|| {
            black_box(apply_transition(
                black_box(CompensationState::Executing),
                black_box(CompensationTransitionEvent::Start),
            ))
        })
    });

    group.finish();
}

fn bench_full_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("saga_lifecycle");

    group.bench_function("pending_start_complete", |b| {
        b.iter(|| {
            let s1 = apply_transition(
                CompensationState::Pending,
                CompensationTransitionEvent::Start,
            )
            .unwrap();
            black_box(apply_transition(s1, CompensationTransitionEvent::Complete))
        })
    });

    group.bench_function("pending_start_fail", |b| {
        b.iter(|| {
            let s1 = apply_transition(
                CompensationState::Pending,
                CompensationTransitionEvent::Start,
            )
            .unwrap();
            black_box(apply_transition(s1, CompensationTransitionEvent::Fail))
        })
    });

    group.finish();
}

fn bench_is_terminal(c: &mut Criterion) {
    let mut group = c.benchmark_group("saga_is_terminal");

    group.bench_function("pending", |b| {
        b.iter(|| black_box(CompensationState::Pending.is_terminal()))
    });
    group.bench_function("completed", |b| {
        b.iter(|| black_box(CompensationState::Completed.is_terminal()))
    });
    group.bench_function("failed", |b| {
        b.iter(|| black_box(CompensationState::Failed.is_terminal()))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_apply_transition,
    bench_full_lifecycle,
    bench_is_terminal,
);
criterion_main!(benches);
