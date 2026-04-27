//! Mirror type detection rule for API test quality gates.

pub mod detector;
pub mod rule;

pub use detector::check_mirror_types_in_tests;
pub use rule::MirrorRule;