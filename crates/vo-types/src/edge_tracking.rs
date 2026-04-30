//! Runtime edge traversal tracking (ADR-022 Section 2).
//!
//! This module provides types for tracking dynamically traversed edges at runtime,
//! enabling conditional fan-in semantics where downstream nodes select the output
//! from the actually-traversed branch rather than all static graph edges.
//!
//! # Types
//!
//! - `TraversedEdge`: A single edge traversal record, capturing the edge identity
//!   and the source node's outcome at traversal time.
//! - `EdgeTraversalLog`: Append-only log of all traversed edges for a workflow
//!   execution, used by conditional fan-in to select which parent output to pipe.
//!
//! # ADR-022 Section 2 Compliance
//!
//! The Engine tracks not just the static graph edges, but the *actually traversed*
//! edges during execution. When executing Node C, the Engine inspects its incoming
//! edges and only pipes the JSON output from the specific parent node that was
//! *actually executed* in the current path.

use serde::{Deserialize, Serialize};

use crate::{NodeName, StepOutcome};

// ============================================================================
// TraversedEdge
// ============================================================================

/// A single dynamically traversed edge at runtime.
///
/// Records that an edge from `source_node` to `target_node` was actually
/// traversed during execution, along with the outcome of the source node
/// at traversal time.
///
/// This is the core unit of runtime edge tracking for conditional fan-in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraversedEdge {
    /// The source node of the traversed edge.
    pub source_node: NodeName,
    /// The target node of the traversed edge.
    pub target_node: NodeName,
    /// The outcome of the source node when this edge was traversed.
    /// `Some` means the edge was conditionally traversed based on a specific outcome.
    /// `None` means the edge was always traversed (unconditional / `EdgeCondition::Always`).
    pub outcome: Option<StepOutcome>,
}

impl TraversedEdge {
    #[must_use]
    pub fn new(source_node: NodeName, target_node: NodeName, outcome: Option<StepOutcome>) -> Self {
        Self {
            source_node,
            target_node,
            outcome,
        }
    }

    /// Create a traversed edge from a successful step.
    #[must_use]
    pub fn on_success(source_node: NodeName, target_node: NodeName) -> Self {
        Self {
            source_node,
            target_node,
            outcome: Some(StepOutcome::Success),
        }
    }

    /// Create a traversed edge from a failed step.
    #[must_use]
    pub fn on_failure(source_node: NodeName, target_node: NodeName) -> Self {
        Self {
            source_node,
            target_node,
            outcome: Some(StepOutcome::Failure),
        }
    }

    /// Create an unconditional traversed edge.
    #[must_use]
    pub fn always(source_node: NodeName, target_node: NodeName) -> Self {
        Self {
            source_node,
            target_node,
            outcome: None,
        }
    }
}

// ============================================================================
// EdgeTraversalLog
// ============================================================================

/// Append-only log of dynamically traversed edges during workflow execution.
///
/// Maintains the set of edges actually traversed (not just static graph edges),
/// enabling conditional fan-in: when a downstream node has multiple incoming edges
/// from different branches, the fan-in selects output from the actually-traversed
/// parent only.
///
/// # Invariants
///
/// - INV-001: Once an edge is recorded, it persists (append-only).
/// - INV-002: Multiple traversals of the same edge are allowed (retries).
/// - INV-003: An edge can be recorded with different outcomes across retrials.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeTraversalLog {
    records: Vec<TraversedEdge>,
}

