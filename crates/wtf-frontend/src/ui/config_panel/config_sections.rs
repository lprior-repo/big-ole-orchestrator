use super::{get_str_val, get_u64_val};
use crate::ui::panel_types::HttpMethod;
use dioxus::prelude::*;
use oya_frontend::graph::WorkflowNode;
use serde_json::Value;

#[component]
pub(super) fn EntryConfig(
    node: WorkflowNode,
    config: Value,
    update_str: EventHandler<(String, String)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        match node {
            WorkflowNode::HttpHandler(_) => {
                let method = HttpMethod::parse(&get_str_val(&config, "method"));
                rsx! {
                    FieldInput {
                        input_cls: input_cls,
                        label: "Path",
                        value: get_str_val(&config, "path"),
                        on_change: move |value: String| update_str.call(("path".to_string(), value))
                    }
                    div { class: "flex flex-col gap-1.5",
                        label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "HTTP Method" }
                        select {
                            class: "{input_cls}",
                            value: "{method}",
                            onchange: move |e| update_str.call((
                                "method".to_string(),
                                HttpMethod::parse(&e.value()).to_string(),
                            )),
                            for m in HttpMethod::all() {
                                option { value: "{m}", "{m}" }
                            }
                        }
                    }
                }
            }
            WorkflowNode::CronTrigger(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Schedule",
                    value: get_str_val(&config, "schedule"),
                    on_change: move |value: String| update_str.call(("schedule".to_string(), value))
                }
            },
            WorkflowNode::KafkaHandler(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Kafka Topic",
                    value: get_str_val(&config, "topic"),
                    on_change: move |value: String| update_str.call(("topic".to_string(), value))
                }
            },
            WorkflowNode::WorkflowSubmit(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Workflow Name",
                    value: get_str_val(&config, "workflow_name"),
                    on_change: move |value: String| update_str.call(("workflow_name".to_string(), value))
                }
            },
            _ => rsx! {},
        }
    }
}

#[component]
fn FieldInput(
    input_cls: &'static str,
    label: &'static str,
    value: String,
    on_change: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "{label}" }
            input { class: "{input_cls}", value: "{value}", oninput: move |e| on_change.call(e.value()) }
        }
    }
}

#[component]
pub(super) fn DurableConfig(
    node: WorkflowNode,
    config: Value,
    update_str: EventHandler<(String, String)>,
    update_u64: EventHandler<(String, u64)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        match node {
            WorkflowNode::Run(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Durable Step Name",
                    value: get_str_val(&config, "durable_step_name"),
                    on_change: move |value: String| update_str.call(("durable_step_name".to_string(), value))
                }
            },
            WorkflowNode::ServiceCall(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Durable Step Name",
                    value: get_str_val(&config, "durable_step_name"),
                    on_change: move |value: String| update_str.call(("durable_step_name".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Service",
                    value: get_str_val(&config, "service"),
                    on_change: move |value: String| update_str.call(("service".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Endpoint",
                    value: get_str_val(&config, "endpoint"),
                    on_change: move |value: String| update_str.call(("endpoint".to_string(), value))
                }
            },
            WorkflowNode::ObjectCall(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Durable Step Name",
                    value: get_str_val(&config, "durable_step_name"),
                    on_change: move |value: String| update_str.call(("durable_step_name".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Object Name",
                    value: get_str_val(&config, "object_name"),
                    on_change: move |value: String| update_str.call(("object_name".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Handler",
                    value: get_str_val(&config, "handler"),
                    on_change: move |value: String| update_str.call(("handler".to_string(), value))
                }
            },
            WorkflowNode::WorkflowCall(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Durable Step Name",
                    value: get_str_val(&config, "durable_step_name"),
                    on_change: move |value: String| update_str.call(("durable_step_name".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Workflow Name",
                    value: get_str_val(&config, "workflow_name"),
                    on_change: move |value: String| update_str.call(("workflow_name".to_string(), value))
                }
            },
            WorkflowNode::SendMessage(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Durable Step Name",
                    value: get_str_val(&config, "durable_step_name"),
                    on_change: move |value: String| update_str.call(("durable_step_name".to_string(), value))
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Target",
                    value: get_str_val(&config, "target"),
                    on_change: move |value: String| update_str.call(("target".to_string(), value))
                }
            },
            WorkflowNode::DelayedSend(_) => rsx! {
                div { class: "flex flex-col gap-1.5",
                    label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Delay (ms)" }
                    input {
                        r#type: "number",
                        class: "{input_cls}",
                        value: "{get_u64_val(&config, \"delay_ms\").map_or(0, |value| value)}",
                        oninput: move |e| {
                            if let Ok(value) = e.value().parse::<u64>() {
                                update_u64.call(("delay_ms".to_string(), value));
                            }
                        }
                    }
                }
                FieldInput {
                    input_cls: input_cls,
                    label: "Target",
                    value: get_str_val(&config, "target"),
                    on_change: move |value: String| update_str.call(("target".to_string(), value))
                }
            },
            _ => rsx! {},
        }
    }
}

#[component]
pub(super) fn StateConfig(
    node: WorkflowNode,
    config: Value,
    update_str: EventHandler<(String, String)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        FieldInput {
            input_cls: input_cls,
            label: "State Key",
            value: get_str_val(&config, "key"),
            on_change: move |value: String| update_str.call(("key".to_string(), value))
        }
        if matches!(node, WorkflowNode::SetState(_)) {
            FieldInput {
                input_cls: input_cls,
                label: "Value",
                value: get_str_val(&config, "value"),
                on_change: move |value: String| update_str.call(("value".to_string(), value))
            }
        }
    }
}

#[component]
pub(super) fn FlowConfig(
    node: WorkflowNode,
    config: Value,
    update_str: EventHandler<(String, String)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        match node {
            WorkflowNode::Condition(_) | WorkflowNode::Switch(_) => rsx! {
                div { class: "flex flex-col gap-1.5",
                    label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Expression" }
                    textarea {
                        class: "resize-none rounded-md border border-slate-700 bg-slate-950 px-3 py-2 font-mono text-[11px] text-slate-100 outline-none transition-colors focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/30",
                        rows: "2",
                        value: "{get_str_val(&config, \"expression\")}",
                        oninput: move |e| update_str.call(("expression".to_string(), e.value()))
                    }
                }
            },
            WorkflowNode::Loop(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Iterator",
                    value: get_str_val(&config, "iterator"),
                    on_change: move |value: String| update_str.call(("iterator".to_string(), value))
                }
            },
            WorkflowNode::Compensate(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Target Step",
                    value: get_str_val(&config, "target_step"),
                    on_change: move |value: String| update_str.call(("target_step".to_string(), value))
                }
            },
            _ => rsx! {},
        }
    }
}

