//! Internal helper functions for octree operations.

use crate::structures::octree::{Bounds, Vec3};

#[inline]
#[allow(dead_code)]
pub(crate) fn bounds_center(b: &Bounds) -> Vec3 {
    Vec3::new(
        (b.min.x + b.max.x) / 2.0,
        (b.min.y + b.max.y) / 2.0,
        (b.min.z + b.max.z) / 2.0,
    )
}

#[inline]
#[allow(dead_code)]
pub(crate) fn bounds_extent(b: &Bounds) -> Vec3 {
    Vec3::new(b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)
}

#[inline]
#[allow(dead_code)]
pub(crate) fn child_index(parent: &Bounds, point: Vec3) -> usize {
    let center = bounds_center(parent);
    usize::from(point.x >= center.x)
        | (usize::from(point.y >= center.y) << 1)
        | (usize::from(point.z >= center.z) << 2)
}
