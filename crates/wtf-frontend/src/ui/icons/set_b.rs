use dioxus::prelude::*;

#[component]
pub fn SendIcon(class: String) -> Element {
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
            path { d: "M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z" }
            path { d: "m21.854 2.147-10.94 10.939" }
        }
    }
}

#[component]
pub fn FileOutputIcon(class: String) -> Element {
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
            path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
            path { d: "M4 7V4a2 2 0 0 1 2-2 2 2 0 0 0-2 2" }
            path { d: "M4.063 20.999a2 2 0 0 0 2 1L18 22a2 2 0 0 0 2-2V7l-5-5H6" }
            path { d: "m5 11-3 3" }
            path { d: "m5 17-3-3h10" }
        }
    }
}

#[component]
pub fn CheckCircleIcon(class: String) -> Element {
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
            path { d: "M22 11.08V12a10 10 0 1 1-5.93-9.14" }
            path { d: "m9 11 3 3L22 4" }
        }
    }
}

#[component]
pub fn AlertCircleIcon(class: String) -> Element {
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
            line { x1: "12", x2: "12", y1: "8", y2: "12" }
            line { x1: "12", x2: "12.01", y1: "16", y2: "16" }
        }
    }
}

#[component]
pub fn LoaderIcon(class: String) -> Element {
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
            path { d: "M12 2v4" }
            path { d: "m16.2 7.8 2.9-2.9" }
            path { d: "M18 12h4" }
            path { d: "m16.2 16.2 2.9 2.9" }
            path { d: "M12 18v4" }
            path { d: "m4.9 19.1 2.9-2.9" }
            path { d: "M2 12h4" }
            path { d: "m4.9 4.9 2.9 2.9" }
        }
    }
}

#[component]
pub fn SearchIcon(class: String) -> Element {
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
            circle { cx: "11", cy: "11", r: "8" }
            path { d: "m21 21-4.3-4.3" }
        }
    }
}

#[component]
pub fn BoxIcon(class: String) -> Element {
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
            path { d: "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" }
            path { d: "m3.3 7 8.7 5 8.7-5" }
            path { d: "M12 22V12" }
        }
    }
}

#[component]
pub fn PlayIcon(class: String) -> Element {
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
            polygon { points: "6 3 20 12 6 21 6 3" }
        }
    }
}

#[component]
pub fn SaveIcon(class: String) -> Element {
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
            path { d: "M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" }
            path { d: "M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" }
            path { d: "M7 3v4a1 1 0 0 0 1 1h7" }
        }
    }
}

#[component]
pub fn UndoIcon(class: String) -> Element {
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
            path { d: "M3 7v6h6" }
            path { d: "M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13" }
        }
    }
}

#[component]
pub fn RedoIcon(class: String) -> Element {
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
            path { d: "M21 7v6h-6" }
            path { d: "M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3l3 2.7" }
        }
    }
}

#[component]
pub fn ZoomInIcon(class: String) -> Element {
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
            circle { cx: "11", cy: "11", r: "8" }
            line { x1: "21", x2: "16.65", y1: "21", y2: "16.65" }
            line { x1: "11", x2: "11", y1: "8", y2: "14" }
            line { x1: "8", x2: "14", y1: "11", y2: "11" }
        }
    }
}

#[component]
pub fn ZoomOutIcon(class: String) -> Element {
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
            circle { cx: "11", cy: "11", r: "8" }
            line { x1: "21", x2: "16.65", y1: "21", y2: "16.65" }
            line { x1: "8", x2: "14", y1: "11", y2: "11" }
        }
    }
}

#[component]
pub fn MaximizeIcon(class: String) -> Element {
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
            polyline { points: "15 3 21 3 21 9" }
            polyline { points: "9 21 3 21 3 15" }
            line { x1: "21", x2: "14", y1: "3", y2: "10" }
            line { x1: "3", x2: "10", y1: "21", y2: "14" }
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
pub fn CheckIcon(class: String) -> Element {
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
            polyline { points: "20 6 9 17 4 12" }
        }
    }
}

#[component]
pub fn XCircleIcon(class: String) -> Element {
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
            path { d: "m15 9-6 6" }
            path { d: "m9 9 6 6" }
        }
    }
}
