#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::ui::icons::{icon_by_name, BoxIcon, ChevronDownIcon, HelpCircleIcon, SearchIcon};
use dioxus::prelude::*;

use super::model::{no_results, visible_indices, Category, NodeTemplate, NODE_TEMPLATES};

// ── Calculations ──────────────────────────────────────────────────────────────

/// Chevron rotation class: rotated when collapsed.
const fn chevron_class(collapsed: bool) -> &'static str {
    if collapsed {
        "h-3 w-3 text-slate-400/50 transition-transform -rotate-90"
    } else {
        "h-3 w-3 text-slate-400/50 transition-transform"
    }
}

// ── Components (Action layer) ─────────────────────────────────────────────────

#[component]
pub fn NodeSidebar(
    search: ReadSignal<String>,
    on_search_change: EventHandler<String>,
    on_pickup_node: EventHandler<&'static str>,
    on_add_node: EventHandler<&'static str>,
) -> Element {
    // Lower-cased once here; passed into pure calc functions below.
    let query = search.read().to_lowercase();

    // Per-category collapsed state.  A `HashSet` of collapsed categories is
    // simpler than a `HashMap<_, bool>` and avoids the `unwrap_or` on lookup.
    // `mut` is required by Dioxus 0.7 Signal API for write access.
    let mut collapsed: Signal<std::collections::HashSet<Category>> =
        use_signal(std::collections::HashSet::new);

    rsx! {
        aside {
            class: "flex h-full w-[280px] shrink-0 flex-col border-r border-slate-200 \
                    bg-slate-50/95 backdrop-blur",

            // Header
            div {
                class: "flex items-center gap-2.5 border-b border-slate-200 px-4 py-3",
                div {
                    class: "flex h-6 w-6 items-center justify-center rounded-md bg-indigo-600/10",
                    BoxIcon { class: "h-3.5 w-3.5 text-indigo-600" }
                }
                div { class: "flex flex-col",
                    span {
                        class: "text-[13px] font-semibold leading-tight text-slate-900",
                        "Application Composer"
                    }
                    span {
                        class: "text-[10px] leading-tight text-slate-500",
                        "Drag nodes onto the canvas to build your workflow"
                    }
                }
            }

            // Search
            div { class: "px-3 py-2.5",
                div { class: "relative",
                    SearchIcon {
                        class: "pointer-events-none absolute left-2.5 top-1/2 \
                                h-3.5 w-3.5 -translate-y-1/2 text-slate-400"
                    }
                    input {
                        r#type: "text",
                        placeholder: "Search nodes...",
                        value: "{search.read()}",
                        class: "h-8 w-full rounded-md border border-slate-300 bg-white \
                                pl-8 pr-3 text-[12px] text-slate-800 placeholder:text-slate-400 \
                                outline-none transition-colors focus:border-indigo-500/60 \
                                focus:ring-1 focus:ring-indigo-500/20",
                        oninput: move |evt| on_search_change.call(evt.value()),
                    }
                }
            }

            // Category groups
            div { class: "flex-1 overflow-y-auto px-3 pb-4",
                for category in Category::ORDER {
                    {
                        // Pure calc: which templates are visible?
                        let indices = visible_indices(category, &query);

                        if indices.is_empty() {
                            rsx! {}
                        } else {
                            let count = indices.len();
                            let is_collapsed = collapsed.read().contains(&category);

                            rsx! {
                                div { class: "mb-3", key: "{category:?}",

                                    // Collapsible category header
                                    button {
                                        r#type: "button",
                                        class: "group/cat flex w-full items-center gap-2 px-1 py-2",
                                        onclick: move |_| {
                                            // Signal write — only mutation in this component.
                                            let _ = collapsed.try_write().map(|mut set| {
                                                if set.contains(&category) {
                                                    set.remove(&category);
                                                } else {
                                                    set.insert(category);
                                                }
                                            });
                                        },
                                        div {
                                            class: "h-1.5 w-1.5 shrink-0 rounded-full \
                                                    {category.dot_class()}"
                                        }
                                        span {
                                            class: "text-[11px] font-medium uppercase \
                                                    tracking-wider text-slate-500 \
                                                    transition-colors group-hover/cat:text-slate-800",
                                            "{category.label()}"
                                        }
                                        span {
                                            class: "ml-auto text-[10px] text-slate-400/60",
                                            "{count}"
                                        }
                                        div { class: "{chevron_class(is_collapsed)}",
                                            ChevronDownIcon { class: "h-3 w-3" }
                                        }
                                    }

                                    // Node buttons — hidden while collapsed
                                    if !is_collapsed {
                                        div { class: "flex flex-col gap-0.5",
                                            for idx in indices {
                                                NodeButton {
                                                    template: &NODE_TEMPLATES[idx],
                                                    on_pickup_node,
                                                    on_add_node,
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Empty-search fallback
                if no_results(&query) {
                    div {
                        class: "flex flex-col items-center justify-center py-12 text-center",
                        SearchIcon { class: "mb-3 h-8 w-8 text-slate-300" }
                        p { class: "text-[12px] text-slate-500", "No nodes found" }
                    }
                }
            }
        }
    }
}

// ── NodeButton sub-component ──────────────────────────────────────────────────

#[component]
fn NodeButton(
    template: &'static NodeTemplate,
    on_pickup_node: EventHandler<&'static str>,
    on_add_node: EventHandler<&'static str>,
) -> Element {
    // `mut` required by Dioxus 0.7 Signal API for write access in event handlers.
    let mut show_tooltip: Signal<bool> = use_signal(|| false);

    rsx! {
        div { class: "relative",
            button {
                r#type: "button",
                key: "{template.node_type}",
                class: "group flex w-full cursor-grab items-center gap-2.5 rounded-md \
                        px-2 py-2 text-left transition-all duration-100 \
                        hover:bg-slate-200/80 active:scale-[0.98] active:cursor-grabbing",
                onmousedown: move |_| on_pickup_node.call(template.node_type),
                onclick: move |_| on_add_node.call(template.node_type),
                onmouseenter: move |_| { let _ = show_tooltip.try_write().map(|mut v| *v = true); },
                onmouseleave: move |_| { let _ = show_tooltip.try_write().map(|mut v| *v = false); },

                div {
                    class: "flex h-7 w-7 shrink-0 items-center justify-center rounded-md \
                            border transition-colors {template.category.icon_badge_class()}",
                    { icon_by_name(template.icon, "h-3.5 w-3.5".to_string()) }
                }
                div { class: "flex min-w-0 flex-1 flex-col",
                    span {
                        class: "truncate text-[12px] font-medium leading-tight text-slate-900",
                        "{template.label}"
                    }
                    span {
                        class: "truncate text-[10px] leading-tight text-slate-500",
                        "{template.description}"
                    }
                }
                if template.doc_link.is_some() {
                    div {
                        class: "flex items-center gap-1 opacity-0 transition-opacity \
                                group-hover:opacity-100",
                        if let Some(doc_link) = template.doc_link {
                            a {
                                href: "/docs/10_RESTATE_SDK.md{doc_link}",
                                target: "_blank",
                                class: "text-slate-400 hover:text-indigo-600",
                                onclick: move |evt| evt.stop_propagation(),
                                HelpCircleIcon { class: "h-3.5 w-3.5" }
                            }
                        }
                    }
                }
            }

            // Tooltip — floats to the right of the sidebar
            if *show_tooltip.read() {
                div {
                    class: "absolute left-full top-0 z-50 ml-2 w-64 rounded-lg \
                            border border-slate-200 bg-white p-3 shadow-xl",
                    div { class: "flex items-start gap-2",
                        div {
                            class: "mt-0.5 flex h-5 w-5 shrink-0 items-center \
                                    justify-center rounded-md \
                                    {template.category.icon_badge_class()}",
                            { icon_by_name(template.icon, "h-3 w-3".to_string()) }
                        }
                        div { class: "flex-1",
                            h4 {
                                class: "text-[12px] font-semibold text-slate-900",
                                "{template.label}"
                            }
                            p {
                                class: "mt-1 text-[11px] leading-relaxed \
                                        text-slate-600 whitespace-pre-line",
                                "{template.tooltip}"
                            }
                        }
                    }
                    if let Some(doc_link) = template.doc_link {
                        div { class: "mt-2 border-t border-slate-200 pt-2",
                            a {
                                href: "/docs/10_RESTATE_SDK.md{doc_link}",
                                target: "_blank",
                                class: "flex items-center gap-1 text-[11px] \
                                        text-indigo-600 hover:underline",
                                "View documentation "
                                span { class: "text-indigo-500", "->" }
                            }
                        }
                    }
                }
            }
        }
    }
}
