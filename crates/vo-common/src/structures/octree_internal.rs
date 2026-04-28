//! Internal helper functions for octree operations.
//!
//! These helpers are `pub(crate)` for use within vo-common only.

use crate::structures::octree::{Bounds, Vec3};

/// Compute the center point of a bounds.
#[inline]
#[allow(dead_code)]
pub(crate) fn bounds_center(b: &Bounds) -> Vec3 {
    Vec3::new(
        (b.min.x + b.max.x) / 2.0,
        (b.min.y + b.max.y) / 2.0,
        (b.min.z + b.max.z) / 2.0,
    )
}

/// Compute the extent (size) of a bounds.
#[inline]
#[allow(dead_code)]
pub(crate) fn bounds_extent(b: &Bounds) -> Vec3 {
    Vec3::new(b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)
}

/// Compute the octree child index (0-7) for a point within a bounds.
#[inline]
#[allow(dead_code)]
pub(crate) fn child_index(parent: &Bounds, point: Vec3) -> usize {
    let center = bounds_center(parent);
    let mut idx = 0usize;
    if point.x >= center.x {
        idx |= 1;
    }
    if point.y >= center.y {
        idx |= 2;
    }
    if point.z >= center.z {
        idx |= 4;
    }
    idx
}
