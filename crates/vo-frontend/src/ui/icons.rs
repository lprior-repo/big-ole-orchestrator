use dioxus::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct CopyIcon;

#[component]
pub fn IconByName(icon_name: String, class: String) -> Element {
    rsx! {
        span {
            class: "inline-flex items-center justify-center {class}",
            match icon_name.as_str() {
                "shield-check" | "shield-alert" | "shield-off" => rsx! {
                    svg {
                        "aria-hidden": "true",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke: "currentColor",
                        path {
                            d: "M9 12.75L11.25 15 15 9.75m-1.5-4.5a4.5 4.5 0 110 9 4.5 4.5 0 010-9z",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                },
                "check" => rsx! {
                    svg {
                        "aria-hidden": "true",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke: "currentColor",
                        path {
                            d: "M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                },
                "zap" | "database" | "cog" | "clock" | "wifi" | "rocket" => rsx! {
                    svg {
                        "aria-hidden": "true",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke: "currentColor",
                        path {
                            d: "M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                },
                _ => rsx! {
                    svg {
                        "aria-hidden": "true",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke: "currentColor",
                        path {
                            d: "M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9 5.25h.008v.008H12v-.008z",
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                        }
                    }
                },
            }
        }
    }
}

pub fn icon_by_name(icon_name: &str, class: String) -> Element {
    rsx! {
        IconByName {
            icon_name: icon_name.to_string(),
            class: class,
        }
    }
}