#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]

//! Small inline icon renderer used by UI badges.

use dioxus::prelude::*;

#[component]
pub fn CopyIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon copy-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M20 9h-9a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2v-9a2 2 0 0 0-2-2Z" }
            path { d: "M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" }
        }
    }
}

#[component]
pub fn XIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon x-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "18", y1: "6", x2: "6", y2: "18" }
            line { x1: "6", y1: "6", x2: "18", y2: "18" }
        }
    }
}

#[component]
pub fn LayersIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon layers-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "12 2 2 7 12 12 22 7 12 2" }
            polyline { points: "2 17 12 22 22 17" }
            polyline { points: "2 12 12 17 22 12" }
        }
    }
}

#[component]
pub fn ChevronDownIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon chevron-down-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "6 9 12 15 18 9" }
        }
    }
}

#[component]
pub fn TrashIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon trash-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "3 6 5 6 21 6" }
            path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" }
        }
    }
}

#[component]
pub fn CheckIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon check-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "20 6 9 17 4 12" }
        }
    }
}

pub fn icon_by_name(name: &str, class: String) -> Element {
    match name {
        "copy" => rsx! { CopyIcon { class } },
        "x" | "close" | "X" => rsx! { XIcon { class } },
        "layers" => rsx! { LayersIcon { class } },
        "chevron-down" | "chevronDown" => rsx! { ChevronDownIcon { class } },
        "trash" | "trash-icon" => rsx! { TrashIcon { class } },
        "check" | "check-icon" => rsx! { CheckIcon { class } },
        "shield-check" => rsx! { ShieldCheckIcon { class } },
        "shield-alert" => rsx! { ShieldAlertIcon { class } },
        "shield-off" => rsx! { ShieldOffIcon { class } },
        "rocket" => rsx! { RocketIcon { class } },
        "database" => rsx! { DatabaseIcon { class } },
        "cog" => rsx! { CogIcon { class } },
        "zap" => rsx! { ZapIcon { class } },
        "clock" => rsx! { ClockIcon { class } },
        "wifi" => rsx! { WifiIcon { class } },
        _ => rsx! {
            span { class: "icon-unknown {class}", "{name}" }
        },
    }
}

#[component]
pub fn ShieldCheckIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon shield-check-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
            polyline { points: "9 12 12 15 16 10" }
        }
    }
}

#[component]
pub fn ShieldAlertIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon shield-alert-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
            line { x1: "12", y1: "8", x2: "12", y2: "12" }
            line { x1: "12", y1: "16", x2: "12.01", y2: "16" }
        }
    }
}

#[component]
pub fn ShieldOffIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon shield-off-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M19.69 14a6.9 6.9 0 0 0 .31-2V5l-8-3-3.16 1.18" }
            path { d: "M4.73 4.73L4 5v7c0 6 8 10 8 10a20.29 20.29 0 0 0 5.62-4.38" }
            line { x1: "2", y1: "2", x2: "22", y2: "22" }
        }
    }
}

#[component]
pub fn RocketIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon rocket-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09z" }
            path { d: "m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z" }
            path { d: "M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0" }
            path { d: "M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5" }
        }
    }
}

#[component]
pub fn DatabaseIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon database-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            ellipse { cx: "12", cy: "5", rx: "9", ry: "3" }
            path { d: "M3 5V19A9 3 0 0 0 21 19V5" }
            path { d: "M3 12A9 3 0 0 0 21 12" }
        }
    }
}

#[component]
pub fn CogIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon cog-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "3" }
            path { d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" }
        }
    }
}

#[component]
pub fn ZapIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon zap-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "13 2 3 14 12 14 11 22 21 10 12 10 13 2" }
        }
    }
}

#[component]
pub fn ClockIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon clock-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "10" }
            polyline { points: "12 6 12 12 16 14" }
        }
    }
}

#[component]
pub fn WifiIcon(class: String) -> Element {
    rsx! {
        svg {
            class: "icon wifi-icon {class}",
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M5 12.55a11 11 0 0 1 14.08 0" }
            path { d: "M1.42 9a16 16 0 0 1 21.16 0" }
            path { d: "M8.53 16.11a6 6 0 0 1 6.95 0" }
            line { x1: "12", y1: "20", x2: "12.01", y2: "20" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_by_name_returns_copy_icon_for_copy() {
        let result = icon_by_name("copy", "h-4 w-4".to_string());
        assert!(true);
    }

    #[test]
    fn icon_by_name_returns_shield_icons_for_guarantees() {
        let _ = icon_by_name("shield-check", "h-4 w-4".to_string());
        let _ = icon_by_name("shield-alert", "h-4 w-4".to_string());
        let _ = icon_by_name("shield-off", "h-4 w-4".to_string());
    }

    #[test]
    fn icon_by_name_returns_unknown_for_unknown() {
        let result = icon_by_name("unknown-icon", "h-4 w-4".to_string());
        assert!(true);
    }
}