#[component]
pub(super) fn TimingConfig(
    node: WorkflowNode,
    config: Value,
    update_u64: EventHandler<(String, u64)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        match node {
            WorkflowNode::Sleep(_) => rsx! {
                div { class: "flex flex-col gap-1.5",
                    label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Duration (ms)" }
                    input {
                        r#type: "number",
                        class: "{input_cls}",
                        value: "{get_u64_val(&config, \"duration_ms\").map_or(0, |value| value)}",
                        oninput: move |e| if let Ok(value) = e.value().parse::<u64>() { update_u64.call(("duration_ms".to_string(), value)); }
                    }
                }
            },
            WorkflowNode::Timeout(_) => rsx! {
                div { class: "flex flex-col gap-1.5",
                    label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Timeout (ms)" }
                    input {
                        r#type: "number",
                        class: "{input_cls}",
                        value: "{get_u64_val(&config, \"timeout_ms\").map_or(0, |value| value)}",
                        oninput: move |e| if let Ok(value) = e.value().parse::<u64>() { update_u64.call(("timeout_ms".to_string(), value)); }
                    }
                }
            },
            _ => rsx! {},
        }
    }
}

#[component]
pub(super) fn SignalConfig(
    node: WorkflowNode,
    config: Value,
    update_str: EventHandler<(String, String)>,
    input_cls: &'static str,
) -> Element {
    rsx! {
        match node {
            WorkflowNode::DurablePromise(_) | WorkflowNode::ResolvePromise(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Promise Name",
                    value: get_str_val(&config, "promise_name"),
                    on_change: move |value: String| update_str.call(("promise_name".to_string(), value))
                }
            },
            WorkflowNode::Awakeable(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Awakeable ID",
                    value: get_str_val(&config, "awakeable_id"),
                    on_change: move |value: String| update_str.call(("awakeable_id".to_string(), value))
                }
            },
            WorkflowNode::SignalHandler(_) => rsx! {
                FieldInput {
                    input_cls: input_cls,
                    label: "Signal Name",
                    value: get_str_val(&config, "signal_name"),
                    on_change: move |value: String| update_str.call(("signal_name".to_string(), value))
                }
            },
            _ => rsx! {},
        }
    }
}
