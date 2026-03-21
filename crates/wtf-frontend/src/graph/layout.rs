use crate::graph::{NodeId, Workflow};
use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;
use petgraph::Graph;
use std::collections::HashMap;

pub struct DagLayout {
    pub layer_spacing: f32,
    pub node_spacing: f32,
}

impl Default for DagLayout {
    fn default() -> Self {
        Self {
            layer_spacing: 140.0,
            node_spacing: 60.0,
        }
    }
}

const NODE_WIDTH: f32 = 220.0;
const NODE_HEIGHT: f32 = 68.0;
const LEFT_PADDING: f32 = 120.0;
const TOP_PADDING: f32 = 80.0;

impl DagLayout {
    #[allow(
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::items_after_statements
    )]
    pub fn apply(&self, workflow: &mut Workflow) {
        if workflow.nodes.is_empty() {
            return;
        }

        let mut graph = Graph::<NodeId, ()>::new();
        let mut index_map = HashMap::new();
        let mut reverse_map = HashMap::new();
        let mut node_order = HashMap::new();

        // Add nodes to petgraph
        for (order, node) in workflow.nodes.iter().enumerate() {
            let idx = graph.add_node(node.id);
            index_map.insert(node.id, idx);
            reverse_map.insert(idx, node.id);
            node_order.insert(idx, order);
        }

        // Add edges
        for conn in &workflow.connections {
            if let (Some(&src), Some(&tgt)) =
                (index_map.get(&conn.source), index_map.get(&conn.target))
            {
                graph.add_edge(src, tgt, ());
            }
        }

        // 1. Cycle Removal / Toposort
        let Ok(sorted_indices) = toposort(&graph, None) else {
            // If cyclic, try to layout what we can or return
            return;
        };

        // 2. Layer Assignment (Longest Path Layering)
        let mut layers: HashMap<NodeIndex, usize> = HashMap::new();
        for &node_idx in &sorted_indices {
            let layer = graph
                .neighbors_directed(node_idx, petgraph::Direction::Incoming)
                .filter_map(|parent| layers.get(&parent).map(|&l| l + 1))
                .max()
                .map_or(0, |layer| layer);
            layers.insert(node_idx, layer);
        }

        // Group by layer in deterministic topological order
        let mut nodes_by_layer: Vec<Vec<NodeIndex>> = Vec::new();
        for &node_idx in &sorted_indices {
            let layer = layers.get(&node_idx).map_or(0, |value| *value);
            while nodes_by_layer.len() <= layer {
                nodes_by_layer.push(Vec::new());
            }
            nodes_by_layer[layer].push(node_idx);
        }

        // 3. Crossing minimization (barycenter sweep)
        for _ in 0..4 {
            for layer_idx in 1..nodes_by_layer.len() {
                let mut barycenters: Vec<(NodeIndex, f32)> = nodes_by_layer[layer_idx]
                    .iter()
                    .map(|&node| {
                        let (sum, count) = graph
                            .neighbors_directed(node, petgraph::Direction::Incoming)
                            .filter_map(|parent| {
                                nodes_by_layer[layer_idx - 1]
                                    .iter()
                                    .position(|&n| n == parent)
                                    .map(|pos| pos as f32)
                            })
                            .fold((0.0, 0.0), |(s, c), pos| (s + pos, c + 1.0));

                        let barycenter = if count > 0.0 { sum / count } else { 0.0 };
                        (node, barycenter)
                    })
                    .collect();

                barycenters.sort_by(|a, b| {
                    let order_a = node_order.get(&a.0).map_or(usize::MAX, |value| *value);
                    let order_b = node_order.get(&b.0).map_or(usize::MAX, |value| *value);
                    a.1.total_cmp(&b.1).then_with(|| order_a.cmp(&order_b))
                });
                nodes_by_layer[layer_idx] = barycenters.into_iter().map(|(n, _)| n).collect();
            }
        }

        // 4. Coordinate assignment (left-to-right layered layout)
        let mut y_by_index: HashMap<NodeIndex, f32> = HashMap::new();
        let mut max_layer_height = 0.0_f32;

        for (layer, nodes) in nodes_by_layer.iter().enumerate() {
            let mut placed_y: Vec<f32> = Vec::new();

            for node_idx in nodes {
                let parent_positions = graph
                    .neighbors_directed(*node_idx, petgraph::Direction::Incoming)
                    .filter_map(|parent| y_by_index.get(&parent).copied())
                    .collect::<Vec<_>>();

                let preferred_y = if parent_positions.is_empty() {
                    0.0
                } else {
                    parent_positions.iter().sum::<f32>() / (parent_positions.len() as f32)
                };

                let y = placed_y.last().map_or(preferred_y, |prev| {
                    preferred_y.max(*prev + NODE_HEIGHT + self.node_spacing)
                });
                placed_y.push(y);
                y_by_index.insert(*node_idx, y);
            }

            if let (Some(first), Some(last)) = (placed_y.first(), placed_y.last()) {
                let layer_height = (last - first + NODE_HEIGHT).max(0.0);
                max_layer_height = max_layer_height.max(layer_height);
            }

            let x = (layer as f32) * (NODE_WIDTH + self.layer_spacing);
            for node_idx in nodes {
                if let Some(node_id) = reverse_map.get(node_idx) {
                    if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == *node_id) {
                        node.x = x;
                    }
                }
            }
        }

        for nodes in &nodes_by_layer {
            let layer_positions = nodes
                .iter()
                .filter_map(|idx| y_by_index.get(idx).copied())
                .collect::<Vec<_>>();
            if layer_positions.is_empty() {
                continue;
            }

            let first = layer_positions.first().map_or(0.0, |v| *v);
            let last = layer_positions.last().map_or(0.0, |v| *v);
            let layer_height = (last - first + NODE_HEIGHT).max(0.0);
            let layer_offset = (max_layer_height - layer_height) / 2.0;

            for node_idx in nodes {
                if let Some(node_id) = reverse_map.get(node_idx) {
                    if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == *node_id) {
                        let y = y_by_index.get(node_idx).map_or(0.0, |value| *value);
                        node.y = y + layer_offset;
                    }
                }
            }
        }

        let min_x = workflow
            .nodes
            .iter()
            .map(|node| node.x)
            .reduce(f32::min)
            .map_or(0.0, |value| value);
        let min_y = workflow
            .nodes
            .iter()
            .map(|node| node.y)
            .reduce(f32::min)
            .map_or(0.0, |value| value);

        for node in &mut workflow.nodes {
            node.x = node.x - min_x + LEFT_PADDING;
            node.y = node.y - min_y + TOP_PADDING;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DagLayout, LEFT_PADDING, TOP_PADDING};
    use crate::graph::{Connection, PortName, Workflow};

    #[test]
    fn given_cycle_when_applying_layout_then_node_positions_remain_unchanged() {
        let mut workflow = Workflow::new();
        let a = workflow.add_node("run", 10.0, 20.0);
        let b = workflow.add_node("run", 40.0, 50.0);
        let before: Vec<(f32, f32)> = workflow.nodes.iter().map(|n| (n.x, n.y)).collect();

        workflow.connections.push(Connection {
            id: uuid::Uuid::new_v4(),
            source: a,
            target: b,
            source_port: PortName::from("main"),
            target_port: PortName::from("main"),
        });
        workflow.connections.push(Connection {
            id: uuid::Uuid::new_v4(),
            source: b,
            target: a,
            source_port: PortName::from("main"),
            target_port: PortName::from("main"),
        });

        DagLayout::default().apply(&mut workflow);

        let after: Vec<(f32, f32)> = workflow.nodes.iter().map(|n| (n.x, n.y)).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn given_disconnected_graph_when_applying_layout_then_nodes_have_distinct_y_positions() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("run", 0.0, 0.0);
        let _ = workflow.add_node("run", 0.0, 0.0);
        let _ = workflow.add_node("run", 0.0, 0.0);

        DagLayout::default().apply(&mut workflow);

        let mut ys: Vec<f32> = workflow.nodes.iter().map(|n| n.y).collect();
        ys.sort_by(f32::total_cmp);
        ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        assert_eq!(ys.len(), 3);
    }

    #[test]
    fn given_same_graph_when_applying_twice_then_positions_are_deterministic() {
        let mut workflow = Workflow::new();
        let n1 = workflow.add_node("http-handler", 0.0, 0.0);
        let n2 = workflow.add_node("run", 0.0, 0.0);
        let n3 = workflow.add_node("condition", 0.0, 0.0);
        let main = PortName::from("main");
        let _ = workflow.add_connection(n1, n2, &main, &main);
        let _ = workflow.add_connection(n2, n3, &main, &main);

        let layout = DagLayout::default();
        layout.apply(&mut workflow);
        let once: Vec<(String, f32, f32)> = workflow
            .nodes
            .iter()
            .map(|n| (n.name.clone(), n.x, n.y))
            .collect();

        layout.apply(&mut workflow);
        let twice: Vec<(String, f32, f32)> = workflow
            .nodes
            .iter()
            .map(|n| (n.name.clone(), n.x, n.y))
            .collect();

        assert_eq!(once, twice);
    }

    #[test]
    fn given_layout_result_when_normalized_then_minimum_coordinates_match_padding() {
        let mut workflow = Workflow::new();
        let _ = workflow.add_node("run", -500.0, -400.0);
        let _ = workflow.add_node("run", -350.0, -200.0);

        DagLayout::default().apply(&mut workflow);

        let min_x = workflow.nodes.iter().map(|n| n.x).reduce(f32::min);
        let min_y = workflow.nodes.iter().map(|n| n.y).reduce(f32::min);
        assert!(min_x.is_some_and(|value| (value - LEFT_PADDING).abs() < 0.001));
        assert!(min_y.is_some_and(|value| (value - TOP_PADDING).abs() < 0.001));
    }
}
