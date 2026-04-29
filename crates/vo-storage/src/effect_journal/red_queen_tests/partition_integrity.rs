//! Red Queen tests — partition constant integrity.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::super::EFFECTS_PARTITION;

#[test]
fn red_queen_effects_partition_is_nonempty_utf8() {
    assert!(
        !EFFECTS_PARTITION.is_empty(),
        "BUG: EFFECTS_PARTITION is empty"
    );
    assert!(
        EFFECTS_PARTITION.chars().all(|c| !c.is_control()),
        "BUG: EFFECTS_PARTITION contains control characters"
    );
}

#[test]
fn red_queen_effects_partition_no_leading_trailing_whitespace() {
    assert_eq!(
        EFFECTS_PARTITION,
        EFFECTS_PARTITION.trim(),
        "BUG: EFFECTS_PARTITION has leading/trailing whitespace"
    );
}
