//! Detection of unused (unreachable) steps in a workflow DAG.
//!
//! Uses BFS from the entry node to find all reachable nodes, then flags
//! any nodes that were not visited.

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use crate::diagnostic::{Diagnostic, LintCode};
use std::collections::{HashMap, HashSet, VecDeque};

use super::graph::{DagGraph, Step};

/// Check a DAG for unused (unreachable) steps.
///
/// Starting from the entry node (the step with `is_entry == true`), performs
/// a BFS to find all reachable nodes. Any node not reached is flagged as
/// unused with a Warning-level diagnostic.
///
/// The entry node is exempt from this check even if it has no incoming edges.
///
/// # Examples
///
/// ```
/// use vo_linter::rules::unused_steps::check_unused_steps;
/// use vo_linter::rules::unused_steps::graph::{DagGraph, Step};
///
/// let graph = DagGraph::new()
///     .add_step(Step { name: "a".into(), is_entry: true })
///     .add_step(Step { name: "b".into(), is_entry: false })
///     .add_step(Step { name: "c".into(), is_entry: false })
///     .add_edge("a", "b");
///
/// let diagnostics = check_unused_steps(&graph);
/// assert_eq!(diagnostics.len(), 1);
/// assert_eq!(diagnostics[0].message, "unused step: `c`");
/// ```
pub fn check_unused_steps(graph: &DagGraph) -> Vec<Diagnostic> {
    // No steps at all - nothing to check
    if graph.steps.is_empty() {
        return Vec::new();
    }

    // No edges - all non-entry nodes are unreachable
    // (entry node is exempt)
    let entry = match graph.entry_node() {
        Some(e) => e,
        None => return Vec::new(),
    };

    let adj = graph.adjacency_list();
    let all_nodes: HashSet<String> = graph.steps.iter().map(|s| s.name.clone()).collect();

    // BFS from entry node
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(entry.clone());
    visited.insert(entry);

    while let Some(current) = queue.pop_front() {
        if let Some(neighbors) = adj.get(&current) {
            for neighbor in neighbors {
                if visited.insert(neighbor.clone()) {
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    // Find unreachable nodes
    let mut unused: Vec<String> = all_nodes
        .difference(&visited)
        .cloned()
        .collect();
    unused.sort();

    unused
        .into_iter()
        .map(|name| {
            Diagnostic::new(
                LintCode::L004,
                format!("unused step: `{name}`"),
            )
            .with_suggestion("Remove unused step or add edge from a reachable step")
            .with_severity(crate::diagnostic::LintSeverity::Warning)
        })
        .collect()
}
