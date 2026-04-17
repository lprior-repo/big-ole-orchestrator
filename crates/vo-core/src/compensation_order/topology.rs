//! Topological sort algorithms for compensation ordering

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
                    let cycle_start = path.iter().position(|n| n == dep).unwrap();
                    return Some(path[cycle_start..].to_vec());
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
