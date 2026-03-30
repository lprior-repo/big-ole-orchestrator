import os

path = "crates/vo-types/src/red_queen_tests.rs"
with open(path, "r") as f:
    c = f.read()

c = c.replace(
"""    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));""",
"""    assert!(matches!(
        result,
        Err(RetryPolicyError::InvalidMultiplier { .. })
    ));"""
)

c = c.replace('assert!(result.is_err(), "NaN must be rejected");', 'assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "NaN must be rejected");')

c = c.replace("""    assert!(
        result.is_err(),
        "serde_json must reject NaN in JSON by default"
    );""", """    result.unwrap_err();""")

c = c.replace("""    assert!(
        result.is_err(),
        "serde_json must reject INFINITY in JSON by default"
    );""", """    result.unwrap_err();""")

c = c.replace("""    assert!(
        result.is_err(),
        "serde_json must reject -INFINITY in JSON by default"
    );""", """    result.unwrap_err();""")

c = c.replace("assert!(result.is_err());", "assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })));")

c = c.replace('prop_assert!(result.is_err(), "multiplier {} should be rejected", multiplier);', 'prop_assert!(matches!(result, Err(RetryPolicyError::InvalidMultiplier { .. })), "multiplier {} should be rejected", multiplier);')

c = c.replace("let _ = result.unwrap();", "result.unwrap();")

# append the test for RQ-26b
test_append = """
// RQ-26b: Exponential paths DAG (tests that memoization is present)
#[test]
fn rq_exponential_paths_dag_does_not_timeout() {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let n = 40;
    
    for i in 0..n {
        nodes.push(serde_json::json!({
            "node_name": format!("n{}", i),
            "retry_policy": {"max_attempts": 1, "backoff_ms": 0, "backoff_multiplier": 1.0}
        }));
        if i + 1 < n {
            edges.push(serde_json::json!({
                "source_node": format!("n{}", i),
                "target_node": format!("n{}", i+1),
                "condition": "Always"
            }));
        }
        if i + 2 < n {
            edges.push(serde_json::json!({
                "source_node": format!("n{}", i),
                "target_node": format!("n{}", i+2),
                "condition": "Always"
            }));
        }
    }
    
    let json = serde_json::json!({
        "workflow_name": "test",
        "nodes": nodes,
        "edges": edges
    });
    let bytes = serde_json::to_vec(&json).unwrap();
    let result = WorkflowDefinition::parse(&bytes);
    assert!(matches!(result, Ok(_)));
}
"""

if "rq_exponential_paths_dag_does_not_timeout" not in c:
    c += test_append

with open(path, "w") as f:
    f.write(c)
