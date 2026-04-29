use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use rand::Rng;
use vo_common::structures::{Bounds, Octree, Vec3};

fn bench_octree_coplanar(c: &mut Criterion) {
    let mut group = c.benchmark_group("octree_coplanar");
    group.throughput(Throughput::Elements(1));

    let bounds = Bounds::new(Vec3::new(-100.0, -100.0, -100.0), Vec3::new(100.0, 100.0, 100.0));

    group.bench_function("insert_10k_coplanar_z0", |b| {
        b.iter_batched(
            || Octree::new(bounds),
            |mut tree| {
                for i in 0..10_000u32 {
                    let x = (i % 100) as f64 - 50.0;
                    let y = (i / 100) as f64 - 50.0;
                    let z = 0.0;
                    let _ = tree.insert(black_box(Vec3::new(x, y, z)), black_box(i));
                }
                tree
            },
            BatchSize::SmallInput,
        )
    });

    let mut populated = Octree::new(bounds);
    for i in 0..10_000u32 {
        let x = (i % 100) as f64 - 50.0;
        let y = (i / 100) as f64 - 50.0;
        let z = 0.0;
        let _ = populated.insert(Vec3::new(x, y, z), i);
    }

    let query_bounds = Bounds::new(Vec3::new(-25.0, -25.0, -1.0), Vec3::new(25.0, 25.0, 1.0));
    group.bench_function("query_10k_coplanar_selectivity_6.25pct", |b| {
        b.iter(|| black_box(populated.query_range(black_box(&query_bounds))))
    });

    let wide_query = Bounds::new(Vec3::new(-50.0, -50.0, -1.0), Vec3::new(50.0, 50.0, 1.0));
    group.bench_function("query_10k_coplanar_selectivity_25pct", |b| {
        b.iter(|| black_box(populated.query_range(black_box(&wide_query))))
    });

    let full_query = Bounds::new(Vec3::new(-100.0, -100.0, -100.0), Vec3::new(100.0, 100.0, 100.0));
    group.bench_function("query_10k_coplanar_selectivity_100pct", |b| {
        b.iter(|| black_box(populated.query_range(black_box(&full_query))))
    });

    group.finish();
}

fn bench_octree_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("octree_random");
    group.throughput(Throughput::Elements(1));

    let bounds = Bounds::new(Vec3::new(-100.0, -100.0, -100.0), Vec3::new(100.0, 100.0, 100.0));

    group.bench_function("insert_10k_random", |b| {
        b.iter_batched(
            || {
                let mut rng = rand::thread_rng();
                Octree::new(bounds)
            },
            |mut tree| {
                let mut rng = rand::thread_rng();
                for i in 0..10_000u32 {
                    let x: f64 = rng.gen_range(-100.0..100.0);
                    let y: f64 = rng.gen_range(-100.0..100.0);
                    let z: f64 = rng.gen_range(-100.0..100.0);
                    let _ = tree.insert(black_box(Vec3::new(x, y, z)), black_box(i));
                }
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(benches, bench_octree_coplanar, bench_octree_random);
criterion_main!(benches);