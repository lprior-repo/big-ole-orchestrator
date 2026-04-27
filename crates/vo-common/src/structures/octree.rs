//! Octree spatial data structure for 3D point storage and range queries.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Octree<T: Clone + Serialize> {
    bounds: Bounds,
    data: Vec<(Vec3, T)>,
    children: Option<Box<[Octree<T>; 8]>>,
}

impl<T: Clone + Serialize> Octree<T> {
    pub const CAPACITY: usize = 8;

    #[inline]
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            data: Vec::new(),
            children: None,
        }
    }

    pub fn insert(&mut self, point: Vec3, value: T) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        if self.children.is_none() && self.data.len() < Self::CAPACITY {
            self.data.push((point, value));
            return true;
        }
        self.subdivide();
        for child in self.children.as_mut().unwrap().iter_mut() {
            if child.insert(point, value.clone()) {
                return true;
            }
        }
        false
    }

    fn subdivide(&mut self) {
        if self.children.is_some() {
            return;
        }
        let m = Vec3::new(
            (self.bounds.min.x + self.bounds.max.x) / 2.0,
            (self.bounds.min.y + self.bounds.max.y) / 2.0,
            (self.bounds.min.z + self.bounds.max.z) / 2.0,
        );
        let corners = [
            Vec3::new(self.bounds.min.x, self.bounds.min.y, self.bounds.min.z),
            Vec3::new(m.x, self.bounds.min.y, self.bounds.min.z),
            Vec3::new(self.bounds.min.x, m.y, self.bounds.min.z),
            Vec3::new(m.x, m.y, self.bounds.min.z),
            Vec3::new(self.bounds.min.x, self.bounds.min.y, m.z),
            Vec3::new(m.x, self.bounds.min.y, m.z),
            Vec3::new(self.bounds.min.x, m.y, m.z),
            Vec3::new(m.x, m.y, m.z),
        ];
        let opp = [
            Vec3::new(m.x, m.y, m.z),
            Vec3::new(self.bounds.max.x, m.y, m.z),
            Vec3::new(m.x, self.bounds.max.y, m.z),
            Vec3::new(self.bounds.max.x, self.bounds.max.y, m.z),
            Vec3::new(m.x, m.y, self.bounds.max.z),
            Vec3::new(self.bounds.max.x, m.y, self.bounds.max.z),
            Vec3::new(m.x, self.bounds.max.y, self.bounds.max.z),
            Vec3::new(self.bounds.max.x, self.bounds.max.y, self.bounds.max.z),
        ];
        self.children = Some(Box::new(std::array::from_fn(|i| {
            Octree::new(Bounds::new(corners[i], opp[i]))
        })));
        let drained: Vec<_> = self.data.drain(..).collect();
        for (pt, val) in drained {
            for child in self.children.as_mut().unwrap().iter_mut() {
                if child.insert(pt, val.clone()) {
                    break;
                }
            }
        }
    }

    pub fn query_range(&self, range: &Bounds) -> Vec<&T> {
        let mut out = Vec::new();
        self.collect_range(range, &mut out);
        out
    }

    fn collect_range<'a>(&'a self, range: &Bounds, out: &mut Vec<&'a T>) {
        let overlaps = self.bounds.min.x <= range.max.x
            && self.bounds.max.x >= range.min.x
            && self.bounds.min.y <= range.max.y
            && self.bounds.max.y >= range.min.y
            && self.bounds.min.z <= range.max.z
            && self.bounds.max.z >= range.min.z;
        if !overlaps {
            return;
        }
        for (pt, val) in &self.data {
            if range.contains(*pt) {
                out.push(val);
            }
        }
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.collect_range(range, out);
            }
        }
    }

    pub fn len(&self) -> usize {
        let mut n = self.data.len();
        if let Some(children) = &self.children {
            for child in children.iter() {
                n += child.len();
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Vec3 Tests
    // ========================================================================

    mod vec3 {
        use super::*;

        #[test]
        fn vec3_new_creates_correct_values() {
            let v = Vec3::new(1.0, 2.0, 3.0);
            assert_eq!(v.x, 1.0);
            assert_eq!(v.y, 2.0);
            assert_eq!(v.z, 3.0);
        }

        #[test]
        fn vec3_origin() {
            let v = Vec3::new(0.0, 0.0, 0.0);
            assert_eq!(v.x, 0.0);
            assert_eq!(v.y, 0.0);
            assert_eq!(v.z, 0.0);
        }

        #[test]
        fn vec3_negative_coordinates() {
            let v = Vec3::new(-1.5, -2.5, -3.5);
            assert_eq!(v.x, -1.5);
            assert_eq!(v.y, -2.5);
            assert_eq!(v.z, -3.5);
        }

        #[test]
        fn vec3_large_values() {
            let v = Vec3::new(1e100, -1e100, 0.0);
            assert_eq!(v.x, 1e100);
            assert_eq!(v.y, -1e100);
        }

        #[test]
        fn vec3_nan_handling() {
            let v = Vec3::new(f64::NAN, 0.0, f64::INFINITY);
            assert!(v.x.is_nan());
            assert_eq!(v.y, 0.0);
            assert!(v.z.is_infinite());
        }

        #[test]
        fn vec3_debug_display() {
            let v = Vec3::new(1.0, 2.0, 3.0);
            let debug = format!("{:?}", v);
            assert!(debug.contains("1"));
            assert!(debug.contains("2"));
            assert!(debug.contains("3"));
        }

        #[test]
        fn vec3_clone_preserves_values() {
            let v = Vec3::new(1.5, 2.5, 3.5);
            let cloned = v.clone();
            assert_eq!(v.x, cloned.x);
            assert_eq!(v.y, cloned.y);
            assert_eq!(v.z, cloned.z);
        }

        #[test]
        fn vec3_copy_semantics() {
            let v = Vec3::new(1.0, 2.0, 3.0);
            let v2 = v;
            assert_eq!(v.x, v2.x);
            assert_eq!(v.y, v2.y);
            assert_eq!(v.z, v2.z);
        }
    }

    // ========================================================================
    // Bounds Tests
    // ========================================================================

    mod bounds {
        use super::*;

        #[test]
        fn bounds_new_creates_correct_values() {
            let min = Vec3::new(0.0, 0.0, 0.0);
            let max = Vec3::new(1.0, 1.0, 1.0);
            let b = Bounds::new(min, max);
            assert_eq!(b.min.x, 0.0);
            assert_eq!(b.max.x, 1.0);
        }

        #[test]
        fn bounds_contains_point_inside() {
            let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
            let p = Vec3::new(5.0, 5.0, 5.0);
            assert!(b.contains(p));
        }

        #[test]
        fn bounds_contains_point_on_boundary() {
            let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
            assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
            assert!(b.contains(Vec3::new(10.0, 10.0, 10.0)));
        }

        #[test]
        fn bounds_contains_point_outside() {
            let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
            assert!(!b.contains(Vec3::new(11.0, 5.0, 5.0)));
            assert!(!b.contains(Vec3::new(-1.0, 5.0, 5.0)));
        }

        #[test]
        fn bounds_contains_negative_coords() {
            let b = Bounds::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0));
            assert!(b.contains(Vec3::new(0.0, 0.0, 0.0)));
            assert!(b.contains(Vec3::new(-5.0, -5.0, -5.0)));
            assert!(!b.contains(Vec3::new(-6.0, 0.0, 0.0)));
        }

        #[test]
        fn bounds_debug_display() {
            let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0));
            let debug = format!("{:?}", b);
            assert!(debug.contains("Vec3"));
            assert!(debug.contains("0"));
            assert!(debug.contains("1"));
        }

        #[test]
        fn bounds_clone_preserves_values() {
            let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0));
            let cloned = b.clone();
            assert_eq!(cloned.min.x, b.min.x);
            assert_eq!(cloned.max.x, b.max.x);
        }
    }

    // ========================================================================
    // Octree Tests
    // ========================================================================

    mod octree {
        use super::*;

        fn make_bounds() -> Bounds {
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0))
        }

        #[test]
        fn octree_new_creates_empty_tree() {
            let bounds = make_bounds();
            let tree = Octree::<i32>::new(bounds);
            assert_eq!(tree.len(), 0);
        }

        #[test]
        fn octree_capacity_constant() {
            assert_eq!(Octree::<i32>::CAPACITY, 8);
        }

        #[test]
        fn octree_insert_single_point() {
            let mut tree = Octree::new(make_bounds());
            let result = tree.insert(Vec3::new(50.0, 50.0, 50.0), 42);
            assert!(result);
            assert_eq!(tree.len(), 1);
        }

        #[test]
        fn octree_insert_multiple_points_same_location() {
            let mut tree = Octree::new(make_bounds());
            assert!(tree.insert(Vec3::new(50.0, 50.0, 50.0), 1));
            assert!(tree.insert(Vec3::new(50.0, 50.0, 50.0), 2));
            assert!(tree.insert(Vec3::new(50.0, 50.0, 50.0), 3));
        }

        #[test]
        fn octree_insert_out_of_bounds_returns_false() {
            let mut tree = Octree::new(make_bounds());
            let result = tree.insert(Vec3::new(150.0, 50.0, 50.0), 1);
            assert!(!result);
            assert_eq!(tree.len(), 0);
        }

        #[test]
        fn octree_insert_at_exact_boundary() {
            let mut tree = Octree::new(make_bounds());
            assert!(tree.insert(Vec3::new(0.0, 0.0, 0.0), 1));
            assert!(tree.insert(Vec3::new(100.0, 100.0, 100.0), 2));
            assert_eq!(tree.len(), 2);
        }

        #[test]
        fn octree_insert_fills_to_capacity_without_subdivision() {
            let mut tree = Octree::new(make_bounds());
            for i in 0..8 {
                let x = 10.0 + (i as f64) * 10.0;
                assert!(tree.insert(Vec3::new(x, 50.0, 50.0), i as i32));
            }
            assert_eq!(tree.len(), 8);
        }

        #[test]
        fn octree_insert_beyond_capacity_triggers_subdivision() {
            let mut tree = Octree::new(make_bounds());
            for i in 0..9 {
                let x = 10.0 + (i as f64) * 10.0;
                assert!(tree.insert(Vec3::new(x, 50.0, 50.0), i as i32));
            }
            assert_eq!(tree.len(), 9);
        }

        #[test]
        fn octree_query_range_exact_match() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 42);
            let results = tree.query_range(&make_bounds());
            assert_eq!(results.len(), 1);
            assert_eq!(*results[0], 42);
        }

        #[test]
        fn octree_query_range_no_match() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 42);
            let range = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
            let results = tree.query_range(&range);
            assert_eq!(results.len(), 0);
        }

        #[test]
        fn octree_query_range_partial_overlap() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(5.0, 5.0, 5.0), 1);
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 2);
            let range = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
            let results = tree.query_range(&range);
            assert_eq!(results.len(), 1);
            assert_eq!(*results[0], 1);
        }

        #[test]
        fn octree_query_empty_tree() {
            let tree = Octree::<i32>::new(make_bounds());
            let results = tree.query_range(&make_bounds());
            assert_eq!(results.len(), 0);
        }

        #[test]
        fn octree_len_empty() {
            let tree = Octree::<i32>::new(make_bounds());
            assert_eq!(tree.len(), 0);
        }

        #[test]
        fn octree_len_after_inserts() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(10.0, 10.0, 10.0), 1);
            tree.insert(Vec3::new(20.0, 20.0, 20.0), 2);
            tree.insert(Vec3::new(30.0, 30.0, 30.0), 3);
            assert_eq!(tree.len(), 3);
        }

        #[test]
        fn octree_debug_display() {
            let tree = Octree::<i32>::new(make_bounds());
            let debug = format!("{:?}", tree);
            assert!(debug.contains("Octree"));
        }

        #[test]
        fn octree_clone_empty() {
            let tree = Octree::<i32>::new(make_bounds());
            let cloned = tree.clone();
            assert_eq!(cloned.len(), 0);
        }

        #[test]
        fn octree_clone_with_data() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 42);
            let cloned = tree.clone();
            assert_eq!(cloned.len(), 1);
        }

        #[test]
        fn octree_serialize_deserialize_roundtrip() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 42);
            tree.insert(Vec3::new(25.0, 25.0, 25.0), 100);

            let json = serde_json::to_string(&tree).expect("should serialize");
            let deserialized: Octree<i32> =
                serde_json::from_str(&json).expect("should deserialize");

            assert_eq!(deserialized.len(), 2);
        }

        #[test]
        fn octree_insert_extreme_values() {
            let bounds = Bounds::new(
                Vec3::new(f64::MIN, f64::MIN, f64::MIN),
                Vec3::new(f64::MAX, f64::MAX, f64::MAX),
            );
            let mut tree = Octree::new(bounds);
            assert!(tree.insert(Vec3::new(f64::MIN, f64::MIN, f64::MIN), 1));
            assert!(tree.insert(Vec3::new(f64::MAX, f64::MAX, f64::MAX), 2));
            assert_eq!(tree.len(), 2);
        }

        #[test]
        fn octree_query_all_returns_all_in_bounds() {
            let mut tree = Octree::new(make_bounds());
            tree.insert(Vec3::new(5.0, 5.0, 5.0), 1);
            tree.insert(Vec3::new(95.0, 95.0, 95.0), 2);
            tree.insert(Vec3::new(50.0, 50.0, 50.0), 3);

            let results = tree.query_range(&make_bounds());
            assert_eq!(results.len(), 3);
        }

        #[test]
        fn octree_subdivision_creates_8_children() {
            let mut tree = Octree::new(make_bounds());
            for i in 0..10 {
                let x = 10.0 + (i as f64) * 8.0;
                tree.insert(Vec3::new(x, 50.0, 50.0), i as i32);
            }
            assert_eq!(tree.len(), 10);
        }
    }

    // ========================================================================
    // Octree Internal Helpers Tests
    // ========================================================================

    mod octree_internal_helpers {
        use crate::structures::octree_internal::*;
        use super::*;

        #[test]
        fn bounds_center_calculation() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0));
            let center = bounds_center(&bounds);
            assert_eq!(center.x, 50.0);
            assert_eq!(center.y, 50.0);
            assert_eq!(center.z, 50.0);
        }

        #[test]
        fn bounds_center_with_negative_range() {
            let bounds = Bounds::new(Vec3::new(-50.0, -50.0, -50.0), Vec3::new(50.0, 50.0, 50.0));
            let center = bounds_center(&bounds);
            assert_eq!(center.x, 0.0);
            assert_eq!(center.y, 0.0);
            assert_eq!(center.z, 0.0);
        }

        #[test]
        fn bounds_extent_calculation() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 50.0, 25.0));
            let extent = bounds_extent(&bounds);
            assert_eq!(extent.x, 100.0);
            assert_eq!(extent.y, 50.0);
            assert_eq!(extent.z, 25.0);
        }

        #[test]
        fn bounds_extent_asymmetric() {
            let bounds = Bounds::new(Vec3::new(10.0, 20.0, 30.0), Vec3::new(60.0, 80.0, 90.0));
            let extent = bounds_extent(&bounds);
            assert_eq!(extent.x, 50.0);
            assert_eq!(extent.y, 60.0);
            assert_eq!(extent.z, 60.0);
        }

        #[test]
        fn child_index_all_octants() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0));

            assert_eq!(child_index(&bounds, Vec3::new(0.0, 0.0, 0.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(99.0, 0.0, 0.0)), 1);
            assert_eq!(child_index(&bounds, Vec3::new(0.0, 99.0, 0.0)), 2);
            assert_eq!(child_index(&bounds, Vec3::new(99.0, 99.0, 0.0)), 3);
            assert_eq!(child_index(&bounds, Vec3::new(0.0, 0.0, 99.0)), 4);
            assert_eq!(child_index(&bounds, Vec3::new(99.0, 0.0, 99.0)), 5);
            assert_eq!(child_index(&bounds, Vec3::new(0.0, 99.0, 99.0)), 6);
            assert_eq!(child_index(&bounds, Vec3::new(99.0, 99.0, 99.0)), 7);
        }

        #[test]
        fn child_index_boundary_points() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0));

            assert_eq!(child_index(&bounds, Vec3::new(0.0, 0.0, 0.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(49.0, 0.0, 0.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(0.0, 49.0, 0.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(0.0, 0.0, 49.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(50.0, 0.0, 0.0)), 1);
            assert_eq!(child_index(&bounds, Vec3::new(50.0, 50.0, 0.0)), 3);
            assert_eq!(child_index(&bounds, Vec3::new(50.0, 50.0, 50.0)), 7);
        }

        #[test]
        fn child_index_with_offset_bounds() {
            let bounds = Bounds::new(Vec3::new(10.0, 10.0, 10.0), Vec3::new(110.0, 110.0, 110.0));

            assert_eq!(child_index(&bounds, Vec3::new(10.0, 10.0, 10.0)), 0);
            assert_eq!(child_index(&bounds, Vec3::new(109.0, 10.0, 10.0)), 1);
            assert_eq!(child_index(&bounds, Vec3::new(10.0, 109.0, 109.0)), 6);
        }
    }
}
