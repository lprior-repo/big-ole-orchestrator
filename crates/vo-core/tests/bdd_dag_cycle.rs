//! BDD tests for DAG cycle detection.
//!
//! Given-When-Then style validation of compensation order cycle detection.

use vo_core::compensation_order::{detect_cycle, CompensationNode};

fn node(id: &str, deps: &[&str]) -> CompensationNode {
    CompensationNode {
        effect_id: id.to_string(),
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
    }
}

fn given_nodes<'a>(descriptions: &[(&'a str, &'a [&'a str])]) -> Vec<CompensationNode> {
    descriptions
        .iter()
        .map(|(id, deps)| node(id, deps))
        .collect()
}

fn when_detect_cycle(nodes: &[CompensationNode]) -> Option<Vec<String>> {
    detect_cycle(nodes)
}

fn then_no_cycle(result: Option<Vec<String>>) -> bool {
    result.is_none()
}

fn then_has_cycle(result: Option<Vec<String>>) -> bool {
    result.is_some()
}

fn then_cycle_contains(result: Option<Vec<String>>, node_id: &str) -> bool {
    result
        .as_ref()
        .map(|cycle| cycle.iter().any(|n| n == node_id))
        .unwrap_or(false)
}

fn then_cycle_length(result: Option<Vec<String>>, expected: usize) -> bool {
    result.map(|cycle| cycle.len() == expected).unwrap_or(false)
}

#[test]
fn dag_with_no_cycles_returns_none() {
    let nodes = given_nodes(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
    let result = when_detect_cycle(&nodes);
    assert!(then_no_cycle(result), "DAG should have no cycle");
}

#[test]
fn dag_with_diamond_dependency_returns_none() {
    let nodes = given_nodes(&[("d", &[]), ("b", &["d"]), ("c", &["d"]), ("a", &["b", "c"])]);
    let result = when_detect_cycle(&nodes);
    assert!(then_no_cycle(result), "Diamond graph is still a DAG");
}

#[test]
fn self_referencing_node_is_cycle() {
    let nodes = given_nodes(&[("a", &["a"])]);
    let result = when_detect_cycle(&nodes);
    assert!(then_has_cycle(result), "Self-reference is a cycle");
    assert!(then_cycle_contains(result, "a"), "Cycle should contain 'a'");
}

#[test]
fn two_node_mutual_cycle_is_detected() {
    let nodes = given_nodes(&[("x", &["y"]), ("y", &["x"])]);
    let result = when_detect_cycle(&nodes);
    assert!(then_has_cycle(result), "Mutual dependency is a cycle");
}

#[test]
fn three_node_chain_cycle_is_detected() {
    let nodes = given_nodes(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"])]);
    let result = when_detect_cycle(&nodes);
    assert!(
        then_has_cycle(result),
        "Chain returning to start is a cycle"
    );
    assert!(then_cycle_length(result, 3), "Cycle should have 3 members");
}

#[test]
fn empty_graph_returns_none() {
    let nodes: Vec<CompensationNode> = vec![];
    let result = when_detect_cycle(&nodes);
    assert!(then_no_cycle(result), "Empty graph has no cycle");
}

#[test]
fn single_node_no_deps_returns_none() {
    let nodes = given_nodes(&[("solo", &[])]);
    let result = when_detect_cycle(&nodes);
    assert!(
        then_no_cycle(result),
        "Single node with no deps is not a cycle"
    );
}

#[test]
fn partial_cycle_is_detected() {
    let nodes = given_nodes(&[("a", &[]), ("b", &["a"]), ("c", &["b", "c"])]);
    let result = when_detect_cycle(&nodes);
    assert!(
        then_has_cycle(result),
        "Partial cycle with DAG branch should be detected"
    );
    assert!(then_cycle_contains(result, "c"), "Cycle should contain 'c'");
}
