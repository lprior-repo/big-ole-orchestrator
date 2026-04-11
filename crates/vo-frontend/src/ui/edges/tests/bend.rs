use crate::ui::edges::layout::normalize_bend_delta;
use crate::ui::edges::types::Rect;

// ==================== Rect Tests ====================

#[test]
fn given_rect_when_created_then_has_correct_values() {
    let rect = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };

    assert_eq!(rect.x, 10.0);
    assert_eq!(rect.y, 20.0);
    assert_eq!(rect.width, 100.0);
    assert_eq!(rect.height, 50.0);
}

// ==================== Zoom-Normalized Bend Tests ====================

#[test]
fn given_valid_zoom_when_normalize_bend_then_returns_scaled_delta() {
    let page_delta = 100.0;
    let zoom = 2.0; // 200% zoom

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 50.0);
}

#[test]
fn given_zoom_of_one_when_normalize_bend_then_returns_same_delta() {
    let page_delta = 75.0;
    let zoom = 1.0;

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 75.0);
}

#[test]
fn given_invalid_zoom_zero_when_normalize_bend_then_returns_zero() {
    let page_delta = 100.0;
    let zoom = 0.0;

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 0.0);
}

#[test]
fn given_invalid_zoom_negative_when_normalize_bend_then_returns_zero() {
    let page_delta = 100.0;
    let zoom = -1.0;

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 0.0);
}

#[test]
fn given_invalid_zoom_nan_when_normalize_bend_then_returns_zero() {
    let page_delta = 100.0;
    let zoom = f32::NAN;

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 0.0);
}

#[test]
fn given_invalid_zoom_infinity_when_normalize_bend_then_returns_zero() {
    let page_delta = 100.0;
    let zoom = f32::INFINITY;

    let result = normalize_bend_delta(page_delta, zoom);

    assert_eq!(result, 0.0);
}
