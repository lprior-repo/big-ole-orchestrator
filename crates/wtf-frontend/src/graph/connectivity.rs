use uuid::Uuid;

use super::{Connection, NodeId, PortName, Workflow};
use crate::graph::restate_types::types_compatible;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionError {
    SelfConnection,
    MissingSourceNode(NodeId),
    MissingTargetNode(NodeId),
    WouldCreateCycle,
    Duplicate,
    TypeMismatch {
        source_type: String,
        target_type: String,
    },
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelfConnection => write!(f, "Cannot connect node to itself"),
            Self::MissingSourceNode(node_id) => {
                write!(f, "Source node not found: {node_id}")
            }
            Self::MissingTargetNode(node_id) => {
                write!(f, "Target node not found: {node_id}")
            }
            Self::WouldCreateCycle => write!(f, "Connection would create a cycle"),
            Self::Duplicate => write!(f, "Connection already exists"),
            Self::TypeMismatch {
                source_type,
                target_type,
            } => write!(
                f,
                "Type mismatch: {source_type} is not compatible with {target_type}"
            ),
        }
    }
}

impl std::error::Error for ConnectionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionResult {
    Created,
}

impl Workflow {
    pub fn add_connection(
        &mut self,
        source: NodeId,
        target: NodeId,
        source_port: &PortName,
        target_port: &PortName,
    ) -> bool {
        self.add_connection_checked(source, target, source_port, target_port)
            .is_ok()
    }

    /// Adds a connection with full type checking.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError` if:
    /// - `source` and `target` are the same node
    /// - Either endpoint does not exist in the workflow
    /// - The connection would create a cycle
    /// - An identical connection already exists
    /// - Source and target port types are incompatible
    pub fn add_connection_checked(
        &mut self,
        source: NodeId,
        target: NodeId,
        source_port: &PortName,
        target_port: &PortName,
    ) -> Result<ConnectionResult, ConnectionError> {
        if source == target {
            return Err(ConnectionError::SelfConnection);
        }

        if !self.nodes.iter().any(|node| node.id == source) {
            return Err(ConnectionError::MissingSourceNode(source));
        }

        if !self.nodes.iter().any(|node| node.id == target) {
            return Err(ConnectionError::MissingTargetNode(target));
        }

        if Self::path_exists(&self.connections, target, source) {
            return Err(ConnectionError::WouldCreateCycle);
        }

        if self.connections.iter().any(|c| {
            c.source == source
                && c.target == target
                && c.source_port == *source_port
                && c.target_port == *target_port
        }) {
            return Err(ConnectionError::Duplicate);
        }

        Self::check_port_type_compatibility(&self.nodes, source, target)?;

        self.connections.push(Connection {
            id: Uuid::new_v4(),
            source,
            target,
            source_port: source_port.clone(),
            target_port: target_port.clone(),
        });

        Ok(ConnectionResult::Created)
    }

    fn check_port_type_compatibility(
        nodes: &[super::Node],
        source: NodeId,
        target: NodeId,
    ) -> Result<(), ConnectionError> {
        let source_node = match nodes.iter().find(|n| n.id == source) {
            Some(node) => node,
            None => return Err(ConnectionError::MissingSourceNode(source)),
        };
        let target_node = match nodes.iter().find(|n| n.id == target) {
            Some(node) => node,
            None => return Err(ConnectionError::MissingTargetNode(target)),
        };

        let source_type = Self::get_node_output_port_type(source_node);
        let target_type = Self::get_node_input_port_type(target_node);

        if let (Some(src), Some(tgt)) = (source_type, target_type) {
            if !types_compatible(src, tgt) {
                return Err(ConnectionError::TypeMismatch {
                    source_type: src.to_string(),
                    target_type: tgt.to_string(),
                });
            }
        }

        Ok(())
    }

    fn get_node_output_port_type(
        node: &super::Node,
    ) -> Option<crate::graph::restate_types::PortType> {
        node.node_type
            .parse::<crate::graph::workflow_node::WorkflowNode>()
            .ok()
            .map(|workflow_node| workflow_node.output_port_type())
    }

    fn get_node_input_port_type(
        node: &super::Node,
    ) -> Option<crate::graph::restate_types::PortType> {
        node.node_type
            .parse::<crate::graph::workflow_node::WorkflowNode>()
            .ok()
            .map(|workflow_node| workflow_node.input_port_type())
    }

