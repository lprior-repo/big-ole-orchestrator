#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use crate::ui::panel_types::{
    chevron_rotation_class, panel_height_class, CollapseState, InvocationStatus,
};
use dioxus::prelude::*;
use oya_frontend::graph::{ExecutionState, Node, NodeId, Workflow};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
struct PlanSnapshot {
    layers: Vec<Vec<NodeId>>,
    unscheduled: Vec<NodeId>,
}

fn node_invocation_status(node: &Node) -> InvocationStatus {
    match node.execution_state {
        ExecutionState::Running => InvocationStatus::Running,
        ExecutionState::Completed => InvocationStatus::Completed,
        ExecutionState::Failed => InvocationStatus::Failed,
        ExecutionState::Skipped => InvocationStatus::Skipped,
        ExecutionState::Idle | ExecutionState::Queued => InvocationStatus::Queued,
    }
}

fn status_badge_classes(status: InvocationStatus) -> &'static str {
    match status {
        InvocationStatus::Running => "bg-blue-50 text-blue-700 border-blue-200",
        InvocationStatus::Completed => "bg-emerald-50 text-emerald-700 border-emerald-200",
        InvocationStatus::Failed => "bg-rose-50 text-rose-700 border-rose-200",
        InvocationStatus::Skipped => "bg-amber-50 text-amber-700 border-amber-200",
        _ => "bg-slate-50 text-slate-600 border-slate-200",
    }
}

fn compare_node_ids(a: &NodeId, b: &NodeId, nodes: &HashMap<NodeId, Node>) -> std::cmp::Ordering {
    let left = nodes.get(a);
    let right = nodes.get(b);
    match (left, right) {
        (Some(ln), Some(rn)) => {
            ln.x.partial_cmp(&rn.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| ln.y.partial_cmp(&rn.y).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| ln.name.cmp(&rn.name))
        }
        _ => std::cmp::Ordering::Equal,
    }
}

