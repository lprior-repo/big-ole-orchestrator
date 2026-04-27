//! Unit tests for vo-common utility types: Vec3, Bounds, TimestampMs methods.
//!
//! Fills coverage gaps not reached by existing integration/proptest suites:
//! - Vec3::new construction
//! - Bounds::new and Bounds::contains with boundary and interior cases
//! - TimestampMs::as_u64 and TimestampMs::new_unchecked

use vo_common::types::TimestampMs;
use vo_common::{Bounds, Vec3};

// ============================================================================
// Vec3 Tests
// ============================================================================

#[cfg(test)]
mod vec3_tests {
    use super::*;

    #[test]
    fn vec3_new_basic() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn vec3_new_zero() {
        let v = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn vec3_new_negative() {
        let v = Vec3::new(-1.0, -2.0, -3.0);
        assert_eq!(v.x, -1.0);
        assert_eq!(v.y, -2.0);
        assert_eq!(v.z, -3.0);
    }

    #[test]
    fn vec3_new_mixed_signs() {
        let v = Vec3::new(-1.5, 0.0, 2.5);
        assert_eq!(v.x, -1.5);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 2.5);
    }

    #[test]
    fn vec3_copy_equality() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(1.0, 2.0, 3.0);
        let c = Vec3::new(1.0, 2.0, 4.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn vec3_copy_cloned() {
        let v = Vec3::new(1.0, -2.0, 3.0);
        let cloned = v.clone();
        assert_eq!(v, cloned);
    }

    #[test]
    fn vec3_copy_debug() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let debug = format!("{v:?}");
        assert!(debug.contains("Vec3"));
        assert!(debug.contains("1.0"));
        assert!(debug.contains("2.0"));
        assert!(debug.contains("3.0"));
    }
}

// ============================================================================
// Bounds Tests
// ============================================================================

#[cfg(test)]
mod bounds_tests {
    use super::*;

    fn sample_bounds() -> Bounds {
        Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0))
    }

    #[test]
    fn bounds_new_basic() {
        let min = Vec3::new(0.0, 0.0, 0.0);
        let max = Vec3::new(10.0, 10.0, 10.0);
        let b = Bounds::new(min, max);
        assert_eq!(b.min, min);
        assert_eq!(b.max, max);
    }

    #[test]
    fn bounds_new_negative_range() {
        let b = Bounds::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(0.0, 0.0, 0.0));
        assert!(b.contains(Vec3::new(-2.5, -2.5, -2.5)));
    }

    #[test]
    fn bounds_contains_origin_in_positive_bounds() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn bounds_contains_interior_point() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(5.0, 5.0, 5.0)));
    }

    #[test]
    fn bounds_contains_corner_min() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn bounds_contains_corner_max() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(10.0, 10.0, 10.0)));
    }

    #[test]
    fn bounds_contains_face_center() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(5.0, 0.0, 5.0)));
        assert!(b.contains(Vec3::new(0.0, 5.0, 5.0)));
        assert!(b.contains(Vec3::new(5.0, 5.0, 0.0)));
    }

    #[test]
    fn bounds_contains_edge_center() {
        let b = sample_bounds();
        assert!(b.contains(Vec3::new(5.0, 0.0, 0.0)));
        assert!(b.contains(Vec3::new(0.0, 5.0, 0.0)));
        assert!(b.contains(Vec3::new(0.0, 0.0, 5.0)));
    }

    #[test]
    fn bounds_not_contains_outside_positive_x() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(10.0001, 5.0, 5.0)));
    }

    #[test]
    fn bounds_not_contains_outside_negative_x() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(-0.0001, 5.0, 5.0)));
    }

    #[test]
    fn bounds_not_contains_outside_positive_y() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(5.0, 10.0001, 5.0)));
    }

    #[test]
    fn bounds_not_contains_outside_negative_y() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(5.0, -0.0001, 5.0)));
    }

    #[test]
    fn bounds_not_contains_outside_positive_z() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(5.0, 5.0, 10.0001)));
    }

    #[test]
    fn bounds_not_contains_outside_negative_z() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(5.0, 5.0, -0.0001)));
    }

    #[test]
    fn bounds_not_contains_far_outside() {
        let b = sample_bounds();
        assert!(!b.contains(Vec3::new(100.0, 100.0, 100.0)));
        assert!(!b.contains(Vec3::new(-100.0, -100.0, -100.0)));
    }

    #[test]
    fn bounds_copy_cloned() {
        let b = sample_bounds();
        let cloned = b.clone();
        // Bounds does not derive PartialEq - verify clone by testing contains
        assert!(cloned.contains(Vec3::new(5.0, 5.0, 5.0)));
        assert!(cloned.contains(Vec3::new(0.0, 0.0, 0.0)));
        assert!(cloned.contains(Vec3::new(10.0, 10.0, 10.0)));
    }

    #[test]
    fn bounds_copy_debug() {
        let b = sample_bounds();
        let debug = format!("{b:?}");
        assert!(debug.contains("Bounds"));
    }

    #[test]
    fn bounds_point_at_diagonal_corner() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
        assert!(b.contains(Vec3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn bounds_symmetric_range_query() {
        let b = Bounds::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0));
        assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
        assert!(b.contains(Vec3::new(-5.0, 0.0, 0.0)));
        assert!(b.contains(Vec3::new(5.0, 0.0, 0.0)));
    }
}

