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
use syn::{visit::Visit, File};

use super::graph::{DagGraph, Edge, Step};

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
    if graph.steps.is_empty() {
        return Vec::new();
    }

    let entry = match graph.entry_node() {
        Some(e) => e,
        None => return Vec::new(),
    };

    let adj = graph.adjacency_list();
    let all_nodes: HashSet<String> = graph.steps.iter().map(|s| s.name.clone()).collect();

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

    let mut unused: Vec<String> = all_nodes.difference(&visited).cloned().collect();
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

fn member_is(member: &syn::Member, name: &str) -> bool {
    if let syn::Member::Named(n) = member {
        n == name
    } else {
        false
    }
}

struct WorkflowDetector {
    diagnostics: Vec<Diagnostic>,
}

impl Default for WorkflowDetector {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for WorkflowDetector {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node.path.is_ident("WorkflowDefinition") {
            if let Some(graph) = self.extract_graph_from_workflow(node) {
                let unused_diags = check_unused_steps(&graph);
                self.diagnostics.extend(unused_diags);
            }
        }
        syn::visit::visit_expr_struct(self, node);
    }
}

impl WorkflowDetector {
    fn extract_graph_from_workflow(
        &self,
        node: &syn::ExprStruct,
    ) -> Option<DagGraph> {
        let mut steps: Vec<Step> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut nodes_field: Option<&syn::Expr> = None;
        let mut edges_field: Option<&syn::Expr> = None;

        for field in &node.fields {
            if member_is(&field.member, "nodes") {
                nodes_field = Some(&field.expr);
            } else if member_is(&field.member, "edges") {
                edges_field = Some(&field.expr);
            }
        }

        if let Some(nodes_expr) = nodes_field {
            steps = self.extract_steps_from_expr(nodes_expr);
        }

        if let Some(edges_expr) = edges_field {
            edges = self.extract_edges_from_expr(edges_expr);
        }

        if steps.is_empty() {
            return None;
        }

        let mut graph = DagGraph::new();
        for step in steps {
            graph = graph.add_step(step);
        }
        for edge in edges {
            graph = graph.add_edge(edge.from, edge.to);
        }

        Some(graph)
    }

    fn extract_steps_from_expr(&self, expr: &syn::Expr) -> Vec<Step> {
        let mut steps = Vec::new();

        if let syn::Expr::Array(arr) = expr {
            for elem in &arr.elems {
                if let syn::Expr::Struct(node_struct) = elem {
                    if node_struct.path.is_ident("DagNode")
                        || node_struct.path.is_ident("Step")
                    {
                        if let Some(step) = self.extract_step_from_struct(node_struct) {
                            steps.push(step);
                        }
                    }
                }
            }
        }

        steps
    }

    fn extract_step_from_struct(&self, node_struct: &syn::ExprStruct) -> Option<Step> {
        let mut name: Option<String> = None;
        let mut is_entry = false;

        for field in &node_struct.fields {
            if member_is(&field.member, "node_name")
                || member_is(&field.member, "name")
            {
                if let Some(n) = self.extract_string_from_expr(&field.expr) {
                    name = Some(n);
                }
            }
            if member_is(&field.member, "is_entry") {
                if let syn::Expr::Lit(lit) = &field.expr {
                    if let syn::Lit::Bool(b) = &lit.lit {
                        is_entry = b.value;
                    }
                }
            }
        }

        name.map(|n| Step { name: n, is_entry })
    }

    fn extract_edges_from_expr(&self, expr: &syn::Expr) -> Vec<Edge> {
        let mut edges = Vec::new();

        if let syn::Expr::Array(arr) = expr {
            for elem in &arr.elems {
                if let syn::Expr::Struct(node_struct) = elem {
                    if node_struct.path.is_ident("Edge") {
                        if let Some(edge) = self.extract_edge_from_struct(node_struct) {
                            edges.push(edge);
                        }
                    }
                }
            }
        }

        edges
    }

    fn extract_edge_from_struct(&self, node_struct: &syn::ExprStruct) -> Option<Edge> {
        let mut from: Option<String> = None;
        let mut to: Option<String> = None;

        for field in &node_struct.fields {
            if member_is(&field.member, "source_node")
                || member_is(&field.member, "from")
            {
                if let Some(n) = self.extract_string_from_expr(&field.expr) {
                    from = Some(n);
                }
            }
            if member_is(&field.member, "target_node")
                || member_is(&field.member, "to")
            {
                if let Some(n) = self.extract_string_from_expr(&field.expr) {
                    to = Some(n);
                }
            }
        }

        from.and_then(|f| to.map(|t| Edge { from: f, to: t }))
    }

    fn extract_string_from_expr(&self, expr: &syn::Expr) -> Option<String> {
        match expr {
            syn::Expr::Call(call) => {
                if call.args.len() == 1 {
                    return self.extract_string_from_expr(&call.args[0]);
                }
                None
            }
            syn::Expr::Path(path) => {
                let seg = path.path.segments.last()?;
                if seg.ident == "into" {
                    return None;
                }
                Some(seg.ident.to_string())
            }
            syn::Expr::Lit(lit) => match &lit.lit {
                syn::Lit::Str(s) => Some(s.value()),
                syn::Lit::Verbatim(s) => {
                    let s_str = s.to_string();
                    if (s_str.starts_with('"') && s_str.ends_with('"'))
                        || (s_str.starts_with('\'') && s_str.ends_with('\''))
                    {
                        Some(s_str[1..s_str.len() - 1].to_string())
                    } else {
                        Some(s_str)
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

/// Check Rust source code for unused steps in workflow definitions.
///
/// Parses the AST to find `WorkflowDefinition` struct literals and checks
/// for unreachable nodes using BFS from the entry node.
///
/// # Examples
///
/// ```
/// use vo_linter::rules::unused_steps::check_unused_steps_ast;
///
/// let src = r#"
///     WorkflowDefinition {
///         nodes: [
///             DagNode { node_name: NodeName("start".into()), is_entry: true },
///             DagNode { node_name: NodeName("a".into()) },
///             DagNode { node_name: NodeName("orphan".into()) }
///         ],
///         edges: [
///             Edge { source_node: "start".into(), target_node: "a".into() }
///         ]
///     }
/// "#;
///
/// let file = syn::parse_str(src).unwrap();
/// let diagnostics = check_unused_steps_ast(&file);
/// assert_eq!(diagnostics.len(), 1);
/// ```
#[must_use]
pub fn check_unused_steps_ast(file: &File) -> Vec<Diagnostic> {
    let mut detector = WorkflowDetector::default();
    detector.visit_file(file);
    detector.diagnostics
}