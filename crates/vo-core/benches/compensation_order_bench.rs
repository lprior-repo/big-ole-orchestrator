use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use std::collections::HashMap;
use vo_core::compensation_order::{
    compute_compensation_order, detect_cycle, filter_compensatable, validate_dependencies,
    CompensationNode, CompensationPolicy,
};

fn node(id: &str, deps: &[&str]) -> CompensationNode {
    CompensationNode {
        effect_id: id.to_string(),
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
    }
}

fn make_linear_chain(n: usize) -> Vec<CompensationNode> {
    (0..n)
        .rev()
        .map(|i| {
            if i == 0 {
                node(&format!("e{i}"), &[])
            } else {
                node(&format!("e{i}"), &[&format!("e{}", i - 1)])
            }
        })
        .collect()
}

fn make_diamond_graph(layers: usize) -> Vec<CompensationNode> {
    let mut nodes = Vec::new();
    for layer in (0..layers).rev() {
        if layer == layers - 1 {
            for i in 0..2_usize.pow(layer as u32) {
                nodes.push(node(&format!("e-{layer}-{i}"), &[]));
            }
        } else {
            let width = 2_usize.pow(layer as u32);
            let child_width = 2_usize.pow((layer + 1) as u32);
            for i in 0..width {
                let left_child = &format!("e-{next}-{left}", next = layer + 1, left = i * 2);
                let right_child = &format!("e-{next}-{right}", next = layer + 1, right = i * 2 + 1);
                nodes.push(node(&format!("e-{layer}-{i}"), &[left_child, right_child]));
            }
        }
    }
    nodes
}

fn make_independent_nodes(n: usize) -> Vec<CompensationNode> {
    (0..n).map(|i| node(&format!("e{i}"), &[])).collect()
}

fn bench_compute_compensation_order(c: &mut Criterion) {
    let mut group = c.benchmark_group("compensation_order_compute");

    for size in [1, 10, 50, 100] {
        let nodes = make_linear_chain(size);
        group.bench_function(format!("linear_chain_{size}"), |b| {
            b.iter_batched(
                || nodes.clone(),
                |n| black_box(compute_compensation_order(black_box(n))),
                BatchSize::SmallInput,
            )
        });
    }

    for size in [1, 10, 50, 100] {
        let nodes = make_independent_nodes(size);
        group.bench_function(format!("independent_{size}"), |b| {
            b.iter_batched(
                || nodes.clone(),
                |n| black_box(compute_compensation_order(black_box(n))),
                BatchSize::SmallInput,
            )
        });
    }

    let diamond = make_diamond_graph(5);
    group.bench_function("diamond_5_layers", |b| {
        b.iter_batched(
            || diamond.clone(),
            |n| black_box(compute_compensation_order(black_box(n))),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_detect_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("compensation_order_detect_cycle");

    for size in [10, 50, 100] {
        let nodes = make_linear_chain(size);
        group.bench_function(format!("no_cycle_chain_{size}"), |b| {
            b.iter(|| black_box(detect_cycle(black_box(&nodes))))
        });
    }

    let cyclic: Vec<CompensationNode> = (0..50)
        .map(|i| {
            let next = (i + 1) % 50;
            node(&format!("e{i}"), &[&format!("e{next}")])
        })
        .collect();
    group.bench_function("cyclic_50", |b| {
        b.iter(|| black_box(detect_cycle(black_box(&cyclic))))
    });

    group.finish();
}

fn bench_validate_dependencies(c: &mut Criterion) {
    let mut group = c.benchmark_group("compensation_order_validate");

    for size in [10, 50, 100] {
        let nodes = make_linear_chain(size);
        group.bench_function(format!("valid_chain_{size}"), |b| {
            b.iter(|| black_box(validate_dependencies(black_box(&nodes))))
        });
    }

    let with_unknown = {
        let mut nodes = make_linear_chain(50);
        let last = nodes.last_mut().unwrap();
        last.dependencies.push("nonexistent".to_string());
        nodes
    };
    group.bench_function("unknown_dep_50", |b| {
        b.iter(|| black_box(validate_dependencies(black_box(&with_unknown))))
    });

    group.finish();
}

fn bench_filter_compensatable(c: &mut Criterion) {
    let mut group = c.benchmark_group("compensation_order_filter");

    for size in [10, 50, 100] {
        let nodes = make_independent_nodes(size);
        let mut policies = HashMap::new();
        for i in 0..size {
            policies.insert(
                format!("e{i}"),
                if i % 3 == 0 {
                    CompensationPolicy::NotNeeded
                } else {
                    CompensationPolicy::Required
                },
            );
        }
        group.bench_function(format!("filter_{size}"), |b| {
            b.iter(|| {
                black_box(filter_compensatable(
                    black_box(&nodes),
                    black_box(&policies),
                ))
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compute_compensation_order,
    bench_detect_cycle,
    bench_validate_dependencies,
    bench_filter_compensatable,
);
criterion_main!(benches);
