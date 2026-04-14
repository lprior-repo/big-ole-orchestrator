use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use vo_core::quadtree::{Point as QtPoint, Quadtree, AABB as QtAABB};
use vo_core::segment_tree::{LazySegmentTree, SegmentTree};

fn bench_segment_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("segment_tree");
    group.throughput(Throughput::Elements(1));

    let data: Vec<u64> = (0..10_000).collect();
    let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);

    group.bench_function("query_full_range_10k", |b| {
        b.iter(|| black_box(tree.query(black_box(0), black_box(10_000))))
    });

    group.bench_function("query_narrow_range_10k", |b| {
        b.iter(|| black_box(tree.query(black_box(500), black_box(600))))
    });

    group.bench_function("query_single_element_10k", |b| {
        b.iter(|| black_box(tree.query(black_box(5000), black_box(5001))))
    });

    group.bench_function("build_10k", |b| {
        b.iter(|| black_box(SegmentTree::from_slice(black_box(&data), |a, b| a + b, 0)))
    });

    let data_1k: Vec<u64> = (0..1_000).collect();
    group.bench_function("build_1k", |b| {
        b.iter(|| {
            black_box(SegmentTree::from_slice(
                black_box(&data_1k),
                |a, b| a + b,
                0,
            ))
        })
    });

    group.finish();
}

fn bench_segment_tree_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("segment_tree_update");

    group.bench_function("update_10k", |b| {
        b.iter_batched(
            || {
                let data: Vec<u64> = (0..10_000).collect();
                SegmentTree::from_slice(&data, |a, b| a + b, 0)
            },
            |mut tree| {
                for i in 0..100 {
                    black_box(tree.update(black_box(i), black_box(i as u64 * 2)));
                }
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_lazy_segment_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("lazy_segment_tree");
    group.throughput(Throughput::Elements(1));

    let data: Vec<u64> = vec![0u64; 10_000];
    fn merge(a: &u64, b: &u64) -> u64 {
        *a + *b
    }
    fn apply(val: &u64, update: &u64, _len: usize) -> u64 {
        *val + *update
    }
    fn compose(pending: &u64, new: &u64) -> u64 {
        *pending + *new
    }
    let identity = 0u64;

    group.bench_function("range_update_10k_wide", |b| {
        b.iter_batched(
            || LazySegmentTree::from_slice(&data, merge, identity, apply, compose),
            |mut tree| {
                black_box(tree.update_range(black_box(0), black_box(10_000), black_box(1)));
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("range_update_10k_narrow", |b| {
        b.iter_batched(
            || LazySegmentTree::from_slice(&data, merge, identity, apply, compose),
            |mut tree| {
                black_box(tree.update_range(black_box(100), black_box(200), black_box(5)));
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("range_query_after_updates", |b| {
        b.iter_batched(
            || {
                let mut tree = LazySegmentTree::from_slice(&data, merge, identity, apply, compose);
                tree.update_range(0, 5000, 3);
                tree.update_range(2000, 8000, 7);
                tree
            },
            |mut tree| {
                black_box(tree.query(black_box(1000), black_box(4000)));
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("build_10k", |b| {
        b.iter(|| {
            black_box(LazySegmentTree::from_slice(
                black_box(&data),
                merge,
                identity,
                apply,
                compose,
            ))
        })
    });

    group.finish();
}

fn bench_quadtree(c: &mut Criterion) {
    let mut group = c.benchmark_group("quadtree");

    let bounds = QtAABB::new(0.0, 0.0, 1000.0, 1000.0);

    group.bench_function("insert_10k_points", |b| {
        b.iter_batched(
            || Quadtree::new(bounds, 4, 20),
            |mut tree| {
                for i in 0..10_000u32 {
                    let x = (i % 100) as f64 * 10.0 + 5.0;
                    let y = (i / 100) as f64 * 10.0 + 5.0;
                    let _ = tree.insert(QtPoint::new(x, y, format!("p{i}")));
                }
                tree
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("insert_1k_points", |b| {
        b.iter_batched(
            || Quadtree::new(bounds, 4, 20),
            |mut tree| {
                for i in 0..1_000u32 {
                    let x = (i % 32) as f64 * 31.25;
                    let y = (i / 32) as f64 * 31.25;
                    let _ = tree.insert(QtPoint::new(x, y, format!("p{i}")));
                }
                tree
            },
            BatchSize::SmallInput,
        )
    });

    let mut populated = Quadtree::new(bounds, 4, 20);
    for i in 0..10_000u32 {
        let x = (i % 100) as f64 * 10.0 + 5.0;
        let y = (i / 100) as f64 * 10.0 + 5.0;
        let _ = populated.insert(QtPoint::new(x, y, format!("p{i}")));
    }

    group.throughput(Throughput::Elements(1));
    let query_region = QtAABB::new(200.0, 200.0, 400.0, 400.0);
    group.bench_function("query_10k_selectivity_4pct", |b| {
        b.iter(|| black_box(populated.query(black_box(query_region))))
    });

    let wide_query = QtAABB::new(0.0, 0.0, 600.0, 600.0);
    group.bench_function("query_10k_selectivity_36pct", |b| {
        b.iter(|| black_box(populated.query(black_box(wide_query))))
    });

    let full_query = QtAABB::new(0.0, 0.0, 1000.0, 1000.0);
    group.bench_function("query_10k_selectivity_100pct", |b| {
        b.iter(|| black_box(populated.query(black_box(full_query))))
    });

    group.finish();
}

fn bench_quadtree_capacity_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("quadtree_capacity");
    let bounds = QtAABB::new(0.0, 0.0, 1000.0, 1000.0);

    for capacity in [1, 4, 16, 64] {
        let label = format!("insert_1k_capacity_{capacity}");
        group.bench_function(&label, |b| {
            b.iter_batched(
                || Quadtree::new(bounds, capacity, 20),
                |mut tree| {
                    for i in 0..1_000u32 {
                        let x = (i % 32) as f64 * 31.25;
                        let y = (i / 32) as f64 * 31.25;
                        let _ = tree.insert(QtPoint::new(x, y, format!("p{i}")));
                    }
                    tree
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_segment_tree,
    bench_segment_tree_update,
    bench_lazy_segment_tree,
    bench_quadtree,
    bench_quadtree_capacity_comparison,
);
criterion_main!(benches);
