use super::expressions::ExpressionContext;
use super::{ExecutionState, NodeCategory, NodeId, RunRecord, Workflow};

impl Workflow {
    fn compare_execution_priority(&self, a: NodeId, b: NodeId) -> std::cmp::Ordering {
        let node_a = self.nodes.iter().find(|node| node.id == a);
        let node_b = self.nodes.iter().find(|node| node.id == b);

        match (node_a, node_b) {
            (Some(left), Some(right)) => left
                .x
                .partial_cmp(&right.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.y
                        .partial_cmp(&right.y)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.name.cmp(&right.name)),
            _ => std::cmp::Ordering::Equal,
        }
    }

    #[must_use]
    pub fn resolve_expressions(&self, config: &serde_json::Value) -> serde_json::Value {
        let ctx = ExpressionContext::new(&self.nodes);
        match config {
            serde_json::Value::String(s) => {
                if s.starts_with("{{") && s.ends_with("}}") {
                    let inner = s[2..s.len() - 2].trim();
                    return ctx.resolve(inner);
                }
                config.clone()
            }
            serde_json::Value::Object(map) => {
                let new_map = map
                    .iter()
                    .map(|(k, v)| (k.clone(), self.resolve_expressions(v)))
                    .collect();
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.resolve_expressions(v)).collect())
            }
            _ => config.clone(),
        }
    }

    pub fn prepare_run(&mut self) {
        let mut queue = Vec::new();
        let node_ids: std::collections::HashSet<NodeId> = self.nodes.iter().map(|n| n.id).collect();
        let mut in_degree: std::collections::HashMap<NodeId, usize> =
            self.nodes.iter().map(|n| (n.id, 0)).collect();

        self.connections.iter().for_each(|conn| {
            if node_ids.contains(&conn.source) && node_ids.contains(&conn.target) {
                if let Some(count) = in_degree.get_mut(&conn.target) {
                    *count += 1;
                }
            }
        });

        let mut available: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|n| in_degree.get(&n.id).is_some_and(|&d| d == 0))
            .map(|n| n.id)
            .collect();

        available.sort_by(|a, b| self.compare_execution_priority(*b, *a));

        while let Some(id) = available.pop() {
            queue.push(id);
            self.connections
                .iter()
                .filter(|c| {
                    c.source == id && node_ids.contains(&c.source) && node_ids.contains(&c.target)
                })
                .for_each(|conn| {
                    if let Some(count) = in_degree.get_mut(&conn.target) {
                        *count -= 1;
                        if *count == 0 {
                            available.push(conn.target);
                            available.sort_by(|a, b| self.compare_execution_priority(*b, *a));
                        }
                    }
                });
        }

        self.execution_queue = queue;
        self.current_step = 0;
        self.nodes.iter_mut().for_each(|node| {
            node.executing = false;
            node.last_output = None;
            node.skipped = false;
            node.error = None;
            Self::set_node_status(node, ExecutionState::Queued);
        });
    }

    fn collect_descendants(&self, start_ids: &[NodeId]) -> std::collections::HashSet<NodeId> {
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<NodeId> = start_ids.to_vec();

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            for target in self
                .connections
                .iter()
                .filter(|c| c.source == current)
                .map(|c| c.target)
            {
                if !visited.contains(&target) {
                    stack.push(target);
                }
            }
        }

        visited
    }

    async fn execute_node_type(
        &self,
        node_type_str: &str,
        resolved_config: &serde_json::Value,
        parent_outputs: &[serde_json::Value],
    ) -> serde_json::Value {
        match node_type_str {
            "http-handler" | "kafka-handler" | "cron-trigger" => serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "source": node_type_str
            }),
            "run" => resolved_config
                .get("mapping")
                .cloned()
                .map_or_else(|| serde_json::json!({}), std::convert::identity),
            "service-call" | "object-call" | "workflow-call" => {
                self.execute_service_call(resolved_config).await
            }
            "condition" => {
                let condition = resolved_config
                    .get("expression")
                    .and_then(serde_json::Value::as_str)
                    .map_or("false", |s| s);
                let result = condition == "true" || (!condition.is_empty() && condition != "false");
                serde_json::json!({ "result": result, "condition": condition })
            }
            _ => serde_json::json!({
                "executed": true,
                "step": self.current_step,
                "input_count": parent_outputs.len(),
                "config": resolved_config
            }),
        }
    }

    async fn execute_service_call(
        &self,
        _resolved_config: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({ "executed": true })
    }

    async fn execute_http_request(&self, config: &serde_json::Value) -> serde_json::Value {
        let url = config
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map_or("https://httpbin.org/get", |s| s);
        let method = config
            .get("method")
            .and_then(serde_json::Value::as_str)
            .map_or("GET", |s| s);

        let client = reqwest::Client::new();
        let rb = match method {
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            _ => client.get(url),
        };

        match rb.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_or_else(|_| serde_json::json!({}), std::convert::identity);
                serde_json::json!({ "status": status, "url": url, "body": body })
            }
            Err(e) => serde_json::json!({ "error": e.to_string(), "url": url }),
        }
    }

    pub async fn step(&mut self) -> bool {
        if self.current_step >= self.execution_queue.len() {
            self.nodes.iter_mut().for_each(|node| {
                node.executing = false;
            });
            return false;
        }

        let node_id = match self.execution_queue.get(self.current_step) {
            Some(id) => *id,
            None => return false,
        };

        if self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .is_some_and(|n| n.skipped)
        {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                Self::set_node_status(node, ExecutionState::Skipped);
            }
            self.current_step += 1;
            return true;
        }

        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.executing = true;
            Self::set_node_status(node, ExecutionState::Running);
        }

        let parent_outputs: Vec<serde_json::Value> = self
            .connections
            .iter()
            .filter(|c| c.target == node_id)
            .filter_map(|c| {
                self.nodes
                    .iter()
                    .find(|n| n.id == c.source)
                    .and_then(|n| n.last_output.clone())
            })
            .collect();

        if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
            let node_type = node.node_type.clone();
            let node_config_json = node.config.clone();
            let resolved_config = self.resolve_expressions(&node_config_json);
            let output = self
                .execute_node_type(&node_type, &resolved_config, &parent_outputs)
                .await;

            if node_type == "condition" {
                let result = output
                    .get("result")
                    .and_then(serde_json::Value::as_bool)
                    .is_some_and(|value| value);
                let skip_port = if result { "false" } else { "true" };

                // Collect all nodes in the non-taken branch (direct targets and descendants)
                let branch_targets: Vec<NodeId> = self
                    .connections
                    .iter()
                    .filter(|c| c.source == node_id && c.source_port.0 == skip_port)
                    .map(|c| c.target)
                    .collect();

                let branch_descendants = self.collect_descendants(&branch_targets);

                // Build the full skip set: condition node + all branch nodes
                let mut skip_set: std::collections::HashSet<NodeId> =
                    std::collections::HashSet::new();
                skip_set.insert(node_id);
                skip_set.extend(branch_descendants);

                // Only skip a node if ALL its incoming connections come from the skip set
                let target_ids: Vec<NodeId> = self.nodes.iter().map(|n| n.id).collect();
                for target_id in target_ids {
                    // Get all incoming connections to this target
                    let incoming: Vec<NodeId> = self
                        .connections
                        .iter()
                        .filter(|c| c.target == target_id)
                        .map(|c| c.source)
                        .collect();

                    // If all incoming connections are from the skip set, mark as skipped
                    if !incoming.is_empty() && incoming.iter().all(|src| skip_set.contains(src)) {
                        if let Some(target_node) = self.nodes.iter_mut().find(|n| n.id == target_id)
                        {
                            target_node.skipped = true;
                            Self::set_node_status(target_node, ExecutionState::Skipped);
                        }
                    }
                }
            }

            if let Some(n) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                if let Some(err) = output.get("error").and_then(serde_json::Value::as_str) {
                    n.error = Some(err.to_string());
                    Self::set_node_status(n, ExecutionState::Failed);
                } else {
                    Self::set_node_status(n, ExecutionState::Completed);
                }
                n.executing = false;
                n.last_output = Some(output);
            }
        }

        self.current_step += 1;
        true
    }

    pub async fn run(&mut self) {
        self.prepare_run();
        let start_time = chrono::Utc::now();
        let mut results = std::collections::HashMap::new();

        if self.nodes.is_empty()
            || !self
                .nodes
                .iter()
                .any(|node| node.category == NodeCategory::Entry)
        {
            self.history.push(RunRecord {
                id: uuid::Uuid::new_v4(),
                timestamp: start_time,
                results,
                success: false,
                restate_invocation_id: None,
            });
            if self.history.len() > 10 {
                let _ = self.history.remove(0);
            }
            return;
        }

        while self.step().await {
            if let Some(id) = self
                .execution_queue
                .get(self.current_step.saturating_sub(1))
            {
                if let Some(node) = self.nodes.iter().find(|n| n.id == *id) {
                    if let Some(out) = &node.last_output {
                        let _ = results.insert(*id, out.clone());
                    }
                }
            }
        }

        let success = self.nodes.iter().all(|node| {
            if node.error.is_some() {
                return false;
            }

            matches!(
                node.execution_state,
                ExecutionState::Completed | ExecutionState::Skipped
            )
        });
        self.history.push(RunRecord {
            id: uuid::Uuid::new_v4(),
            timestamp: start_time,
            results,
            success,
            restate_invocation_id: None,
        });

        if self.history.len() > 10 {
            let _ = self.history.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Connection, PortName};
    use serde_json::json;

    #[test]
    fn given_parallel_zero_indegree_nodes_when_preparing_run_then_order_is_deterministic_by_name() {
        let mut workflow = Workflow::new();
        let a = workflow.add_node("run", 0.0, 0.0);
        let b = workflow.add_node("run", 0.0, 0.0);
        let c = workflow.add_node("run", 0.0, 0.0);

        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == a) {
            node.name = "alpha".to_string();
            node.x = 500.0;
        }
        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == b) {
            node.name = "bravo".to_string();
            node.x = 300.0;
        }
        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == c) {
            node.name = "charlie".to_string();
            node.x = 100.0;
        }

        workflow.prepare_run();

        let names: Vec<String> = workflow
            .execution_queue
            .iter()
            .filter_map(|id| workflow.nodes.iter().find(|n| n.id == *id))
            .map(|n| n.name.clone())
            .collect();
        assert_eq!(names, vec!["charlie", "bravo", "alpha"]);
    }

    #[tokio::test]
    async fn given_condition_true_when_running_then_false_branch_nodes_are_marked_skipped() {
        let mut workflow = Workflow::new();
        let trigger = workflow.add_node("http-handler", 0.0, 0.0);
        let condition = workflow.add_node("condition", 0.0, 0.0);
        let false_branch = workflow.add_node("run", 0.0, 0.0);
        let main = PortName::from("main");

        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == condition) {
            node.config = json!({"condition": "true"});
        }

        let _ = workflow.add_connection(trigger, condition, &main, &main);
        workflow.connections.push(Connection {
            id: uuid::Uuid::new_v4(),
            source: condition,
            target: false_branch,
            source_port: PortName::from("false"),
            target_port: main,
        });

        workflow.run().await;

        let false_node = workflow.nodes.iter().find(|n| n.id == false_branch);
        assert!(false_node.is_some_and(|n| n.skipped));
    }

    #[test]
    fn given_nested_expression_placeholders_when_resolving_then_all_levels_are_resolved() {
        let mut workflow = Workflow::new();
        let source = workflow.add_node("run", 0.0, 0.0);
        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == source) {
            node.name = "Fetcher".to_string();
            node.last_output = Some(json!({"user": {"id": 7}}));
        }

        let config = json!({
            "outer": {
                "id": "{{ $node[\"Fetcher\"].json.user.id }}",
                "arr": ["{{ $node[\"Fetcher\"].json.user.id }}"]
            }
        });

        let resolved = workflow.resolve_expressions(&config);
        assert_eq!(resolved["outer"]["id"], json!(7));
        assert_eq!(resolved["outer"]["arr"][0], json!(7));
    }

    #[tokio::test]
    async fn given_node_with_custom_config_when_executing_then_uses_persisted_config() {
        let mut workflow = Workflow::new();
        let trigger = workflow.add_node("http-handler", 0.0, 0.0);
        let run_node = workflow.add_node("run", 100.0, 100.0);
        let main = PortName::from("main");
        let _ = workflow.add_connection(trigger, run_node, &main, &main);

        // Set custom config on the node - this is the user-edited config that gets persisted
        if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == run_node) {
            node.config = json!({
                "mapping": {
                    "custom_field": "custom_value",
                    "nested": {"key": "nested_value"}
                }
            });
        }

        workflow.run().await;

        // Verify the node executed and used the custom config from node.config
        let node = workflow.nodes.iter().find(|n| n.id == run_node);
        assert!(node.is_some_and(|n| n.execution_state == ExecutionState::Completed));

        // Verify the output contains the custom config fields
        if let Some(n) = node {
            let output = n.last_output.as_ref();
            assert!(output.is_some());
            let out = output.unwrap();
            assert_eq!(out.get("custom_field"), Some(&json!("custom_value")));
            assert_eq!(out.get("nested"), Some(&json!({"key": "nested_value"})));
        }
    }
}
