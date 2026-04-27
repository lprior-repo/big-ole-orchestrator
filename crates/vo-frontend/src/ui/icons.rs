//! Minimal icon stubs for vo-frontend UI components.

/// Returns an icon SVG string for the given icon name.
///
/// This is a minimal implementation that returns a generic icon SVG.
/// In a full implementation, this would return specific SVG markup
/// for each named icon.
pub fn icon_by_name(name: &str, size_class: String) -> String {
    let _ = size_class;
    format!("<svg class=\"{size_class}\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"currentColor\"/></svg>")
}
