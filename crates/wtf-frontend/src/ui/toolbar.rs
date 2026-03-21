use crate::ui::icons::{
    LayersIcon, MaximizeIcon, PlayIcon, RedoIcon, SaveIcon, SettingsIcon, UndoIcon, ZoomInIcon,
    ZoomOutIcon,
};
use dioxus::prelude::*;

#[component]
fn ToolbarButton(
    label: &'static str,
    disabled: bool,
    on_click: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    let disabled_classes = if disabled {
        "opacity-40 pointer-events-none"
    } else {
        ""
    };

    rsx! {
        button {
            class: "flex h-9 w-9 items-center justify-center rounded-lg text-slate-600 transition-all duration-150 hover:-translate-y-px hover:bg-white hover:text-slate-900 hover:shadow-sm {disabled_classes}",
            title: "{label}",
            disabled,
            onclick: move |evt| on_click.call(evt),
            {children}
        }
    }
}

#[component]
pub fn FlowToolbar(
    workflow_name: ReadSignal<String>,
    on_workflow_name_change: EventHandler<String>,
    node_count: ReadSignal<usize>,
    edge_count: ReadSignal<usize>,
    zoom_label: ReadSignal<String>,
    on_zoom_in: EventHandler<MouseEvent>,
    on_zoom_out: EventHandler<MouseEvent>,
    on_fit_view: EventHandler<MouseEvent>,
    on_layout: EventHandler<MouseEvent>,
    on_execute: EventHandler<MouseEvent>,
    on_undo: EventHandler<MouseEvent>,
    on_redo: EventHandler<MouseEvent>,
    on_save: EventHandler<MouseEvent>,
    on_settings: EventHandler<MouseEvent>,
    can_undo: ReadSignal<bool>,
    can_redo: ReadSignal<bool>,
) -> Element {
    rsx! {
        header { class: "flex h-[68px] items-center justify-between gap-2 border-b border-slate-200/80 bg-gradient-to-r from-slate-50 via-white to-cyan-50/60 px-3 md:px-4 backdrop-blur",
            div { class: "flex min-w-0 items-center gap-2 md:gap-3",
                div { class: "flex items-center gap-2",
                    div { class: "flex h-9 w-9 items-center justify-center rounded-xl border border-cyan-200 bg-cyan-500/10 shadow-[0_0_0_4px_rgba(34,211,238,0.08)]",
                        PlayIcon { class: "ml-0.5 h-3.5 w-3.5 text-cyan-700" }
                    }
                    input {
                        r#type: "text",
                        value: "{workflow_name.read()}",
                        class: "h-8 w-auto min-w-[120px] max-w-[180px] border-none bg-transparent text-[14px] font-semibold text-slate-900 outline-none md:max-w-[320px] md:text-[15px]",
                        spellcheck: false,
                        oninput: move |evt| on_workflow_name_change.call(evt.value())
                    }
                }
                div { class: "hidden items-center gap-2 text-[11px] text-slate-500 lg:flex",
                    span { class: "rounded-full border border-cyan-200 bg-cyan-50 px-2 py-0.5 text-cyan-700", "Workflow" }
                    span { class: "rounded-full border border-slate-200 bg-white px-2 py-0.5 font-mono", "{node_count.read()} nodes" }
                    span { class: "rounded-full border border-slate-200 bg-white px-2 py-0.5 font-mono", "{edge_count.read()} links" }
                    span { class: "hidden rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 text-amber-700 md:inline-flex", "K to add node" }
                }
            }

            div { class: "hidden items-center gap-0.5 rounded-xl border border-slate-200/80 bg-white px-1 py-1 shadow-sm md:flex",
                ToolbarButton {
                    label: "Zoom Out",
                    disabled: false,
                    on_click: move |evt| on_zoom_out.call(evt),
                    ZoomOutIcon { class: "h-4 w-4" }
                }
                span { class: "min-w-[3rem] text-center font-mono text-[11px] text-slate-600", "{zoom_label.read()}" }
                ToolbarButton {
                    label: "Zoom In",
                    disabled: false,
                    on_click: move |evt| on_zoom_in.call(evt),
                    ZoomInIcon { class: "h-4 w-4" }
                }
                div { class: "mx-1 h-5 w-px bg-slate-300" }
                ToolbarButton {
                    label: "Fit View",
                    disabled: false,
                    on_click: move |evt| on_fit_view.call(evt),
                    MaximizeIcon { class: "h-4 w-4" }
                }
                div { class: "mx-1 h-5 w-px bg-slate-300" }
                ToolbarButton {
                    label: "Auto Layout",
                    disabled: false,
                    on_click: move |evt| on_layout.call(evt),
                    LayersIcon { class: "h-4 w-4" }
                }
            }

            // Mobile: visible buttons for fit view and auto layout (accessible without right-click)
            div { class: "flex items-center gap-0.5 md:hidden",
                ToolbarButton {
                    label: "Fit View",
                    disabled: false,
                    on_click: move |evt| on_fit_view.call(evt),
                    MaximizeIcon { class: "h-4 w-4" }
                }
                ToolbarButton {
                    label: "Auto Layout",
                    disabled: false,
                    on_click: move |evt| on_layout.call(evt),
                    LayersIcon { class: "h-4 w-4" }
                }
            }

            div { class: "flex items-center gap-0.5 md:gap-1",
                ToolbarButton {
                    label: "Undo",
                    disabled: !*can_undo.read(),
                    on_click: move |evt| on_undo.call(evt),
                    UndoIcon { class: "h-4 w-4" }
                }
                ToolbarButton {
                    label: "Redo",
                    disabled: !*can_redo.read(),
                    on_click: move |evt| on_redo.call(evt),
                    RedoIcon { class: "h-4 w-4" }
                }
                div { class: "mx-1 h-5 w-px bg-slate-300" }
                ToolbarButton {
                    label: "Save Workflow",
                    disabled: false,
                    on_click: move |evt| on_save.call(evt),
                    SaveIcon { class: "h-4 w-4" }
                }
                ToolbarButton {
                    label: "Settings",
                    disabled: false,
                    on_click: move |evt| on_settings.call(evt),
                    SettingsIcon { class: "h-4 w-4" }
                }
                button {
                    class: "ml-1 flex h-9 items-center gap-1.5 rounded-lg bg-gradient-to-r from-cyan-600 to-teal-600 px-3 text-[12px] font-semibold text-white transition-all duration-150 hover:-translate-y-px hover:from-cyan-500 hover:to-teal-500 hover:shadow-lg hover:shadow-cyan-500/30",
                    title: "Run this workflow",
                    onclick: move |evt| on_execute.call(evt),
                    PlayIcon { class: "h-3.5 w-3.5" }
                    "Execute"
                }
            }
        }
    }
}
