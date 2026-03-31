use dioxus::prelude::*;
use oya_frontend::graph::{Node, NodeCategory};
use serde_json::Value;

use super::config_panel::{get_str_val, get_u64_val};
use super::domain_types::HttpMethod;

const INPUT_CLASS: &str =
    "h-7 w-full rounded border border-slate-300 bg-white px-2 font-mono text-[11px] text-slate-800 outline-none transition-colors focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/30";

fn parse_http_method(config: &Value, key: &str) -> HttpMethod {
    get_str_val(config, key)
        .parse::<HttpMethod>()
        .unwrap_or_default()
}

#[component]
pub fn InlineConfigPanel(
    node: Node,
    on_change: EventHandler<Value>,
    on_close: EventHandler<()>,
) -> Element {
    let config = node.config.clone();
    let category = node.category;
    let icon = node.icon.clone();

    rsx! {
        div {
            class: "mt-2 rounded-lg border border-slate-300 bg-white/95 p-2.5 shadow-lg",
            onclick: move |e| e.stop_propagation(),
            ondoubleclick: move |e| e.stop_propagation(),

            div { class: "mb-2 flex items-center justify-between",
                span { class: "text-[10px] font-semibold uppercase tracking-wide text-slate-600", "Quick Edit" }
                button {
                    class: "flex h-5 w-5 items-center justify-center rounded text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600",
                    onclick: move |_| on_close.call(()),
                    crate::ui::icons::XIcon { class: "h-3 w-3" }
                }
            }

            div { class: "flex flex-col gap-2",
                {match category {
                    NodeCategory::Entry => entry_config(&icon, &config, on_change),
                    NodeCategory::Durable => durable_config(&icon, &config, on_change),
                    NodeCategory::State => state_config(&config, on_change),
                    NodeCategory::Flow => flow_config(&icon, &config, on_change),
                    NodeCategory::Timing => timing_config(&icon, &config, on_change),
                    NodeCategory::Signal => signal_config(&icon, &config, on_change),
                }}
            }
        }
    }
}

fn entry_config(icon: &str, config: &Value, on_change: EventHandler<Value>) -> Element {
    match icon {
        "globe" => {
            let method = parse_http_method(config, "method");
            let config_clone = config.clone();
            rsx! {
                {text_field("Path", "path", config, "/orders/{order_id}", on_change.clone())}
                div { class: "flex flex-col gap-0.5",
                    label { class: "text-[9px] font-medium uppercase tracking-wide text-slate-500", "Method" }
                    select {
                        class: "{INPUT_CLASS}",
                        value: "{method}",
                        onchange: move |e| {
                            let mut new_config = config_clone.clone();
                            if let Some(obj) = new_config.as_object_mut() {
                                let parsed: HttpMethod = e.value().parse().unwrap_or_default();
                                obj.insert("method".to_string(), Value::String(parsed.to_string()));
                                on_change.call(new_config);
                            }
                        },
                        option { value: "GET", "GET" }
                        option { value: "POST", "POST" }
                        option { value: "PUT", "PUT" }
                        option { value: "DELETE", "DELETE" }
                        option { value: "PATCH", "PATCH" }
                    }
                }
            }
        }
        "clock" => text_field("Schedule", "schedule", config, "0 */5 * * *", on_change),
        "kafka" => text_field("Topic", "topic", config, "orders-topic", on_change),
        "play" => text_field(
            "Workflow Name",
            "workflow_name",
            config,
            "SignupWorkflow",
            on_change,
        ),
        _ => rsx! { div { class: "text-[10px] italic text-slate-400", "No quick config" } },
    }
}

fn durable_config(icon: &str, config: &Value, on_change: EventHandler<Value>) -> Element {
    let step_field = text_field(
        "Step Name",
        "durableStepName",
        config,
        "create-user",
        on_change,
    );

    if icon == "clock-send" {
        let delay_field = text_field("Delay", "sleepDuration", config, "1h", on_change);
        rsx! {
            {step_field}
            {delay_field}
        }
    } else {
        let target_field = text_field(
            "Target",
            "targetService",
            config,
            "PaymentService",
            on_change,
        );
        rsx! {
            {step_field}
            {target_field}
        }
    }
}