fn build_plan_snapshot(workflow: &Workflow) -> PlanSnapshot {
    let nodes: HashMap<NodeId, Node> = workflow.nodes.iter().map(|n| (n.id, n.clone())).collect();
    let node_ids: HashSet<NodeId> = nodes.keys().copied().collect();
    let mut indegree: HashMap<NodeId, usize> = node_ids.iter().map(|id| (*id, 0)).collect();
    let mut outgoing: HashMap<NodeId, Vec<NodeId>> = node_ids
        .iter()
        .map(|id| (*id, Vec::<NodeId>::new()))
        .collect();

    workflow.connections.iter().for_each(|edge| {
        if node_ids.contains(&edge.source) && node_ids.contains(&edge.target) {
            if let Some(count) = indegree.get_mut(&edge.target) {
                *count += 1;
            }
            if let Some(targets) = outgoing.get_mut(&edge.source) {
                targets.push(edge.target);
            }
        }
    });

    let mut available: Vec<NodeId> = indegree
        .iter()
        .filter_map(|(id, count)| if *count == 0 { Some(*id) } else { None })
        .collect();
    available.sort_by(|a, b| compare_node_ids(a, b, &nodes));

    let mut visited = HashSet::<NodeId>::new();
    let mut layers = Vec::<Vec<NodeId>>::new();

    while !available.is_empty() {
        let current = available.clone();
        layers.push(current.clone());
        available.clear();

        for id in &current {
            let _ = visited.insert(*id);
            if let Some(targets) = outgoing.get(id) {
                for target in targets {
                    if let Some(count) = indegree.get_mut(target) {
                        *count = count.saturating_sub(1);
                    }
                }
            }
        }

        let mut next_ready: Vec<NodeId> = indegree
            .iter()
            .filter_map(|(id, count)| {
                if *count == 0 && !visited.contains(id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        next_ready.sort_by(|a, b| compare_node_ids(a, b, &nodes));
        available = next_ready;
    }

    let mut unscheduled: Vec<NodeId> = indegree
        .iter()
        .filter_map(|(id, count)| {
            if *count > 0 && !visited.contains(id) {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    unscheduled.sort_by(|a, b| compare_node_ids(a, b, &nodes));

    PlanSnapshot {
        layers,
        unscheduled,
    }
}

#[component]
pub fn ExecutionPlanPanel(
    workflow: ReadSignal<Workflow>,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    on_select_node: EventHandler<NodeId>,
    collapsed: Signal<bool>,
) -> Element {
    let collapse_state = CollapseState::from_bool(*collapsed.read());
    let height_class = panel_height_class(collapse_state);
    let chevron_class = chevron_rotation_class(collapse_state);

    let plan = {
        let wf = workflow.read();
        build_plan_snapshot(&wf)
    };

    let queue = workflow.read().execution_queue.clone();
    let current_step = workflow.read().current_step;

    rsx! {
        aside {
            class: "flex flex-col border-t border-slate-200 bg-white/95 transition-all duration-200 {height_class}",

            div {
                class: "flex items-center justify-between px-3 py-2 border-b border-slate-100",
                button {
                    class: "flex items-center gap-2 text-slate-700 hover:text-slate-900 transition-colors",
                    onclick: move |_| {
                        let _ = collapsed.try_write().map(|mut c| *c = !*c);
                    },
                    crate::ui::icons::LayersIcon { class: "h-4 w-4 text-slate-500" }
                    span { class: "text-[12px] font-semibold", "Execution Plan" }
                    span { class: "rounded bg-slate-100 px-1.5 py-0.5 text-[10px] text-slate-600", "{plan.layers.len()} layers" }
                    div { class: "transition-transform {chevron_class}",
                        crate::ui::icons::ChevronDownIcon { class: "h-3 w-3 text-slate-400" }
                    }
                }
            }

            if !collapse_state.is_collapsed() {
                div { class: "flex-1 overflow-y-auto px-3 py-2 space-y-2",
                    if queue.is_empty() {
                        p { class: "text-[11px] text-slate-500", "Queue preview (before run):" }
                    } else {
                        p { class: "text-[11px] text-slate-500", "Current run queue:" }
                        div { class: "rounded border border-slate-200 bg-slate-50 p-2 space-y-1",
                            for (idx, node_id) in queue.iter().enumerate() {
                                QueueItem {
                                    node_id: *node_id,
                                    index: idx,
                                    is_current: idx == current_step,
                                    nodes_by_id,
                                    on_select_node
                                }
                            }
                        }
                    }

                    p { class: "text-[11px] text-slate-500 pt-1", "Topological layers:" }
                    for (layer_idx, layer) in plan.layers.iter().enumerate() {
                        LayerSection {
                            layer_idx,
                            layer: layer.clone(),
                            nodes_by_id,
                            on_select_node
                        }
                    }

                    if !plan.unscheduled.is_empty() {
                        UnscheduledSection {
                            unscheduled: plan.unscheduled.clone(),
                            nodes_by_id,
                            on_select_node
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn QueueItem(
    node_id: NodeId,
    index: usize,
    is_current: bool,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    let node = nodes_by_id.read().get(&node_id).cloned();
    let label = node
        .as_ref()
        .map_or_else(|| "Unknown".to_string(), |n| n.name.clone());
    let status = node
        .as_ref()
        .map_or(InvocationStatus::Queued, node_invocation_status);
    let active_class = if is_current {
        "ring-1 ring-indigo-300 bg-indigo-50"
    } else {
        ""
    };
    let badge = status_badge_classes(status);

    rsx! {
        button {
            class: "flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-white {active_class}",
            key: "q-{index}",
            onclick: move |_| on_select_node.call(node_id),
            span { class: "font-mono text-[10px] text-slate-500 w-8", "#{index}" }
            span { class: "text-[11px] text-slate-700 flex-1 truncate", "{label}" }
            span { class: "text-[10px] px-1.5 py-0.5 rounded border {badge}", "{status.display_label()}" }
        }
    }
}

#[component]
fn LayerSection(
    layer_idx: usize,
    layer: Vec<NodeId>,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    rsx! {
        div { class: "rounded border border-slate-200 bg-white",
            div { class: "px-2 py-1 border-b border-slate-100 text-[10px] uppercase tracking-wide text-slate-500", "Layer {layer_idx}" }
            div { class: "p-1 space-y-1",
                for node_id in &layer {
                    LayerNodeItem {
                        node_id: *node_id,
                        nodes_by_id,
                        on_select_node
                    }
                }
            }
        }
    }
}

#[component]
fn LayerNodeItem(
    node_id: NodeId,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    let node = nodes_by_id.read().get(&node_id).cloned();
    let label = node
        .as_ref()
        .map_or_else(|| "Unknown".to_string(), |n| n.name.clone());
    let status = node
        .as_ref()
        .map_or(InvocationStatus::Queued, node_invocation_status);
    let badge = status_badge_classes(status);

    rsx! {
        button {
            class: "flex w-full items-center gap-2 rounded px-2 py-1 text-left hover:bg-slate-50",
            key: "node-{node_id}",
            onclick: move |_| on_select_node.call(node_id),
            span { class: "text-[11px] text-slate-700 flex-1 truncate", "{label}" }
            span { class: "text-[10px] px-1.5 py-0.5 rounded border {badge}", "{status.display_label()}" }
        }
    }
}

#[component]
fn UnscheduledSection(
    unscheduled: Vec<NodeId>,
    nodes_by_id: ReadSignal<HashMap<NodeId, Node>>,
    on_select_node: EventHandler<NodeId>,
) -> Element {
    rsx! {
        div { class: "rounded border border-amber-200 bg-amber-50 px-2 py-1.5",
            p { class: "text-[10px] font-semibold text-amber-800 uppercase tracking-wide", "Unscheduled" }
            p { class: "text-[10px] text-amber-700 mt-0.5", "Cycle or blocked dependency detected." }
            div { class: "mt-1 space-y-1",
                for node_id in &unscheduled {
                    {
                        let label = nodes_by_id
                            .read()
                            .get(node_id)
                            .map_or_else(|| "Unknown".to_string(), |n| n.name.clone());

                        rsx! {
                            button {
                                class: "w-full rounded bg-white/70 px-2 py-1 text-left text-[10px] text-amber-900 hover:bg-white",
                                key: "unsched-{node_id}",
                                onclick: move |_| on_select_node.call(*node_id),
                                "{label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_plan_snapshot, node_invocation_status, InvocationStatus};
    use oya_frontend::graph::{ExecutionState, Workflow};

    #[test]
    fn given_simple_chain_when_building_plan_then_layers_follow_dependency_order() {
        let mut workflow = Workflow::new();
        let a = workflow.add_node("http-handler", 0.0, 0.0);
        let b = workflow.add_node("run", 300.0, 0.0);
        let c = workflow.add_node("run", 600.0, 0.0);
        let main = oya_frontend::graph::PortName::from("main");
        let _ = workflow.add_connection(a, b, &main, &main);
        let _ = workflow.add_connection(b, c, &main, &main);

        let snapshot = build_plan_snapshot(&workflow);

        assert_eq!(snapshot.layers.len(), 3);
        assert_eq!(snapshot.layers[0], vec![a]);
        assert_eq!(snapshot.layers[1], vec![b]);
        assert_eq!(snapshot.layers[2], vec![c]);
        assert!(snapshot.unscheduled.is_empty());
    }

    #[test]
    fn given_parallel_starts_when_building_plan_then_both_nodes_in_same_layer() {
        let mut workflow = Workflow::new();
        let left = workflow.add_node("run", 100.0, 0.0);
        let right = workflow.add_node("run", 400.0, 0.0);

        let snapshot = build_plan_snapshot(&workflow);

        assert_eq!(snapshot.layers.len(), 1);
        assert_eq!(snapshot.layers[0].len(), 2);
        assert!(snapshot.layers[0].contains(&left));
        assert!(snapshot.layers[0].contains(&right));
    }

    #[test]
    fn given_cycle_when_building_plan_then_unscheduled_nodes_are_reported() {
        let mut workflow = Workflow::new();
        let a = workflow.add_node("run", 0.0, 0.0);
        let b = workflow.add_node("run", 100.0, 0.0);
        workflow.connections.push(oya_frontend::graph::Connection {
            id: uuid::Uuid::new_v4(),
            source: a,
            target: b,
            source_port: oya_frontend::graph::PortName::from("main"),
            target_port: oya_frontend::graph::PortName::from("main"),
        });
        workflow.connections.push(oya_frontend::graph::Connection {
            id: uuid::Uuid::new_v4(),
            source: b,
            target: a,
            source_port: oya_frontend::graph::PortName::from("main"),
            target_port: oya_frontend::graph::PortName::from("main"),
        });

        let snapshot = build_plan_snapshot(&workflow);

        assert!(snapshot.layers.is_empty());
        assert_eq!(snapshot.unscheduled.len(), 2);
    }

    #[test]
    fn given_failed_node_when_getting_invocation_status_then_failed_is_returned() {
        let mut workflow = Workflow::new();
        let id = workflow.add_node("run", 0.0, 0.0);

        let maybe_node = workflow.nodes.iter_mut().find(|n| n.id == id);
        if let Some(node) = maybe_node {
            node.execution_state = ExecutionState::Failed;
            assert_eq!(node_invocation_status(node), InvocationStatus::Failed);
        }
    }

    #[test]
    fn given_queued_node_when_getting_invocation_status_then_queued_is_returned() {
        let mut workflow = Workflow::new();
        let id = workflow.add_node("run", 0.0, 0.0);

        let maybe_node = workflow.nodes.iter_mut().find(|n| n.id == id);
        if let Some(node) = maybe_node {
            node.execution_state = ExecutionState::Queued;
            assert_eq!(node_invocation_status(node), InvocationStatus::Queued);
        }
    }
}
