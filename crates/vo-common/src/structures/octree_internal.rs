//! Internal helper functions for octree operations.
//!
//! These helpers are `pub(crate)` for use within vo-common only.

use crate::structures::octree::{Bounds, Vec3};

/// Compute the center point of a bounds.
#[inline]
pub(crate) fn bounds_center(b: &Bounds) -> Vec3 {
    Vec3::new(
        (b.min.x + b.max.x) / 2.0,
        (b.min.y + b.max.y) / 2.0,
        (b.min.z + b.max.z) / 2.0,
    )
}

/// Compute the extent (size) of a bounds.
#[inline]
pub(crate) fn bounds_extent(b: &Bounds) -> Vec3 {
    Vec3::new(
        b.max.x - b.min.x,
        b.max.y - b.min.y,
        b.max.z - b.min.z,
    )
}

/// Compute the octree child index (0-7) for a point within a bounds.
#[inline]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_center_calculation() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, 5.0);
        assert_eq!(c.y, 5.0);
        assert_eq!(c.z, 5.0);
    }

    #[test]
    fn bounds_center_negative_range() {
        let b = Bounds::new(Vec3::new(-10.0, -10.0, -10.0), Vec3::new(0.0, 0.0, 0.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, -5.0);
        assert_eq!(c.y, -5.0);
        assert_eq!(c.z, -5.0);
    }

    #[test]
    fn bounds_center_asymmetric() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 5.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, 10.0);
        assert_eq!(c.y, 5.0);
        assert_eq!(c.z, 2.5);
    }

    #[test]
    fn bounds_extent_calculation() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 20.0, 30.0));
        let e = bounds_extent(&b);
        assert_eq!(e.x, 10.0);
        assert_eq!(e.y, 20.0);
        assert_eq!(e.z, 30.0);
    }

    #[test]
    fn bounds_extent_negative_range() {
        let b = Bounds::new(Vec3::new(-10.0, -20.0, -30.0), Vec3::new(0.0, 0.0, 0.0));
        let e = bounds_extent(&b);
        assert_eq!(e.x, 10.0);
        assert_eq!(e.y, 20.0);
        assert_eq!(e.z, 30.0);
    }

    #[test]
    fn child_index_all_corners() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let idx_000 = child_index(&b, Vec3::new(0.0, 0.0, 0.0));
        let idx_111 = child_index(&b, Vec3::new(9.0, 9.0, 9.0));
        let idx_100 = child_index(&b, Vec3::new(9.0, 0.0, 0.0));
        let idx_010 = child_index(&b, Vec3::new(0.0, 9.0, 0.0));
        let idx_001 = child_index(&b, Vec3::new(0.0, 0.0, 9.0));
        let idx_110 = child_index(&b, Vec3::new(9.0, 9.0, 0.0));
        let idx_101 = child_index(&b, Vec3::new(9.0, 0.0, 9.0));
        let idx_011 = child_index(&b, Vec3::new(0.0, 9.0, 9.0));
        assert_eq!(idx_000, 0);
        assert_eq!(idx_111, 7);
        assert_eq!(idx_100, 1);
        assert_eq!(idx_010, 2);
        assert_eq!(idx_001, 4);
        assert_eq!(idx_110, 3);
        assert_eq!(idx_101, 5);
        assert_eq!(idx_011, 6);
    }

    #[test]
    fn child_index_point_on_center() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        let idx = child_index(&b, c);
        assert!(idx < 8);
    }

    #[test]
    fn bounds_center_debug() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        let debug_str = format!("{:?}", c);
        assert!(debug_str.contains("5"));
    }
}
