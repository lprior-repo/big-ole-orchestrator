#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

use super::{get_str_val, get_u64_val};
use dioxus::prelude::*;
use serde_json::Value;

#[component]
pub(super) fn CommonConfig(
    config: Value,
    update_u64: EventHandler<(String, u64)>,
    update_str: EventHandler<(String, String)>,
    input_cls: &'static str,
) -> Element {
    let max_retries = get_u64_val(&config, "maxRetries").map_or(3, |value| value);
    let backoff_ms = get_u64_val(&config, "backoffMs").map_or(1000, |value| value);
    let idempotency_key = get_str_val(&config, "idempotencyKey");

    rsx! {
        div { class: "flex flex-col gap-3",
            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Retry Policy" }
            div { class: "grid grid-cols-2 gap-2",
                div { class: "flex flex-col gap-1",
                    span { class: "text-[10px] text-slate-500", "Max Retries" }
                    input {
                        r#type: "number",
                        class: "{input_cls}",
                        value: "{max_retries}",
                        oninput: move |e| {
                            if let Ok(val) = e.value().parse::<u64>() {
                                update_u64.call(("maxRetries".to_string(), val));
                            }
                        }
                    }
                }
                div { class: "flex flex-col gap-1",
                    span { class: "text-[10px] text-slate-500", "Backoff (ms)" }
                    input {
                        r#type: "number",
                        class: "{input_cls}",
                        value: "{backoff_ms}",
                        oninput: move |e| {
                            if let Ok(val) = e.value().parse::<u64>() {
                                update_u64.call(("backoffMs".to_string(), val));
                            }
                        }
                    }
                }
            }
        }

        div { class: "flex flex-col gap-1.5",
            label { class: "text-[11px] font-medium uppercase tracking-wide text-slate-500", "Idempotency Key" }
            input {
                class: "{input_cls}",
                placeholder: "ctx.rand.uuidv4()",
                value: "{idempotency_key}",
                oninput: move |e| update_str.call(("idempotencyKey".to_string(), e.value()))
            }
            span { class: "text-[10px] text-slate-500", "Auto-generated if blank. Ensures exactly-once execution." }
        }
    }
}
