//! Graph emission: `--graph` CLI argument handling and workflow specification types (ADR-004, ADR-009).
//!
//! This module provides the types and functions needed to emit workflow graph
//! specifications when a binary is invoked with `--graph`. The Engine validates,
//! hashes, and stores this spec as a workflow version.

use std::io::Write;

use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use vo_types::NodeKind;
use vo_types::{DedupeScope, GuaranteeClass, NodeName, RetryPolicy, WorkflowName};

/// Marker returned when `--graph` flag is present.
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct GraphArgs;

/// Errors from parsing `--graph` arguments.
#[derive(Debug, PartialEq, Error)]
pub enum GraphArgsError {
    #[error("unrecognized argument: {arg}")]
    UnrecognizedArgument { arg: String },
    #[error("no --graph flag found")]
    NoGraphFlag,
}

/// Parsed CLI arguments combining `--graph` and `--execute-node`.
pub struct ParsedArgs {
    pub graph: bool,
    pub execute_node: Option<String>,
}

/// Parse CLI arguments for `--graph` and `--execute-node` flags.
///
/// # Errors
///
/// Returns `GraphArgsError::UnrecognizedArgument` when an unknown flag is used.
pub fn parse_exec_args(args: &[String]) -> Result<ParsedArgs, GraphArgsError> {
    let mut graph = false;
    let mut execute_node: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--graph" {
            if graph {
                return Err(GraphArgsError::UnrecognizedArgument { arg: arg.clone() });
            }
            graph = true;
        } else if arg == "--execute-node" {
            i += 1;
            if i >= args.len() {
                return Err(GraphArgsError::UnrecognizedArgument {
                    arg: "--execute-node".to_string(),
                });
            }
            if execute_node.is_some() {
                return Err(GraphArgsError::UnrecognizedArgument {
                    arg: "--execute-node".to_string(),
                });
            }
            execute_node = Some(args[i].clone());
        } else if arg.starts_with('-') {
            return Err(GraphArgsError::UnrecognizedArgument { arg: arg.clone() });
        }
        i += 1;
    }

    Ok(ParsedArgs {
        graph,
        execute_node,
    })
}

/// Parse CLI arguments for the `--graph` flag.
///
/// # Errors
///
/// Returns `GraphArgsError::NoGraphFlag` when `--graph` is absent.
/// Returns `GraphArgsError::UnrecognizedArgument` when extra positional args follow `--graph` or when `--graph` appears twice.
pub fn parse_graph_args(args: &[String]) -> Result<GraphArgs, GraphArgsError> {
    let parsed = parse_exec_args(args)?;
    if parsed.graph {
        Ok(GraphArgs)
    } else {
        Err(GraphArgsError::NoGraphFlag)
    }
}

/// Specification of a single workflow node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub name: NodeName,
    pub kind: NodeKind,
    #[serde(default = "default_retry_policy")]
    pub retry_policy: RetryPolicy,
}

fn default_retry_policy() -> RetryPolicy {
    RetryPolicy {
        max_attempts: 1,
        backoff_ms: 0,
        backoff_multiplier: 1.0,
        max_backoff_ms: u64::MAX,
    }
}

/// Specification of an edge between two workflow nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeSpec {
    pub from: NodeName,
    pub to: NodeName,
}

/// Validation errors for [`WorkflowSpec::validate`].
#[derive(Debug, PartialEq, Clone, Error)]
pub enum ValidationError {
    #[error("duplicate node name: {name}")]
    DuplicateNodeName { name: String },
    #[error("duplicate edge: {from} -> {to}")]
    DuplicateEdge { from: String, to: String },
    #[error("edge references non-existent source node: {name}")]
    MissingEdgeSource { name: String },
    #[error("edge references non-existent target node: {name}")]
    MissingEdgeTarget { name: String },
    #[error("self-loop edge on node: {name}")]
    SelfLoop { name: String },
    #[error("cycle detected: {cycle}")]
    CycleDetected { cycle: String },
    #[error("no entry point: every node has at least one incoming edge")]
    NoEntryPoint,
}
/// Full workflow graph specification produced by `--graph` (ADR-004, ADR-009, ADR-031).
///
/// This is the canonical workflow representation emitted by the SDK when
/// `./binary --graph` is invoked. The Engine validates, hashes, and stores
/// this spec as a workflow version.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowSpec {
    pub workflow_name: WorkflowName,
    pub nodes: Vec<NodeSpec>,
    pub edges: Vec<EdgeSpec>,
    #[serde(default)]
    pub dedupe_scope: DedupeScope,
    #[serde(default)]
    pub guarantee_class: GuaranteeClass,
}