// ============================================================================
// TimestampMs Tests
// ============================================================================

#[cfg(test)]
mod timestamp_ms_tests {
    use super::*;

    #[test]
    fn timestamp_ms_as_u64_returns_value() {
        let ts = TimestampMs::new_unchecked(42);
        assert_eq!(ts.as_u64(), 42);
    }

    #[test]
    fn timestamp_ms_as_u64_max() {
        let ts = TimestampMs::new_unchecked(u64::MAX);
        assert_eq!(ts.as_u64(), u64::MAX);
    }

    #[test]
    fn timestamp_ms_as_u64_zero() {
        let ts = TimestampMs::new_unchecked(0);
        assert_eq!(ts.as_u64(), 0);
    }

    #[test]
    fn timestamp_ms_as_u64_large() {
        let ts = TimestampMs::new_unchecked(1_700_000_000_000);
        assert_eq!(ts.as_u64(), 1_700_000_000_000);
    }

    #[test]
    fn timestamp_ms_now_returns_current_time() {
        let ts = TimestampMs::now();
        let now_ms = ts.as_u64();
        // Should be a reasonable Unix timestamp in millis (after year 2000)
        assert!(now_ms > 946_684_800_000);
        // Should not be u64::MAX (fallback for clock before epoch)
        assert_ne!(now_ms, u64::MAX);
    }

    #[test]
    fn timestamp_ms_new_unchecked_preserves_value() {
        for val in [0u64, 1, 100, 1_000, 1_000_000, u64::MAX] {
            let ts = TimestampMs::new_unchecked(val);
            assert_eq!(ts.as_u64(), val);
        }
    }

    #[test]
    fn timestamp_ms_copy_equality() {
        let a = TimestampMs::new_unchecked(100);
        let b = TimestampMs::new_unchecked(100);
        assert_eq!(a, b);
    }

    #[test]
    fn timestamp_ms_copy_inequality() {
        let a = TimestampMs::new_unchecked(100);
        let b = TimestampMs::new_unchecked(200);
        assert_ne!(a, b);
    }

    #[test]
    fn timestamp_ms_copy_ordering() {
        let a = TimestampMs::new_unchecked(100);
        let b = TimestampMs::new_unchecked(200);
        let c = TimestampMs::new_unchecked(200);
        assert!(a < b);
        assert!(b == c);
        assert!(b >= a);
    }

    #[test]
    fn timestamp_ms_copy_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TimestampMs::new_unchecked(42));
        set.insert(TimestampMs::new_unchecked(42));
        set.insert(TimestampMs::new_unchecked(99));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn timestamp_ms_copy_serialization_roundtrip() {
        let ts = TimestampMs::new_unchecked(1234567890);
        let json = serde_json::to_string(&ts).expect("serialize");
        let back: TimestampMs = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ts, back);
    }

    #[test]
    fn timestamp_ms_copy_clone() {
        let ts = TimestampMs::new_unchecked(999);
        let cloned = ts.clone();
        assert_eq!(ts, cloned);
    }

    #[test]
    fn timestamp_ms_copy_debug() {
        let ts = TimestampMs::new_unchecked(42);
        let debug = format!("{ts:?}");
        assert!(debug.contains("TimestampMs"));
    }
}
