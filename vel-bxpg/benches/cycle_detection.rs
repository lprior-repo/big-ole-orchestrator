//! Benchmarks for vel-bxpg
//!
//! Performance-critical paths from test-plan:
//! - detect_cycle on graphs of various sizes
//! - Dag::build() validation overhead
//! - JSON serialization of WorkflowDefinition

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vel_bxpg::{Dag, DagNode, Edge, WorkflowDefinition, detect_cycle};

fn benchmark_detect_cycle_linear_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("detect_cycle_linear_chain");

    for size in [10, 50, 100, 500, 1000] {
        group.bench_function(format!("size_{}", size), |b| {
            // Create a linear chain: 0 -> 1 -> 2 -> ... -> N-1
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for i in 0..size {
                nodes.push(DagNode {
                    name: format!("Node{}", i),
                    retry_policy: None,
                });
                if i > 0 {
                    edges.push(Edge {
                        source_node: format!("Node{}", i - 1),
                        target_node: format!("Node{}", i),
                        condition: None,
                    });
                }
            }

            b.iter(|| {
                let result = detect_cycle(black_box(&nodes), black_box(&edges));
                black_box(result)
            });
        });
    }

    group.finish();
}

fn benchmark_detect_cycle_with_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("detect_cycle_with_cycle");

    for size in [10, 50, 100] {
        group.bench_function(format!("size_{}", size), |b| {
            // Create a cycle: 0 -> 1 -> 2 -> ... -> N-1 -> 0
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for i in 0..size {
                nodes.push(DagNode {
                    name: format!("Node{}", i),
                    retry_policy: None,
                });
                let target = if i == size - 1 { 0 } else { i + 1 };
                edges.push(Edge {
                    source_node: format!("Node{}", i),
                    target_node: format!("Node{}", target),
                    condition: None,
                });
            }

            b.iter(|| {
                let result = detect_cycle(black_box(&nodes), black_box(&edges));
                black_box(result)
            });
        });
    }

    group.finish();
}

fn benchmark_dag_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_build");

    for size in [10, 50, 100, 500] {
        group.bench_function(format!("size_{}", size), |b| {
            b.iter(|| {
                let mut dag = Dag::new("benchmark-workflow");
                for i in 0..size {
                    dag.add_node(DagNode {
                        name: format!("Node{}", i),
                        retry_policy: None,
                    });
                }
                // Create a linear chain
                for i in 0..size - 1 {
                    dag.connect(format!("Node{}", i), format!("Node{}", i + 1));
                }

                let result = dag.build();
                black_box(result)
            });
        });
    }

    group.finish();
}

fn benchmark_workflow_definition_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_serialization");

    for size in [10, 50, 100, 500] {
        group.bench_function(format!("size_{}", size), |b| {
            // Create workflow with specified number of nodes and edges
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for i in 0..size {
                nodes.push(DagNode {
                    name: format!("Node{}", i),
                    retry_policy: None,
                });
                if i > 0 {
                    edges.push(Edge {
                        source_node: format!("Node{}", i - 1),
                        target_node: format!("Node{}", i),
                        condition: None,
                    });
                }
            }

            let workflow = WorkflowDefinition {
                workflow_name: "benchmark-workflow".into(),
                nodes,
                edges,
            };

            b.iter(|| {
                let result = serde_json::to_string(black_box(&workflow));
                black_box(result)
            });
        });
    }

    group.finish();
}

fn benchmark_workflow_definition_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_deserialization");

    for size in [10, 50, 100, 500] {
        group.bench_function(format!("size_{}", size), |b| {
            // Pre-serialize the workflow to avoid including serialization cost
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for i in 0..size {
                nodes.push(DagNode {
                    name: format!("Node{}", i),
                    retry_policy: None,
                });
                if i > 0 {
                    edges.push(Edge {
                        source_node: format!("Node{}", i - 1),
                        target_node: format!("Node{}", i),
                        condition: None,
                    });
                }
            }

            let workflow = WorkflowDefinition {
                workflow_name: "benchmark-workflow".into(),
                nodes,
                edges,
            };

            let json = serde_json::to_string(&workflow).unwrap();

            b.iter(|| {
                let result = serde_json::from_str::<WorkflowDefinition>(black_box(&json));
                black_box(result)
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_detect_cycle_linear_chain,
    benchmark_detect_cycle_with_cycle,
    benchmark_dag_build,
    benchmark_workflow_definition_serialization,
    benchmark_workflow_definition_deserialization
);

criterion_main!(benches);