impl EdgeTraversalLog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity),
        }
    }

    /// Record an edge traversal.
    pub fn record(&mut self, edge: TraversedEdge) {
        self.records.push(edge);
    }

    /// Record a traversal of an edge with a known outcome.
    pub fn record_with_outcome(
        &mut self,
        source: NodeName,
        target: NodeName,
        outcome: StepOutcome,
    ) {
        self.records
            .push(TraversedEdge::new(source, target, Some(outcome)));
    }

    /// Record an unconditional traversal (always traversed).
    pub fn record_always(&mut self, source: NodeName, target: NodeName) {
        self.records.push(TraversedEdge::always(source, target));
    }

    /// Get all edges that were traversed targeting a specific node.
    #[must_use]
    pub fn traversed_to(&self, target: &NodeName) -> Vec<&TraversedEdge> {
        self.records
            .iter()
            .filter(|e| &e.target_node == target)
            .collect()
    }

    /// Get the most recent traversal outcome for a specific edge (source -> target).
    #[must_use]
    pub fn latest_outcome(&self, source: &NodeName, target: &NodeName) -> Option<StepOutcome> {
        self.records
            .iter()
            .rev()
            .find(|e| &e.source_node == source && &e.target_node == target)
            .and_then(|e| e.outcome)
    }

    /// Get all unique source nodes that were actually traversed to a given target.
    /// This is the key data for conditional fan-in: which parents were executed?
    #[must_use]
    pub fn traversed_sources(&self, target: &NodeName) -> Vec<&NodeName> {
        let mut sources = Vec::new();
        for edge in &self.records {
            if &edge.target_node == target && !sources.iter().any(|s| *s == &edge.source_node) {
                sources.push(&edge.source_node);
            }
        }
        sources
    }

    /// Check if any edge targeting the given node was traversed.
    #[must_use]
    pub fn has_traversed(&self, target: &NodeName) -> bool {
        self.records.iter().any(|e| &e.target_node == target)
    }

    /// Get the number of recorded traversals.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if no traversals have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Get all records.
    #[must_use]
    pub fn records(&self) -> &[TraversedEdge] {
        &self.records
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

// ============================================================================
// Conditional Fan-In
// ============================================================================

/// Select the output source for a fan-in node based on runtime traversal data.
///
/// Given a target node and the set of edges that were actually traversed during
/// execution, returns the source node whose output should be piped to the target.
///
/// # Algorithm
///
/// 1. If exactly one source node was traversed, return it.
/// 2. If multiple sources were traversed (e.g., from retried branches),
///    return the one with the most recent traversal.
/// 3. If no sources were traversed, return `None` (fan-in has no input yet).
///
/// This implements ADR-022 Section 2: "the Engine only pipes the JSON output
/// from the specific parent node that was *actually executed* in the current path."
#[must_use]
pub fn select_fan_in_source<'a>(
    target: &'a NodeName,
    traversal_log: &'a EdgeTraversalLog,
) -> Option<&'a NodeName> {
    let sources = traversal_log.traversed_sources(target);
    if sources.is_empty() {
        return None;
    }

    // If only one source, use it
    if sources.len() == 1 {
        return Some(sources[0]);
    }

    // Multiple sources: prefer the one whose last traversal was most recent
    // (latest record in the log)
    let mut best_source: Option<&NodeName> = None;
    let mut best_index: usize = 0;

    for (i, source) in sources.iter().enumerate() {
        if let Some(last_idx) = traversal_log
            .records()
            .iter()
            .rev()
            .position(|e| &e.source_node == *source && &e.target_node == target)
        {
            let last_idx = traversal_log.records().len() - 1 - last_idx;
            match best_source {
                None => {
                    best_source = Some(*source);
                    best_index = last_idx;
                }
                Some(best) => {
                    if last_idx > best_index {
                        best_source = Some(*source);
                        best_index = last_idx;
                    }
                }
            }
        }
    }

    best_source
}

// ============================================================================
// Router Decision
// ============================================================================

/// The boolean decision output by a Router node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouterDecision {
    /// The "Yes" branch was taken.
    Yes,
    /// The "No" branch was taken.
    No,
}

