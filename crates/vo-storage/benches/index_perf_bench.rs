use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn index_api_smoke_bench(c: &mut Criterion) {
    c.bench_function("index_api_smoke", |b| {
        b.iter(|| black_box(1_u64.saturating_add(1)))
    });
}

criterion_group!(benches, index_api_smoke_bench);
criterion_main!(benches);
