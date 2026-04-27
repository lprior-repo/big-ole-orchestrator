use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn replay_load_smoke_bench(c: &mut Criterion) {
    c.bench_function("replay_load_smoke", |b| {
        b.iter(|| black_box(3_u64.saturating_pow(2)))
    });
}

criterion_group!(benches, replay_load_smoke_bench);
criterion_main!(benches);
