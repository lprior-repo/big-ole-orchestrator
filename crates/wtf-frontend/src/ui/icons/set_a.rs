use dioxus::prelude::*;

#[component]
pub fn WebhookIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "M18 16.98h-5.99c-1.1 0-1.95.94-2.48 1.9A4 4 0 0 1 2 17c.01-.7.2-1.4.57-2" }
            path { d: "m6 17 3.13-5.78c.53-.97.1-2.18-.5-3.1a4 4 0 1 1 6.89-4.06" }
            path { d: "m12 6 3.13 5.73C15.66 12.7 16.9 13 18 13a4 4 0 0 1 0 8H16" }
        }
    }
}

#[component]
pub fn ClockIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            circle { cx: "12", cy: "12", r: "10" }
            polyline { points: "12 6 12 12 16 14" }
        }
    }
}

#[component]
pub fn MailIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
            path { d: "m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" }
        }
    }
}

#[component]
pub fn GlobeIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" }
            path { d: "M2 12h20" }
        }
    }
}

#[component]
pub fn DatabaseIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            ellipse { cx: "12", cy: "5", rx: "9", ry: "3" }
            path { d: "M3 5V19A9 3 0 0 0 21 19V5" }
            path { d: "M3 12A9 3 0 0 0 21 12" }
        }
    }
}

#[component]
pub fn ShuffleIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "M2 18h1.4c1.3 0 2.5-.6 3.3-1.7l6.1-8.6c.7-1.1 2-1.7 3.3-1.7H22" }
            path { d: "m18 2 4 4-4 4" }
            path { d: "M2 6h1.9c1.5 0 2.9.9 3.6 2.2" }
            path { d: "M22 18h-5.9c-1.3 0-2.6-.7-3.3-1.8l-.5-.8" }
            path { d: "m18 14 4 4-4 4" }
        }
    }
}

#[component]
pub fn CodeIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            polyline { points: "16 18 22 12 16 6" }
            polyline { points: "8 6 2 12 8 18" }
        }
    }
}

#[component]
pub fn SparklesIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" }
            path { d: "M20 3v4" }
            path { d: "M22 5h-4" }
        }
    }
}

#[component]
pub fn GitBranchIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            line { x1: "6", x2: "6", y1: "3", y2: "15" }
            circle { cx: "18", cy: "6", r: "3" }
            circle { cx: "6", cy: "18", r: "3" }
            path { d: "M18 9a9 9 0 0 1-9 9" }
        }
    }
}

#[component]
pub fn GitForkIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            circle { cx: "12", cy: "18", r: "3" }
            circle { cx: "6", cy: "6", r: "3" }
            circle { cx: "18", cy: "6", r: "3" }
            path { d: "M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9" }
            path { d: "M12 12v3" }
        }
    }
}

#[component]
pub fn RepeatIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "m17 2 4 4-4 4" }
            path { d: "M3 11v-1a4 4 0 0 1 4-4h14" }
            path { d: "m7 22-4-4 4-4" }
            path { d: "M21 13v1a4 4 0 0 1-4 4H3" }
        }
    }
}

#[component]
pub fn MergeIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "m8 6 4-4 4 4" }
            path { d: "M12 2v10.3a4 4 0 0 1-1.172 2.872L4 22" }
            path { d: "m20 22-5-5" }
        }
    }
}

#[component]
pub fn MessageSquareIcon(class: String) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{class}",
            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
        }
    }
}
