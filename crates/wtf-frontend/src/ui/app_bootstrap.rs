use oya_frontend::graph::{ExecutionState, Node, NodeCategory, NodeId, Viewport, Workflow};
use oya_frontend::graph::workflow_node::{WorkflowNode, HttpHandlerConfig, RunConfig, ConditionConfig};

pub fn default_workflow() -> Workflow {
    Workflow {
        nodes: vec![
            Node::from_workflow_node(
                "HTTP Handler".to_string(),
                WorkflowNode::HttpHandler(HttpHandlerConfig {
                    path: Some("/SignupWorkflow/{userId}/run".to_string()),
                    method: Some("POST".to_string()),
                }),
                350.0,
                40.0,
            ),
            Node::from_workflow_node(
                "Durable Step".to_string(),
                WorkflowNode::Run(RunConfig {
                    durable_step_name: Some("create-user".to_string()),
                    code: None,
                }),
                350.0,
                170.0,
            ),
            Node::from_workflow_node(
                "If / Else".to_string(),
                WorkflowNode::Condition(ConditionConfig {
                    expression: Some("Check if user creation succeeded".to_string()),
                }),
                350.0,
                300.0,
            ),
        ],
        connections: vec![],
        viewport: Viewport {
            x: 0.0,
            y: 0.0,
            zoom: 0.85,
        },
        execution_queue: vec![],
        current_step: 0,
        history: vec![],
        execution_records: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::default_workflow;
    use oya_frontend::graph::NodeCategory;

    #[test]
    fn given_default_workflow_when_created_then_it_contains_expected_starter_nodes() {
        let workflow = default_workflow();

        assert_eq!(workflow.nodes.len(), 3);
        assert_eq!(workflow.nodes[0].node_type, "http-handler");
        assert_eq!(workflow.nodes[1].node_type, "run");
        assert_eq!(workflow.nodes[2].node_type, "condition");
        assert_eq!(workflow.nodes[0].category, NodeCategory::Entry);
    }

    #[test]
    fn given_default_workflow_when_created_then_viewport_defaults_are_expected() {
        let workflow = default_workflow();

        assert_eq!(workflow.viewport.x, 0.0);
        assert_eq!(workflow.viewport.y, 0.0);
        assert_eq!(workflow.viewport.zoom, 0.85);
    }
}
