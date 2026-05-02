//! Comprehensive tests for unused step detection.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::LintSeverity;

use super::check_unused_steps;
use super::graph::{DagGraph, Step};
use crate::LintCode;

#[test]
fn test_no_unused_steps_fully_connected() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "c".into(),
            is_entry: false,
        })
        .add_edge("a", "b")
        .add_edge("b", "c");

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "fully connected DAG should have no unused steps, got: {diagnostics:?}"
    );
}

#[test]
fn test_single_unused_step() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_edge("a", "b");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1, "expected exactly one unused step");
    assert_eq!(diagnostics[0].code, LintCode::L004);
    assert_eq!(diagnostics[0].message, "unused step: `c`");
    assert_eq!(
        diagnostics[0].suggestion,
        Some("Remove unused step or add edge from a reachable step".to_string())
    );
}

#[test]
fn test_multiple_unused_steps() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "c".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "d".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "e".into(),
            is_entry: false,
        })
        .add_edge("a", "b");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(
        diagnostics.len(),
        3,
        "expected three unused steps, got {}",
        diagnostics.len()
    );

    let messages: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(messages.iter().any(|m| *m == "unused step: `c`"));
    assert!(messages.iter().any(|m| *m == "unused step: `d`"));
    assert!(messages.iter().any(|m| *m == "unused step: `e`"));
}

#[test]
fn test_entry_node_not_flagged() {
    let graph = DagGraph::new().add_step(Step {
        name: "entry".to_string(),
        is_entry: true,
    });

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "entry node with no edges should not be flagged"
    );
}

#[test]
fn test_disconnected_subgraph() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "x".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "y".into(),
            is_entry: false,
        })
        .add_edge("a", "b")
        .add_edge("x", "y");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(
        diagnostics.len(),
        2,
        "disconnected subgraph nodes should each be flagged"
    );

    let names: Vec<&str> = diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(names.iter().any(|m| *m == "unused step: `x`"));
    assert!(names.iter().any(|m| *m == "unused step: `y`"));
}

#[test]
fn test_diamond_dag_no_false_positives() {
    // Diamond: A->B, A->C, B->D, C->D (A is entry)
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "c".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "d".into(),
            is_entry: false,
        })
        .add_edge("a", "b")
        .add_edge("a", "c")
        .add_edge("b", "d")
        .add_edge("c", "d");

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "diamond DAG should have no unused steps, got: {diagnostics:?}"
    );
}

#[test]
fn test_unused_step_diagnostic_includes_suggestion() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "orphan".into(),
            is_entry: false,
        })
        .add_edge("a", "b");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1);
    let diag = &diagnostics[0];
    assert!(
        diag.suggestion
            .as_ref()
            .is_some_and(|s| s.contains("Remove unused step")),
        "suggestion should contain actionable advice, got: {:?}",
        diag.suggestion
    );
}

#[test]
fn test_warning_severity() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "orphan".into(),
            is_entry: false,
        })
        .add_edge("a", "b");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].severity,
        LintSeverity::Warning,
        "unused steps should be Warning, not Error"
    );
}

#[test]
fn test_empty_graph() {
    let graph = DagGraph::new();
    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "empty graph should produce no diagnostics"
    );
}

#[test]
fn test_entry_only_graph() {
    let graph = DagGraph::new().add_step(Step {
        name: "entry".to_string(),
        is_entry: true,
    });
    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "single entry node should produce no diagnostics"
    );
}

#[test]
fn test_no_entry_node_returns_empty() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_edge("a", "b");

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "graph with no entry node should produce no diagnostics"
    );
}

#[test]
fn test_unused_steps_sorted_alphabetically() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "zebra".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "alpha".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "middle".into(),
            is_entry: false,
        })
        .add_edge("a", "zebra");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].message, "unused step: `alpha`");
    assert_eq!(diagnostics[1].message, "unused step: `middle`");
}

#[test]
fn test_lint_code_is_l004() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "orphan".into(),
            is_entry: false,
        })
        .add_edge("a", "a");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, LintCode::L004);
}

#[test]
fn test_deep_chain_no_unused() {
    let mut graph = DagGraph::new()
        .add_step(Step {
            name: "0".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "1".into(),
            is_entry: false,
        });
    for i in 2..=20 {
        graph = graph.add_step(Step {
            name: i.to_string(),
            is_entry: false,
        });
        graph = graph.add_edge((i - 1).to_string(), i.to_string());
    }

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "deep chain should have no unused steps"
    );
}

#[test]
fn test_multiple_entry_points_no_flag() {
    // Only first entry node is the actual entry
    let graph = DagGraph::new()
        .add_step(Step {
            name: "a".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "b".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "c".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "d".into(),
            is_entry: false,
        })
        .add_edge("a", "b")
        .add_edge("a", "c")
        .add_edge("b", "d");

    let diagnostics = check_unused_steps(&graph);
    assert!(
        diagnostics.is_empty(),
        "all nodes reachable from entry, no unused: {diagnostics:?}"
    );
}

#[test]
fn test_isolated_node_not_entry() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "entry".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "isolated".into(),
            is_entry: false,
        });

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "unused step: `isolated`");
}

#[test]
fn test_three_node_dag_with_unreachable_cleanup() {
    let graph = DagGraph::new()
        .add_step(Step {
            name: "start".into(),
            is_entry: true,
        })
        .add_step(Step {
            name: "middle".into(),
            is_entry: false,
        })
        .add_step(Step {
            name: "cleanup".into(),
            is_entry: false,
        })
        .add_edge("start", "middle");

    let diagnostics = check_unused_steps(&graph);
    assert_eq!(diagnostics.len(), 1, "cleanup node should be unreachable");
    assert_eq!(diagnostics[0].message, "unused step: `cleanup`");
    assert_eq!(diagnostics[0].code, LintCode::L004);
}

#[test]
fn test_self_referencing_node_produces_cycle_warning() {
    use crate::rules::cycles::check_cycles;
    use crate::LintCode;

    let graph = DagGraph::new()
        .add_step(Step {
            name: "self_ref".into(),
            is_entry: true,
        })
        .add_edge("self_ref", "self_ref");

    let diagnostics = check_cycles(&graph);
    assert_eq!(diagnostics.len(), 1, "self-referencing node should produce cycle diagnostic");
    assert_eq!(diagnostics[0].code, LintCode::L008);
}
