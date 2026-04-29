use super::*;

#[test]
fn workflow_name_boundary_consistency_contract_underscore_prefix_is_valid() {
    let result = WorkflowName::parse("_valid");
    assert_eq!(
        result,
        Ok(WorkflowName("_valid".to_string())),
        "CONTRACT VIOLATION: is_identifier_char('_') returns true, therefore \
         WorkflowName::parse(\"_valid\") MUST return Ok, but it returned an error. \
         This is the bug that vel-205 fixes."
    );
}

#[test]
fn node_name_boundary_consistency_contract_underscore_prefix_is_valid() {
    let result = NodeName::parse("_node");
    assert_eq!(
        result,
        Ok(NodeName("_node".to_string())),
        "CONTRACT VIOLATION: NodeName::parse(\"_node\") must succeed per contract"
    );
}