    fn path_exists(connections: &[Connection], from: NodeId, to: NodeId) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![from];

        while let Some(current) = stack.pop() {
            if current == to {
                return true;
            }

            if visited.insert(current) {
                connections
                    .iter()
                    .filter(|connection| connection.source == current)
                    .for_each(|connection| stack.push(connection.target));
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn given_self_connection_when_adding_checked_connection_then_self_connection_error_is_returned()
    {
        let mut workflow = Workflow::new();
        let node_id = workflow.add_node("http-handler", 0.0, 0.0);
        let main = PortName("main".to_string());

        let result = workflow.add_connection_checked(node_id, node_id, &main, &main);

        assert_eq!(result, Err(ConnectionError::SelfConnection));
    }

    #[test]
    fn given_duplicate_connection_when_adding_checked_connection_then_duplicate_error_is_returned()
    {
        let mut workflow = Workflow::new();
        let source = workflow.add_node("http-handler", 0.0, 0.0);
        let target = workflow.add_node("run", 100.0, 0.0);
        let main = PortName("main".to_string());

        let first = workflow.add_connection_checked(source, target, &main, &main);
        assert!(matches!(first, Ok(ConnectionResult::Created)));

        let duplicate = workflow.add_connection_checked(source, target, &main, &main);

        assert_eq!(duplicate, Err(ConnectionError::Duplicate));
    }

    #[test]
    fn given_back_edge_when_adding_checked_connection_then_cycle_error_is_returned() {
        let mut workflow = Workflow::new();
        let first = workflow.add_node("http-handler", 0.0, 0.0);
        let second = workflow.add_node("run", 100.0, 0.0);
        let main = PortName("main".to_string());

        let created = workflow.add_connection_checked(first, second, &main, &main);
        assert!(matches!(created, Ok(ConnectionResult::Created)));

        let cycle = workflow.add_connection_checked(second, first, &main, &main);

        assert_eq!(cycle, Err(ConnectionError::WouldCreateCycle));
    }

    #[test]
    fn given_type_mismatch_ports_when_adding_checked_connection_then_type_mismatch_error_is_returned(
    ) {
        let mut workflow = Workflow::new();
        let source = workflow.add_node("condition", 0.0, 0.0);
        let target = workflow.add_node("signal-handler", 100.0, 0.0);
        let main = PortName("main".to_string());

        let result = workflow.add_connection_checked(source, target, &main, &main);

        assert_eq!(
            result,
            Err(ConnectionError::TypeMismatch {
                source_type: "flow-control".to_string(),
                target_type: "signal".to_string(),
            })
        );
    }

    #[test]
    fn given_missing_source_when_adding_checked_connection_then_source_not_found_error_is_returned()
    {
        let mut workflow = Workflow::new();
        let target = workflow.add_node("run", 100.0, 0.0);
        let main = PortName("main".to_string());

        let missing_source = NodeId(Uuid::new_v4());
        let result = workflow.add_connection_checked(missing_source, target, &main, &main);

        assert_eq!(
            result,
            Err(ConnectionError::MissingSourceNode(missing_source))
        );
    }

    #[test]
    fn given_missing_target_when_adding_checked_connection_then_target_not_found_error_is_returned()
    {
        let mut workflow = Workflow::new();
        let source = workflow.add_node("http-handler", 0.0, 0.0);
        let main = PortName("main".to_string());

        let missing_target = NodeId(Uuid::new_v4());
        let result = workflow.add_connection_checked(source, missing_target, &main, &main);

        assert_eq!(
            result,
            Err(ConnectionError::MissingTargetNode(missing_target))
        );
    }

    #[test]
    fn given_compatible_ports_when_adding_checked_connection_then_connection_is_created() {
        let mut workflow = Workflow::new();
        let source = workflow.add_node("http-handler", 0.0, 0.0);
        let target = workflow.add_node("run", 100.0, 0.0);
        let main = PortName("main".to_string());

        let result = workflow.add_connection_checked(source, target, &main, &main);

        assert_eq!(result, Ok(ConnectionResult::Created));
        assert_eq!(workflow.connections.len(), 1);
    }
}
