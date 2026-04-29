use dioxus::prelude::*;

#[component]
pub fn XIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M6 18L18 6M6 6l12 12",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn CopyIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M8 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2v-2M16 4h2a2 2 0 012 2v2m-4 0h4",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn CheckIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M5 13l4 4L19 7",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn TrashIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ChevronDownIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M19 9l-7 7-7-7",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ChevronRightIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M9 5l7 7-7 7",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ClockIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn LayersIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn XCircleIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn AlertCircleIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn AlertTriangleIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn SearchIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ShieldCheckIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ShieldAlertIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ShieldOffIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn RocketIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M15.59 14.37a6 6 0 01-5.84 7.38v-4.8m5.84-2.58a14.98 14.98 0 006.16-12.12A14.98 14.98 0 009.631 8.41m5.96 5.96a14.926 14.926 0 01-5.841 2.58m-.119-8.54a6 6 0 00-7.381 5.84h4.8m2.581-5.84a14.927 14.927 0 00-2.58 5.84m2.699 2.7c-.103.021-.207.041-.311.06a15.09 15.09 0 01-2.448-2.448 14.9 14.9 0 01.06-.312m-2.24 2.39a4.493 4.493 0 00-1.757 4.306 4.493 4.493 0 004.306-1.758M16.5 9a1.5 1.5 0 11-3 0 1.5 1.5 0 013 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn DatabaseIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn CogIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
            path {
                d: "M15 12a3 3 0 11-6 0 3 3 0 016 0z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn ZapIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M13 10V3L4 14h7v7l9-11h-7z",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

#[component]
pub fn WifiIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M8.111 16.404a5.5 5.5 0 017.778 0M12 20h.01m-7.08-7.071c3.904-3.905 10.236-3.905 14.141 0M1.394 9.393c5.857-5.857 15.355-5.857 21.213 0",
                stroke_linecap: "round",
                stroke_linejoin: "round"
            }
        }
    }
}

pub fn icon_by_name(name: &str, class: String) -> Element {
    match name {
        "x" => rsx! { XIcon { class } },
        "copy" => rsx! { CopyIcon { class } },
        "check" => rsx! { CheckIcon { class } },
        "trash" => rsx! { TrashIcon { class } },
        "chevron-down" => rsx! { ChevronDownIcon { class } },
        "chevron-right" => rsx! { ChevronRightIcon { class } },
        "clock" => rsx! { ClockIcon { class } },
        "layers" => rsx! { LayersIcon { class } },
        "x-circle" => rsx! { XCircleIcon { class } },
        "alert-circle" => rsx! { AlertCircleIcon { class } },
        "alert-triangle" => rsx! { AlertTriangleIcon { class } },
        "search" => rsx! { SearchIcon { class } },
        "shield-check" => rsx! { ShieldCheckIcon { class } },
        "shield-alert" => rsx! { ShieldAlertIcon { class } },
        "shield-off" => rsx! { ShieldOffIcon { class } },
        "rocket" => rsx! { RocketIcon { class } },
        "database" => rsx! { DatabaseIcon { class } },
        "cog" => rsx! { CogIcon { class } },
        "zap" => rsx! { ZapIcon { class } },
        "wifi" => rsx! { WifiIcon { class } },
        _ => rsx! { ZapIcon { class } },
    }
}