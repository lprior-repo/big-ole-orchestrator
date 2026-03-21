use dioxus::prelude::*;
use web_sys::window;

/// Menu dimensions (width: 224px = w-56, estimated height ~180px)
const MENU_WIDTH: f32 = 224.0;
const MENU_HEIGHT: f32 = 180.0;
const PADDING: f32 = 8.0;

/// Clamps position to keep menu within viewport bounds.
/// Returns the clamped (x, y) coordinates.
#[inline]
pub fn clamp_menu_position(
    x: f32,
    y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> (f32, f32) {
    let max_x = viewport_width - MENU_WIDTH - PADDING;
    let max_y = viewport_height - MENU_HEIGHT - PADDING;
    let clamped_x = x.max(PADDING).min(max_x);
    let clamped_y = y.max(PADDING).min(max_y);
    (clamped_x, clamped_y)
}

/// Generates viewport-safe CSS style string for menu positioning.
#[inline]
pub fn generate_menu_style(
    x: f32,
    y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> String {
    let (clamped_x, clamped_y) = clamp_menu_position(x, y, viewport_width, viewport_height);
    format!("left: {}px; top: {}px;", clamped_x, clamped_y)
}

#[component]
pub fn CanvasContextMenu(
    open: ReadSignal<bool>,
    x: ReadSignal<f32>,
    y: ReadSignal<f32>,
    on_close: EventHandler<MouseEvent>,
    on_add_node: EventHandler<MouseEvent>,
    on_fit_view: EventHandler<MouseEvent>,
    on_layout: EventHandler<MouseEvent>,
) -> Element {
    if !open() {
        return rsx! {};
    }

    // Use window dimensions for viewport-safe clamping
    let viewport_width = window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1920.0) as f32;
    let viewport_height = window()
        .and_then(|w| w.inner_height().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1080.0) as f32;
    let menu_style = generate_menu_style(*x.read(), *y.read(), viewport_width, viewport_height);

    rsx! {
        div {
            class: "fixed inset-0 z-50",

            button {
                r#type: "button",
                class: "absolute inset-0 h-full w-full cursor-default bg-transparent",
                aria_label: "Close context menu",
                onclick: move |evt| on_close.call(evt),
            }

            div {
                class: "absolute w-56 overflow-hidden rounded-lg border border-slate-700/80 bg-slate-900/95 shadow-2xl shadow-slate-950/70 ring-1 ring-slate-700/70 backdrop-blur",
                style: "{menu_style}",

                button {
                    r#type: "button",
                    class: "block w-full px-3 py-2 text-left text-sm font-medium text-slate-200 transition-colors hover:bg-slate-800/90 hover:text-slate-50",
                    onclick: move |evt| on_add_node.call(evt),
                    "Add Node"
                }

                button {
                    r#type: "button",
                    class: "block w-full px-3 py-2 text-left text-sm font-medium text-slate-200 transition-colors hover:bg-slate-800/90 hover:text-slate-50",
                    onclick: move |evt| on_fit_view.call(evt),
                    "Fit View"
                }

                button {
                    r#type: "button",
                    class: "block w-full px-3 py-2 text-left text-sm font-medium text-slate-200 transition-colors hover:bg-slate-800/90 hover:text-slate-50",
                    onclick: move |evt| on_layout.call(evt),
                    "Auto Layout"
                }

                div {
                    class: "border-t border-slate-700 px-3 py-2 text-xs text-slate-400",
                    "Hint: Press Esc or click outside to close"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_menu_position, generate_menu_style};

    const VIEWPORT_W: f32 = 1280.0;
    const VIEWPORT_H: f32 = 720.0;
    const MENU_W: f32 = 224.0;
    const MENU_H: f32 = 180.0;
    const PADDING: f32 = 8.0;

    #[test]
    fn given_position_within_bounds_when_clamping_then_position_unchanged() {
        let (x, y) = clamp_menu_position(100.0, 100.0, VIEWPORT_W, VIEWPORT_H);
        assert_eq!(x, 100.0);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn given_x_exceeds_right_edge_when_clamping_then_x_is_capped() {
        let (x, y) = clamp_menu_position(2000.0, 100.0, VIEWPORT_W, VIEWPORT_H);
        // Should be clamped to viewport_width - menu_width - padding
        let expected_x = VIEWPORT_W - MENU_W - PADDING;
        assert!((x - expected_x).abs() < 0.001);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn given_y_exceeds_bottom_edge_when_clamping_then_y_is_capped() {
        let (x, y) = clamp_menu_position(100.0, 1000.0, VIEWPORT_W, VIEWPORT_H);
        assert_eq!(x, 100.0);
        // Should be clamped to viewport_height - menu_height - padding
        let expected_y = VIEWPORT_H - MENU_H - PADDING;
        assert!((y - expected_y).abs() < 0.001);
    }

    #[test]
    fn given_negative_coordinates_when_clamping_then_min_padding_applied() {
        let (x, y) = clamp_menu_position(-50.0, -50.0, VIEWPORT_W, VIEWPORT_H);
        assert_eq!(x, PADDING);
        assert_eq!(y, PADDING);
    }

    #[test]
    fn given_exact_edge_position_when_clamping_then_position_unchanged() {
        let max_x = VIEWPORT_W - MENU_W - PADDING;
        let max_y = VIEWPORT_H - MENU_H - PADDING;
        let (x, y) = clamp_menu_position(max_x, max_y, VIEWPORT_W, VIEWPORT_H);
        assert!((x - max_x).abs() < 0.001);
        assert!((y - max_y).abs() < 0.001);
    }

    #[test]
    fn when_generating_style_then_contains_clamped_coordinates() {
        let style = generate_menu_style(100.0, 100.0, VIEWPORT_W, VIEWPORT_H);
        assert!(style.contains("left: 100px;"));
        assert!(style.contains("top: 100px;"));
    }

    #[test]
    fn when_generating_style_for_off_screen_position_then_clamps_coordinates() {
        let style = generate_menu_style(2000.0, 1000.0, VIEWPORT_W, VIEWPORT_H);
        let max_x = VIEWPORT_W - MENU_W - PADDING;
        let max_y = VIEWPORT_H - MENU_H - PADDING;
        assert!(style.contains(&format!("left: {:.0}px;", max_x)));
        assert!(style.contains(&format!("top: {:.0}px;", max_y)));
    }
}
