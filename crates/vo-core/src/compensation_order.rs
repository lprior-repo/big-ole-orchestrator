//! Reverse dependency ordering for saga compensation (ADR-034).
//!
//! Computes the order in which compensations must execute during rollback,
//! ensuring dependees are compensated before their dependents.

use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompensationNode {
    pub effect_id: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompensationOrderResult {
    Ordered { execution_order: Vec<String> },
    CycleDetected { cycle_members: Vec<String> },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderingError {
    UnknownDependency {
        effect_id: String,
        unknown_dep: String,
    },
    DuplicateEffectId {
        effect_id: String,
    },
    CycleDetected {
        cycle_members: Vec<String>,
    },
}

pub fn compute_compensation_order(
    nodes: Vec<CompensationNode>,
) -> Result<CompensationOrderResult, OrderingError> {
    if nodes.is_empty() {
        return Ok(CompensationOrderResult::Empty);
    }

    validate_dependencies(&nodes)?;

    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for node in &nodes {
        in_degree.insert(node.effect_id.clone(), 0);
        dependents.insert(node.effect_id.clone(), vec![]);
    }

    for node in &nodes {
        for dep in &node.dependencies {
            if let Some(count) = in_degree.get_mut(&node.effect_id) {
                *count += 1;
            }
            if let Some(node_list) = dependents.get_mut(dep) {
                node_list.push(node.effect_id.clone());
            }
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &degree)| degree == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut execution_order: Vec<String> = Vec::new();

    while let Some(node_id) = queue.pop_front() {
        execution_order.push(node_id.clone());
        if let Some(dep_list) = dependents.get(&node_id) {
            for dependent in dep_list {
                if let Some(degree) = in_degree.get_mut(dependent.as_str()) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }
    }

    if execution_order.len() != nodes.len() {
        let cycle_nodes: Vec<String> = nodes
            .iter()
            .filter(|n| !execution_order.contains(&n.effect_id))
            .map(|n| n.effect_id.clone())
            .collect();
        return Err(OrderingError::CycleDetected {
            cycle_members: cycle_nodes,
        });
    }

    execution_order.reverse();
    Ok(CompensationOrderResult::Ordered { execution_order })
}

pub fn detect_cycle(nodes: &[CompensationNode]) -> Option<Vec<String>> {
    if nodes.is_empty() {
        return None;
    }

    let mut color: HashMap<String, u8> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for node in nodes {
        color.insert(node.effect_id.clone(), 0);
        adjacency.insert(node.effect_id.clone(), node.dependencies.clone());
    }

    fn dfs(
        node: &str,
        color: &mut HashMap<String, u8>,
        adjacency: &HashMap<String, Vec<String>>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        color.insert(node.to_string(), 1);
        path.push(node.to_string());

        if let Some(deps) = adjacency.get(node) {
            for dep in deps {
                let dep_color = color.get(dep).copied().unwrap_or(0);
                if dep_color == 1 {
                    let cycle_start = path
                        .iter()
                        .position(|n| n == dep)
                        .unwrap_or_else(|| path.len());
                    return if cycle_start < path.len() {
                        Some(path[cycle_start..].to_vec())
                    } else {
                        None
                    };
                }
                if dep_color == 0 {
                    if let Some(cycle) = dfs(dep, color, adjacency, path) {
                        return Some(cycle);
                    }
                }
            }
        }

        color.insert(node.to_string(), 2);
        path.pop();
        None
    }

    for node in nodes {
        if color.get(&node.effect_id).copied().unwrap_or(0) == 0 {
            if let Some(cycle) = dfs(&node.effect_id, &mut color, &adjacency, &mut vec![]) {
                return Some(cycle);
            }
        }
    }

    None
}

pub fn validate_dependencies(nodes: &[CompensationNode]) -> Result<(), OrderingError> {
    let mut seen: HashSet<String> = HashSet::new();

    for node in nodes {
        if seen.contains(&node.effect_id) {
            return Err(OrderingError::DuplicateEffectId {
                effect_id: node.effect_id.clone(),
            });
        }
        seen.insert(node.effect_id.clone());

        for dep in &node.dependencies {
            if !seen.contains(dep) {
                let all_ids: HashSet<&str> = nodes.iter().map(|n| n.effect_id.as_str()).collect();
                if !all_ids.contains(dep.as_str()) {
                    return Err(OrderingError::UnknownDependency {
                        effect_id: node.effect_id.clone(),
                        unknown_dep: dep.clone(),
                    });
                }
            }
        }
    }

    Ok(())
}

pub fn filter_compensatable(
    nodes: &[CompensationNode],
    policies: &HashMap<String, CompensationPolicy>,
) -> Vec<CompensationNode> {
    nodes
        .iter()
        .filter(|node| {
            policies
                .get(&node.effect_id)
                .map(|p| !matches!(p, CompensationPolicy::NotNeeded))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompensationPolicy {
    Required,
    BestEffort,
    NotNeeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, deps: &[&str]) -> CompensationNode {
        CompensationNode {
            effect_id: id.to_string(),
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = compute_compensation_order(vec![]).expect("empty input should not error");
        assert_eq!(result, CompensationOrderResult::Empty);
    }

    #[test]
    fn single_node_returns_ordered() {
        let nodes = vec![node("a", &[])];
        let result = compute_compensation_order(nodes).expect("single node should not error");
        match result {
            CompensationOrderResult::Ordered { execution_order } => {
                assert_eq!(execution_order, vec!["a"]);
            }
            other => panic!("expected Ordered, got {:?}", other),
        }
    }

    #[test]
    fn linear_chain_a_to_b_to_c() {
        let nodes = vec![node("c", &["b"]), node("b", &["a"]), node("a", &[])];
        let result = compute_compensation_order(nodes).expect("linear chain should not error");
        match result {
            CompensationOrderResult::Ordered { execution_order } => {
                let pos_a = execution_order
                    .iter()
                    .position(|x| x == "a")
                    .expect("a missing");
                let pos_b = execution_order
                    .iter()
                    .position(|x| x == "b")
                    .expect("b missing");
                let pos_c = execution_order
                    .iter()
                    .position(|x| x == "c")
                    .expect("c missing");
                assert!(pos_c < pos_b, "c must come before b in compensation order");
                assert!(pos_b < pos_a, "b must come before a in compensation order");
            }
            other => panic!("expected Ordered, got {:?}", other),
        }
    }

    #[test]
    fn diamond_dependency() {
        let nodes = vec![
            node("d", &[]),
            node("c", &["d"]),
            node("b", &["d"]),
            node("a", &["b", "c"]),
        ];
        let result = compute_compensation_order(nodes).expect("diamond should not error");
        match result {
            CompensationOrderResult::Ordered { execution_order } => {
                let pos =
                    |id: &str| -> usize { execution_order.iter().position(|x| x == id).expect(id) };
                assert!(pos("a") < pos("b"), "a must come before b");
                assert!(pos("a") < pos("c"), "a must come before c");
                assert!(pos("b") < pos("d"), "b must come before d");
                assert!(pos("c") < pos("d"), "c must come before d");
            }
            other => panic!("expected Ordered, got {:?}", other),
        }
    }

    #[test]
    fn cycle_detected() {
        let nodes = vec![node("a", &["b"]), node("b", &["c"]), node("c", &["a"])];
        let result = compute_compensation_order(nodes);
        match result {
            Err(OrderingError::CycleDetected { cycle_members }) => {
                assert!(
                    !cycle_members.is_empty(),
                    "cycle members should not be empty"
                );
            }
            Ok(CompensationOrderResult::CycleDetected { cycle_members }) => {
                assert!(!cycle_members.is_empty());
            }
            other => panic!("expected cycle detection, got {:?}", other),
        }
    }

    #[test]
    fn self_referencing_is_cycle() {
        let nodes = vec![node("a", &["a"])];
        let result = compute_compensation_order(nodes);
        match result {
            Err(OrderingError::CycleDetected { cycle_members }) => {
                assert!(cycle_members.contains(&"a".to_string()));
            }
            Ok(CompensationOrderResult::CycleDetected { cycle_members }) => {
                assert!(cycle_members.contains(&"a".to_string()));
            }
            other => panic!("expected cycle detection for self-ref, got {:?}", other),
        }
    }

    #[test]
    fn detect_cycle_returns_some_for_cyclic_graph() {
        let nodes = vec![node("x", &["y"]), node("y", &["x"])];
        let cycle = detect_cycle(&nodes);
        assert!(
            cycle.is_some(),
            "detect_cycle should return Some for cyclic graph"
        );
    }

    #[test]
    fn detect_cycle_returns_none_for_dag() {
        let nodes = vec![node("a", &[]), node("b", &["a"])];
        let cycle = detect_cycle(&nodes);
        assert!(cycle.is_none(), "detect_cycle should return None for DAG");
    }

    #[test]
    fn validate_dependencies_rejects_unknown() {
        let nodes = vec![node("a", &["nonexistent"])];
        let result = validate_dependencies(&nodes);
        assert!(
            matches!(result, Err(OrderingError::UnknownDependency { .. })),
            "expected UnknownDependency error, got {:?}",
            result
        );
    }

    #[test]
    fn validate_dependencies_rejects_duplicates() {
        let nodes = vec![node("a", &[]), node("a", &[])];
        let result = validate_dependencies(&nodes);
        assert!(
            matches!(result, Err(OrderingError::DuplicateEffectId { .. })),
            "expected DuplicateEffectId error, got {:?}",
            result
        );
    }

    #[test]
    fn two_independent_nodes_any_order() {
        let nodes = vec![node("a", &[]), node("b", &[])];
        let result = compute_compensation_order(nodes).expect("independent nodes should not error");
        match result {
            CompensationOrderResult::Ordered { execution_order } => {
                assert_eq!(execution_order.len(), 2);
                assert!(execution_order.contains(&"a".to_string()));
                assert!(execution_order.contains(&"b".to_string()));
            }
            other => panic!("expected Ordered, got {:?}", other),
        }
    }

    #[test]
    fn filter_compensatable_removes_not_needed() {
        let nodes = vec![node("a", &[]), node("b", &[]), node("c", &[])];
        let mut policies = HashMap::new();
        policies.insert("a".to_string(), CompensationPolicy::Required);
        policies.insert("b".to_string(), CompensationPolicy::NotNeeded);
        policies.insert("c".to_string(), CompensationPolicy::BestEffort);

        let filtered = filter_compensatable(&nodes, &policies);
        let ids: Vec<&str> = filtered.iter().map(|n| n.effect_id.as_str()).collect();
        assert!(ids.contains(&"a"), "required should be kept");
        assert!(!ids.contains(&"b"), "not-needed should be removed");
        assert!(ids.contains(&"c"), "best-effort should be kept");
    }
}