fn default_dedupe_scope() -> DedupeScope {
    DedupeScope::Unbounded
}

impl<'de> serde::Deserialize<'de> for WorkflowSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
         #[derive(serde::Deserialize)]
        struct RawWorkflowSpec {
            workflow_name: WorkflowName,
            nodes: Vec<NodeSpec>,
            edges: Vec<EdgeSpec>,
            #[serde(default = "default_dedupe_scope")]
            dedupe_scope: DedupeScope,
            #[serde(default)]
            guarantee_class: GuaranteeClass,
        }

        let raw: RawWorkflowSpec = RawWorkflowSpec::deserialize(deserializer)?;

        let node_names: std::collections::HashSet<&str> =
            raw.nodes.iter().map(|n| n.name.as_str()).collect();

        let mut seen_edges: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();
        for edge in &raw.edges {
            if edge.from == edge.to {
                return Err(serde::de::Error::custom(format!(
                    "workflow contains a cycle: self-loop edge on {}",
                    edge.from.as_str()
                )));
            }
            if !node_names.contains(edge.from.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge references non-existent node: {}",
                    edge.from.as_str()
                )));
            }
            if !node_names.contains(edge.to.as_str()) {
                return Err(serde::de::Error::custom(format!(
                    "edge references non-existent node: {}",
                    edge.to.as_str()
                )));
            }
            if !seen_edges.insert((edge.from.as_str(), edge.to.as_str())) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate edge: {} -> {}",
                    edge.from.as_str(),
                    edge.to.as_str()
                )));
            }
        }

        let name_to_idx: std::collections::HashMap<&str, usize> = raw
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();

        let n = raw.nodes.len();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for edge in &raw.edges {
            if let (Some(&from), Some(&to)) = (
                name_to_idx.get(edge.from.as_str()),
                name_to_idx.get(edge.to.as_str()),
            ) {
                adj[from].push(to);
            }
        }

        const WHITE: u8 = 0;
        const GRAY: u8 = 1;
        let mut colors = vec![WHITE; n];

        fn has_cycle_from(node: usize, adj: &[Vec<usize>], colors: &mut [u8]) -> bool {
            colors[node] = GRAY;
            for &neighbor in &adj[node] {
                if colors[neighbor] == GRAY {
                    return true;
                }
                if colors[neighbor] == WHITE && has_cycle_from(neighbor, adj, colors) {
                    return true;
                }
            }
            colors[node] = 2;
            false
        }

        for i in 0..n {
            if colors[i] == WHITE && has_cycle_from(i, &adj, &mut colors) {
                return Err(serde::de::Error::custom("workflow contains a cycle"));
            }
        }

        Ok(WorkflowSpec {
            workflow_name: raw.workflow_name,
            nodes: raw.nodes,
            edges: raw.edges,
            dedupe_scope: raw.dedupe_scope,
            guarantee_class: raw.guarantee_class,
        })
    }
}

