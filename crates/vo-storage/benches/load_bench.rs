use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn storage_load_smoke_bench(c: &mut Criterion) {
    c.bench_function("storage_load_smoke", |b| {
        b.iter(|| black_box(2_u64.saturating_mul(2)))
    });
}

criterion_group!(benches, storage_load_smoke_bench);
criterion_main!(benches);