impl RouterDecision {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            RouterDecision::Yes => "yes",
            RouterDecision::No => "no",
        }
    }

    /// Convert to a boolean representation.
    #[must_use]
    pub fn to_bool(&self) -> bool {
        matches!(self, RouterDecision::Yes)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversed_edge_serializes_correctly() {
        let edge = TraversedEdge::on_success(NodeName("a".into()), NodeName("b".into()));
        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("\"source_node\""));
        assert!(json.contains("\"target_node\""));
        assert!(json.contains("\"Success\""));
    }

    #[test]
    fn traversed_edge_round_trips() {
        let edge = TraversedEdge::on_failure(NodeName("x".into()), NodeName("y".into()));
        let json = serde_json::to_string(&edge).unwrap();
        let recovered: TraversedEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, edge);
    }

    #[test]
    fn traversed_edge_always_has_none_outcome() {
        let edge = TraversedEdge::always(NodeName("a".into()), NodeName("b".into()));
        assert!(edge.outcome.is_none());
    }

    #[test]
    fn traversal_log_records_and_retrieves() {
        let mut log = EdgeTraversalLog::new();
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("b".into()),
            StepOutcome::Success,
        );
        assert_eq!(log.len(), 1);
        assert!(log.has_traversed(&NodeName("b".into())));
    }

    #[test]
    fn traversal_log_latest_outcome_returns_correct_outcome() {
        let mut log = EdgeTraversalLog::new();
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("b".into()),
            StepOutcome::Success,
        );
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("b".into()),
            StepOutcome::Failure,
        );
        assert_eq!(
            log.latest_outcome(&NodeName("a".into()), &NodeName("b".into())),
            Some(StepOutcome::Failure)
        );
    }

    #[test]
    fn traversal_log_traversed_sources_returns_unique_sources() {
        let mut log = EdgeTraversalLog::new();
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("c".into()),
            StepOutcome::Success,
        );
        log.record_with_outcome(
            NodeName("b".into()),
            NodeName("c".into()),
            StepOutcome::Failure,
        );
        let sources = log.traversed_sources(&NodeName("c".into()));
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn traversal_log_clear_removes_all_records() {
        let mut log = EdgeTraversalLog::new();
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("b".into()),
            StepOutcome::Success,
        );
        assert!(!log.is_empty());
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn select_fan_in_source_returns_single_source() {
        let mut log = EdgeTraversalLog::new();
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("c".into()),
            StepOutcome::Success,
        );
        let source = select_fan_in_source(&NodeName("c".into()), &log).map(|s| s.to_string());
        assert_eq!(source, Some("a".to_string()));
    }

    #[test]
    fn select_fan_in_source_returns_most_recent_for_multiple_sources() {
        let mut log = EdgeTraversalLog::new();
        // 'a' traversed first
        log.record_with_outcome(
            NodeName("a".into()),
            NodeName("c".into()),
            StepOutcome::Success,
        );
        // 'b' traversed later (most recent)
        log.record_with_outcome(
            NodeName("b".into()),
            NodeName("c".into()),
            StepOutcome::Success,
        );
        let source = select_fan_in_source(&NodeName("c".into()), &log).map(|s| s.to_string());
        assert_eq!(source, Some("b".to_string()));
    }

    #[test]
    fn select_fan_in_source_returns_none_when_no_traversals() {
        let log = EdgeTraversalLog::new();
        let target = NodeName("c".into());
        let source = select_fan_in_source(&target, &log);
        assert!(source.is_none());
    }

    #[test]
    fn router_decision_serializes() {
        let yes = serde_json::to_string(&RouterDecision::Yes).unwrap();
        assert!(yes.contains("Yes"));
        let no = serde_json::to_string(&RouterDecision::No).unwrap();
        assert!(no.contains("No"));
    }

    #[test]
    fn router_decision_round_trips() {
        let json = "\"Yes\"";
        let recovered: RouterDecision = serde_json::from_str(json).unwrap();
        assert_eq!(recovered, RouterDecision::Yes);
    }

    #[test]
    fn router_decision_to_bool() {
        assert!(RouterDecision::Yes.to_bool());
        assert!(!RouterDecision::No.to_bool());
    }

    #[test]
    fn router_decision_labels() {
        assert_eq!(RouterDecision::Yes.label(), "yes");
        assert_eq!(RouterDecision::No.label(), "no");
    }
}
