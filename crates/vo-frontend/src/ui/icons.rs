use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct IconProps {
    pub class: String,
}

impl Default for IconProps {
    fn default() -> Self {
        Self {
            class: "h-5 w-5".to_string(),
        }
    }
}

#[component]
pub fn XIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M6 18L18 6M6 6l12 12"
            }
        }
    }
}

#[component]
pub fn CheckIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M5 13l4 4L19 7"
            }
        }
    }
}

#[component]
pub fn ClockIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
            }
        }
    }
}

#[component]
pub fn ChevronDownIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M19 9l-7 7-7-7"
            }
        }
    }
}

#[component]
pub fn CopyIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
            }
        }
    }
}

#[component]
pub fn SearchIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            }
        }
    }
}

#[component]
pub fn XCircleIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
            }
        }
    }
}

#[component]
pub fn LayersIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2M5 11V5a2 2 0 012-2m14 0V5a2 2 0 00-2-2M5 11v6a2 2 0 002 2h14a2 2 0 002-2v-6"
            }
        }
    }
}

#[component]
pub fn AlertCircleIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            }
        }
    }
}

#[component]
pub fn AlertTriangleIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
            }
        }
    }
}

#[component]
pub fn ChevronRightIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M9 5l7 7-7 7"
            }
        }
    }
}

#[component]
pub fn TrashIcon(props: IconProps) -> Element {
    rsx! {
        svg {
            class: "{props.class}",
            fill: "none",
            view_box: "0 0 24 24",
            stroke: "currentColor",
            stroke_width: "2",
            path {
                d: "M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
            }
        }
    }
}

pub fn icon_by_name(name: &str, class: String) -> Element {
    match name {
        "x" => rsx! { XIcon { class: class } },
        "check" | "check-circle" => rsx! { CheckIcon { class: class } },
        "clock" | "time" => rsx! { ClockIcon { class: class } },
        "chevron-down" => rsx! { ChevronDownIcon { class: class } },
        "copy" => rsx! { CopyIcon { class: class } },
        "search" => rsx! { SearchIcon { class: class } },
        "x-circle" | "x-circle-icon" => rsx! { XCircleIcon { class: class } },
        "layers" => rsx! { LayersIcon { class: class } },
        "alert-circle" | "alert-circle-icon" => rsx! { AlertCircleIcon { class: class } },
        "alert-triangle" | "alert-triangle-icon" => rsx! { AlertTriangleIcon { class: class } },
        "chevron-right" => rsx! { ChevronRightIcon { class: class } },
        "trash" => rsx! { TrashIcon { class: class } },
        _ => rsx! { XIcon { class: class } },
    }
}