fn state_config(config: &Value, on_change: EventHandler<Value>) -> Element {
    text_field("State Key", "stateKey", config, "cart", on_change)
}

fn flow_config(icon: &str, config: &Value, on_change: EventHandler<Value>) -> Element {
    match icon {
        "git-branch" => {
            let config_clone = config.clone();
            let value = get_str_val(config, "conditionExpression");
            rsx! {
                div { class: "flex flex-col gap-0.5",
                    label { class: "text-[9px] font-medium uppercase tracking-wide text-slate-500", "Condition" }
                    textarea {
                        class: "resize-none rounded border border-slate-300 bg-white px-2 py-1 font-mono text-[10px] text-slate-800 outline-none transition-colors focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/30",
                        rows: "2",
                        placeholder: "user.verified === true",
                        value: "{value}",
                        oninput: move |e| {
                            let mut new_config = config_clone.clone();
                            if let Some(obj) = new_config.as_object_mut() {
                                obj.insert("conditionExpression".to_string(), Value::String(e.value()));
                                on_change.call(new_config);
                            }
                        }
                    }
                }
            }
        }
        "repeat" => text_field("Iterator", "loopIterator", config, "items", on_change),
        "undo" => text_field(
            "Compensation",
            "compensationHandler",
            config,
            "refundPayment",
            on_change,
        ),
        _ => rsx! { div { class: "text-[10px] italic text-slate-400", "No quick config" } },
    }
}

fn timing_config(icon: &str, config: &Value, on_change: EventHandler<Value>) -> Element {
    match icon {
        "timer" => text_field("Duration", "sleepDuration", config, "5m", on_change),
        "alarm" => {
            let config_clone = config.clone();
            let val = get_u64_val(config, "timeoutMs").unwrap_or(30000);
            rsx! {
                div { class: "flex flex-col gap-0.5",
                    label { class: "text-[9px] font-medium uppercase tracking-wide text-slate-500", "Timeout (ms)" }
                    input {
                        r#type: "number",
                        class: "{INPUT_CLASS}",
                        placeholder: "30000",
                        value: "{val}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u64>() {
                                let mut new_config = config_clone.clone();
                                if let Some(obj) = new_config.as_object_mut() {
                                    obj.insert("timeoutMs".to_string(), Value::Number(v.into()));
                                    on_change.call(new_config);
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => rsx! { div { class: "text-[10px] italic text-slate-400", "No quick config" } },
    }
}

fn signal_config(icon: &str, config: &Value, on_change: EventHandler<Value>) -> Element {
    match icon {
        "target" => text_field(
            "Promise Name",
            "promise_name",
            config,
            "payment-completed",
            on_change,
        ),
        "radio" => text_field(
            "Awakeable ID",
            "awakeable_id",
            config,
            "payment-callback",
            on_change,
        ),
        "check-circle" => text_field(
            "Promise Name",
            "promise_name",
            config,
            "payment-completed",
            on_change,
        ),
        "bell" => text_field(
            "Signal Name",
            "signal_name",
            config,
            "payment_signal",
            on_change,
        ),
        _ => rsx! { div { class: "text-[10px] italic text-slate-400", "No quick config" } },
    }
}

fn text_field(
    label: &str,
    key: &str,
    config: &Value,
    placeholder: &str,
    on_change: EventHandler<Value>,
) -> Element {
    let key_owned = key.to_string();
    let config_clone = config.clone();
    let value = get_str_val(config, key);
    rsx! {
        div { class: "flex flex-col gap-0.5",
            label { class: "text-[9px] font-medium uppercase tracking-wide text-slate-500", "{label}" }
            input {
                class: "{INPUT_CLASS}",
                placeholder: "{placeholder}",
                value: "{value}",
                oninput: move |e| {
                    let mut new_config = config_clone.clone();
                    if let Some(obj) = new_config.as_object_mut() {
                        obj.insert(key_owned.clone(), Value::String(e.value()));
                        on_change.call(new_config);
                    }
                }
            }
        }
    }
}
