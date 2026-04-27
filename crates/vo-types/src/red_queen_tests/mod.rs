//! Red Queen adversarial tests for vo-types workflow module.
//!
//! bead_id: vo-ald
//! phase: state-5-red-queen
//!
//! Dimensions attacked:
//!   - contract-violations: NaN/INFINITY bypass, direct construction
//!   - error-semantics: misleading error messages
//!   - json-attacks: malformed, wrong types, extra fields, nulls
//!   - cycle-detection-advanced: disconnected, diamond+cycle, large cycles
//!   - next_nodes-edge-cases: non-existent, duplicates, condition filtration
//!   - boundary-values: u8::MAX, u64::MAX, negative zero, sub-1.0
//!   - serde-integrity: round-trip with boundary values
//!   - parse-determinism: deterministic parse behavior
//!   - parse-error-priority: documented error priority order
//!   - trait-compliance: required trait implementations
//!   - proptest-property-attacks: fuzz RetryPolicy, next_nodes, parse

use crate::WorkflowDefinition;

mod helpers;

mod boundary_values;
mod contract_violations;
mod cycle_detection;
mod error_semantics;
mod json_attacks;
mod next_nodes;
mod parse_behavior;
mod prop_tests;

// RQ-26b: Exponential paths DAG (tests that memoization is present)
#[test]
fn rq_exponential_paths_dag_does_not_timeout() {
    let n = 40;

    let nodes: Vec<_> = (0..n)
        .map(|i| {
            serde_json::json!({
                "node_name": format!("n{}", i),
                "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
            })
        })
        .collect();

    let edges: Vec<_> = (0..n)
        .flat_map(|i| {
            let mut res = Vec::new();
            if i + 1 < n {
                res.push(serde_json::json!({
                    "source_node": format!("n{}", i),
                    "target_node": format!("n{}", i+1),
                    "condition": "Always"
                }));
            }
            if i + 2 < n {
                res.push(serde_json::json!({
                    "source_node": format!("n{}", i),
                    "target_node": format!("n{}", i+2),
                    "condition": "Always"
                }));
            }
            res
        })
        .collect();

    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": nodes,
        "edges": edges
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    result.unwrap();
}
