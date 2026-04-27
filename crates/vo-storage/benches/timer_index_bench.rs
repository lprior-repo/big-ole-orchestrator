use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn timer_index_api_smoke_bench(c: &mut Criterion) {
    c.bench_function("timer_index_api_smoke", |b| {
        b.iter(|| black_box(1_u64.saturating_add(1)))
    });
}

criterion_group!(benches, timer_index_api_smoke_bench);
criterion_main!(benches);