impl WorkflowSpec {
    /// Serialize to JSON bytes for `--graph` emission.
    #[must_use]
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("WorkflowSpec is always serializable")
    }

    /// Validate this spec before graph emission.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for node in &self.nodes {
            if !seen.insert(node.name.as_str()) {
                return Err(ValidationError::DuplicateNodeName {
                    name: node.name.as_str().to_string(),
                });
            }
        }

        let node_names: std::collections::HashSet<&str> =
            self.nodes.iter().map(|n| n.name.as_str()).collect();
        let mut seen_edges: std::collections::HashSet<(&str, &str)> = std::collections::HashSet::new();
        for edge in &self.edges {
            if !node_names.contains(edge.from.as_str()) {
                return Err(ValidationError::MissingEdgeSource {
                    name: edge.from.as_str().to_string(),
                });
            }
            if !node_names.contains(edge.to.as_str()) {
                return Err(ValidationError::MissingEdgeTarget {
                    name: edge.to.as_str().to_string(),
                });
            }
            if edge.from == edge.to {
                return Err(ValidationError::SelfLoop {
                    name: edge.from.as_str().to_string(),
                });
            }
            if !seen_edges.insert((edge.from.as_str(), edge.to.as_str())) {
                return Err(ValidationError::DuplicateEdge {
                    from: edge.from.as_str().to_string(),
                    to: edge.to.as_str().to_string(),
                });
            }
        }

        // Cycle detection via DFS (3-color).
        let n = self.nodes.len();
        if n > 0 {
            let name_to_idx: std::collections::HashMap<&str, usize> = self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, node)| (node.name.as_str(), i))
                .collect();

            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
            for edge in &self.edges {
                if let (Some(&from), Some(&to)) = (
                    name_to_idx.get(edge.from.as_str()),
                    name_to_idx.get(edge.to.as_str()),
                ) {
                    adj[from].push(to);
                }
            }

            let mut colors = vec![0u8; n]; // 0=WHITE, 1=GRAY, 2=BLACK
            let mut cycle_path: Vec<usize> = Vec::new();
            let mut stack: Vec<usize> = Vec::new();

            fn dfs(
                node: usize,
                adj: &[Vec<usize>],
                colors: &mut [u8],
                stack: &mut Vec<usize>,
                cycle_path: &mut Vec<usize>,
            ) -> bool {
                colors[node] = 1;
                stack.push(node);
                for &neighbor in &adj[node] {
                    if colors[neighbor] == 1 {
                        if let Some(pos) = stack.iter().position(|&x| x == neighbor) {
                            cycle_path.extend(stack[pos..].iter().copied());
                        }
                        return true;
                    }
                    if colors[neighbor] == 0 && dfs(neighbor, adj, colors, stack, cycle_path) {
                        return true;
                    }
                }
                stack.pop();
                colors[node] = 2;
                false
            }

            for i in 0..n {
                if colors[i] == 0 && dfs(i, &adj, &mut colors, &mut stack, &mut cycle_path) {
                    let names: Vec<String> = cycle_path
                        .iter()
                        .map(|&idx| self.nodes[idx].name.as_str().to_string())
                        .collect();
                    return Err(ValidationError::CycleDetected {
                        cycle: names.join(" -> "),
                    });
                }
            }

            // Entry point check: at least one node with no incoming edges.
            let has_target: std::collections::HashSet<usize> = self
                .edges
                .iter()
                .filter_map(|e| name_to_idx.get(e.to.as_str()).copied())
                .collect();
            if has_target.len() == n {
                return Err(ValidationError::NoEntryPoint);
            }
        }

        Ok(())
    }

    pub(crate) fn detect_cycle(&self) -> Option<String> {
        let n = self.nodes.len();
        if n == 0 {
            return None;
        }
        let name_to_idx: std::collections::HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.name.as_str(), i))
            .collect();
        let edges: Vec<(usize, usize)> = self
            .edges
            .iter()
            .filter_map(|e| {
                let from_idx = name_to_idx.get(e.from.as_str()).copied();
                let to_idx = name_to_idx.get(e.to.as_str()).copied();
                from_idx.and_then(|f| to_idx.map(|t| (f, t)))
            })
            .collect();
        let mut visited = vec![0u8; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut cycle_path: Vec<usize> = Vec::new();

        fn dfs(
            node: usize,
            edges: &[(usize, usize)],
            visited: &mut [u8],
            stack: &mut Vec<usize>,
            cycle_path: &mut Vec<usize>,
        ) -> bool {
            visited[node] = 1;
            stack.push(node);
            for &(_, to) in edges.iter().filter(|&&(_from, _)| _from == node) {
                if visited[to] == 0 {
                    if dfs(to, edges, visited, stack, cycle_path) {
                        return true;
                    }
                } else if visited[to] == 1 {
                    if let Some(pos) = stack.iter().position(|&x| x == to) {
                        let cycle: Vec<usize> = stack[pos..].to_vec();
                        cycle_path.extend(cycle);
                        return true;
                    }
                }
            }
            stack.pop();
            visited[node] = 2;
            false
        }

        for i in 0..n {
            if visited[i] == 0 && dfs(i, &edges, &mut visited, &mut stack, &mut cycle_path) {
                break;
            }
        }

        if cycle_path.is_empty() {
            return None;
        }

        let cycle_names: Vec<String> = cycle_path
            .iter()
            .map(|&idx| self.nodes[idx].name.as_str().to_string())
            .collect();
        Some(cycle_names.join(" -> "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::NodeKind;

    #[test]
    fn validate_accepts_valid_spec() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test-workflow").unwrap(),
            nodes: vec![
                NodeSpec {
                    name: NodeName::parse("step_a").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("step_b").unwrap(),
                    kind: NodeKind::ManagedEffect,
                },
            ],
            edges: vec![EdgeSpec {
                from: NodeName::parse("step_a").unwrap(),
                to: NodeName::parse("step_b").unwrap(),
            }],
        };
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validate_rejects_duplicate_node_names() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![
                NodeSpec {
                    name: NodeName::parse("step_a").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("step_a").unwrap(),
                    kind: NodeKind::Pure,
                },
            ],
            edges: vec![],
        };
        let err = spec.validate().unwrap_err();
        assert_eq!(
            err,
            ValidationError::DuplicateNodeName {
                name: "step_a".to_string()
            }
        );
    }

    #[test]
    fn validate_rejects_missing_edge_source() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![NodeSpec {
                name: NodeName::parse("step_a").unwrap(),
                kind: NodeKind::Pure,
            }],
            edges: vec![EdgeSpec {
                from: NodeName::parse("ghost").unwrap(),
                to: NodeName::parse("step_a").unwrap(),
            }],
        };
        let err = spec.validate().unwrap_err();
        assert_eq!(
            err,
            ValidationError::MissingEdgeSource {
                name: "ghost".to_string()
            }
        );
    }

    #[test]
    fn validate_rejects_missing_edge_target() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![NodeSpec {
                name: NodeName::parse("step_a").unwrap(),
                kind: NodeKind::Pure,
            }],
            edges: vec![EdgeSpec {
                from: NodeName::parse("step_a").unwrap(),
                to: NodeName::parse("ghost").unwrap(),
            }],
        };
        let err = spec.validate().unwrap_err();
        assert_eq!(
            err,
            ValidationError::MissingEdgeTarget {
                name: "ghost".to_string()
            }
        );
    }

    #[test]
    fn validate_rejects_self_loop() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![NodeSpec {
                name: NodeName::parse("step_a").unwrap(),
                kind: NodeKind::Pure,
            }],
            edges: vec![EdgeSpec {
                from: NodeName::parse("step_a").unwrap(),
                to: NodeName::parse("step_a").unwrap(),
            }],
        };
        let err = spec.validate().unwrap_err();
        assert_eq!(
            err,
            ValidationError::SelfLoop {
                name: "step_a".to_string()
            }
        );
    }

    #[test]
    fn validate_rejects_cycle() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![
                NodeSpec {
                    name: NodeName::parse("a").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("b").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("c").unwrap(),
                    kind: NodeKind::Pure,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: NodeName::parse("a").unwrap(),
                    to: NodeName::parse("b").unwrap(),
                },
                EdgeSpec {
                    from: NodeName::parse("b").unwrap(),
                    to: NodeName::parse("c").unwrap(),
                },
                EdgeSpec {
                    from: NodeName::parse("c").unwrap(),
                    to: NodeName::parse("a").unwrap(),
                },
            ],
        };
        let err = spec.validate().unwrap_err();
        assert!(matches!(err, ValidationError::CycleDetected { .. }));
    }

    #[test]
    fn validate_accepts_diamond_dag() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![
                NodeSpec {
                    name: NodeName::parse("start").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("left").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("right").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("end").unwrap(),
                    kind: NodeKind::Pure,
                },
            ],
            edges: vec![
                EdgeSpec {
                    from: NodeName::parse("start").unwrap(),
                    to: NodeName::parse("left").unwrap(),
                },
                EdgeSpec {
                    from: NodeName::parse("start").unwrap(),
                    to: NodeName::parse("right").unwrap(),
                },
                EdgeSpec {
                    from: NodeName::parse("left").unwrap(),
                    to: NodeName::parse("end").unwrap(),
                },
                EdgeSpec {
                    from: NodeName::parse("right").unwrap(),
                    to: NodeName::parse("end").unwrap(),
                },
            ],
        };
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn validate_accepts_single_node_no_edges() {
        let spec = WorkflowSpec {
            workflow_name: WorkflowName::parse("test").unwrap(),
            nodes: vec![NodeSpec {
                name: NodeName::parse("solo").unwrap(),
                kind: NodeKind::Pure,
            }],
            edges: vec![],
        };
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn given_workflow_spec_when_serialized_then_guarantee_class_round_trips() {
        let original = WorkflowSpec {
            workflow_name: WorkflowName::parse("checkout").unwrap(),
            nodes: vec![
                NodeSpec {
                    name: NodeName::parse("validate").unwrap(),
                    kind: NodeKind::Pure,
                },
                NodeSpec {
                    name: NodeName::parse("charge").unwrap(),
                    kind: NodeKind::ManagedEffect,
                },
            ],
            edges: vec![EdgeSpec {
                from: NodeName::parse("validate").unwrap(),
                to: NodeName::parse("charge").unwrap(),
            }],
            dedupe_scope: DedupeScope::WorkflowId,
            guarantee_class: GuaranteeClass::ExactOnce,
        };

        let json = serde_json::to_string(&original).expect("serialize WorkflowSpec");
        assert!(json.contains(r#""guarantee_class":"exact-once""#), "JSON must contain guarantee_class field");

        let deserialized: WorkflowSpec =
            serde_json::from_str(&json).expect("deserialize WorkflowSpec from JSON");
        assert_eq!(original, deserialized, "guarantee_class must round-trip through serialization");
    }
}

/// Emit the workflow spec as JSON to stdout and exit.
///
/// This function is called when the binary is invoked with `--graph`.
/// It serializes the `WorkflowSpec` to JSON, prints it to stdout,
/// and exits with code 0.
///
/// # Example
///
/// ```ignore
/// fn main() {
///     let args: Vec<String> = std::env::args().collect();
///     if let Err(()) = vo_sdk::emit_graph_if_requested(&args, workflow_spec) {
///         std::process::exit(1);
///     }
/// }
/// ```
///
/// # Errors
///
/// Returns `()` if `--graph` was not present. If `--graph` was present,
/// this function always terminates the process.
#[allow(clippy::result_unit_err)]
pub fn emit_graph_if_requested(args: &[String], spec: &WorkflowSpec) -> Result<(), ()> {
    match parse_graph_args(args) {
        Ok(_graph_args) => {
            if let Some(cycle) = spec.detect_cycle() {
                eprintln!("error: cycle detected: {}", cycle);
                std::process::exit(1);
            }
            let json = spec.to_json_bytes();
            std::io::stdout()
                .write_all(&json)
                .expect("stdout write should not fail");
            std::process::exit(0);
        }
        Err(GraphArgsError::NoGraphFlag) => Ok(()),
        Err(e) => {
            eprintln!("error: {e}");
            Err(())
        }
    }
}

/// Errors for node dispatch operations.
#[derive(Debug, PartialEq, Error)]
pub enum ExecuteError {
    #[error("node not found: {0}")]
    NodeNotFound(String),
    #[error("node is not executable: {0}")]
    NotExecutable(String),
}

/// Dispatch execution of a single named node from the workflow spec.
///
/// When `--execute-node <name>` is used, only the named node runs.
/// All other nodes are skipped. The node receives its input from FD3
/// and writes output to FD4 via the standard SDK I/O helpers.
///
/// # Errors
///
/// Returns `ExecuteError::NodeNotFound` if the node does not exist in the spec.
/// Returns `ExecuteError::NotExecutable` if the node is not a task-executable kind.
pub fn find_executable_node(spec: &WorkflowSpec, node_name: &str) -> Result<&NodeSpec, ExecuteError> {
    spec.nodes
        .iter()
        .find(|n| n.name.as_str() == node_name)
        .ok_or_else(|| ExecuteError::NodeNotFound(node_name.to_string()))
}

/// Check if a node is executable (not a signal/wait-only node).
#[must_use]
pub fn is_node_executable(node: &NodeSpec) -> bool {
    matches!(
        node.kind,
        vo_types::NodeKind::Pure
            | vo_types::NodeKind::ManagedEffect
            | vo_types::NodeKind::Unsafe
    )
}
