//! Internal helper functions for octree operations.

use crate::structures::octree::{Bounds, Vec3};

#[inline]
pub fn bounds_center(b: &Bounds) -> Vec3 {
    Vec3::new(
        (b.min.x + b.max.x) / 2.0,
        (b.min.y + b.max.y) / 2.0,
        (b.min.z + b.max.z) / 2.0,
    )
}

#[inline]
pub fn bounds_extent(b: &Bounds) -> Vec3 {
    Vec3::new(b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)
}

#[inline]
pub fn child_index(parent: &Bounds, point: Vec3) -> usize {
    let center = bounds_center(parent);
    usize::from(point.x >= center.x)
        | (usize::from(point.y >= center.y) << 1)
        | (usize::from(point.z >= center.z) << 2)
}
