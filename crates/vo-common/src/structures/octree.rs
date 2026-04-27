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

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        fn vec3_negative_values() {
            let v = Vec3::new(-1.0, -2.0, -3.0);
            assert_eq!(v.x, -1.0);
            assert_eq!(v.y, -2.0);
            assert_eq!(v.z, -3.0);
        }

        #[test]
        fn vec3_debug_format() {
            let v = Vec3::new(1.0, 2.0, 3.0);
            let debug = format!("{:?}", v);
            assert!(debug.contains("1"));
            assert!(debug.contains("2"));
            assert!(debug.contains("3"));
        }

        #[test]
        fn vec3_clone_preserves_data() {
            let v1 = Vec3::new(1.0, 2.0, 3.0);
            let v2 = v1.clone();
            assert_eq!(v1.x, v2.x);
            assert_eq!(v1.y, v2.y);
            assert_eq!(v1.z, v2.z);
        }

        #[test]
        fn vec3_partial_eq() {
            let v1 = Vec3::new(1.0, 2.0, 3.0);
            let v2 = Vec3::new(1.0, 2.0, 3.0);
            let v3 = Vec3::new(1.0, 2.0, 4.0);
            assert_eq!(v1, v2);
            assert_ne!(v1, v3);
        }
    }

    mod bounds {
        use super::*;

        #[test]
        fn bounds_new_creates_correct_values() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            assert_eq!(bounds.min.x, 0.0);
            assert_eq!(bounds.max.x, 1.0);
        }

        #[test]
        fn bounds_contains_point_inside() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            let point = Vec3::new(0.5, 0.5, 0.5);
            assert!(bounds.contains(point));
        }

        #[test]
        fn bounds_contains_point_on_boundary() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            assert!(bounds.contains(Vec3::new(0.0, 0.0, 0.0)));
            assert!(bounds.contains(Vec3::new(1.0, 1.0, 1.0)));
        }

        #[test]
        fn bounds_contains_point_outside() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            assert!(!bounds.contains(Vec3::new(2.0, 0.5, 0.5)));
            assert!(!bounds.contains(Vec3::new(-1.0, 0.5, 0.5)));
        }

        #[test]
        fn bounds_contains_negative_space() {
            let bounds = Bounds::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(0.0, 0.0, 0.0));
            assert!(bounds.contains(Vec3::new(-0.5, -0.5, -0.5)));
            assert!(!bounds.contains(Vec3::new(0.5, 0.5, 0.5)));
        }

        #[test]
        fn bounds_debug_format() {
            let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            let debug = format!("{:?}", bounds);
            assert!(debug.contains("Bounds"));
        }

        #[test]
        fn bounds_clone_preserves_data() {
            let b1 = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
            let b2 = b1.clone();
            assert_eq!(b1.min, b2.min);
            assert_eq!(b1.max, b2.max);
        }
    }

    mod octree {
        use super::*;

        fn create_test_bounds() -> Bounds {
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0))
        }

        #[test]
        fn octree_new_creates_empty_tree() {
            let tree: Octree<i32> = Octree::new(create_test_bounds());
            assert_eq!(tree.len(), 0);
            assert!(tree.is_empty());
        }

        #[test]
        fn octree_insert_single_point() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            let result = tree.insert(Vec3::new(5.0, 5.0, 5.0), 42);
            assert!(result);
            assert_eq!(tree.len(), 1);
        }

        #[test]
        fn octree_insert_out_of_bounds() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            let result = tree.insert(Vec3::new(15.0, 15.0, 15.0), 42);
            assert!(!result);
            assert_eq!(tree.len(), 0);
        }

        #[test]
        fn octree_insert_at_boundary() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            let result = tree.insert(Vec3::new(0.0, 0.0, 0.0), 1);
            assert!(result);
            let result = tree.insert(Vec3::new(10.0, 10.0, 10.0), 2);
            assert!(result);
            assert_eq!(tree.len(), 2);
        }

        #[test]
        fn octree_query_range_empty_tree() {
            let tree: Octree<i32> = Octree::new(create_test_bounds());
            let range = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0));
            let results = tree.query_range(&range);
            assert!(results.is_empty());
        }

        #[test]
        fn octree_query_range_finds_points() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            tree.insert(Vec3::new(2.0, 2.0, 2.0), 1);
            tree.insert(Vec3::new(8.0, 8.0, 8.0), 2);
            tree.insert(Vec3::new(3.0, 3.0, 3.0), 3);

            let range = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0));
            let results = tree.query_range(&range);
            assert_eq!(results.len(), 2);
        }

        #[test]
        fn octree_query_range_no_match() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            tree.insert(Vec3::new(2.0, 2.0, 2.0), 1);

            let range = Bounds::new(Vec3::new(6.0, 6.0, 6.0), Vec3::new(10.0, 10.0, 10.0));
            let results = tree.query_range(&range);
            assert!(results.is_empty());
        }

        #[test]
        fn octree_insert_same_point_twice() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            tree.insert(Vec3::new(5.0, 5.0, 5.0), 1);
            tree.insert(Vec3::new(5.0, 5.0, 5.0), 2);
            assert_eq!(tree.len(), 2);
        }

        #[test]
        fn octree_capacity_triggers_subdivision() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            for i in 0..9 {
                let x = 1.0 + (i as f64);
                tree.insert(Vec3::new(x, 1.0, 1.0), i as i32);
            }
            assert_eq!(tree.len(), 9);
        }

        #[test]
        fn octree_debug_format() {
            let tree: Octree<i32> = Octree::new(create_test_bounds());
            let debug = format!("{:?}", tree);
            assert!(debug.contains("Octree"));
        }

        #[test]
        fn octree_with_string_data() {
            let mut tree: Octree<String> = Octree::new(create_test_bounds());
            tree.insert(Vec3::new(1.0, 2.0, 3.0), "test".to_string());
            assert_eq!(tree.len(), 1);
        }

        #[test]
        fn octree_query_range_negative_space() {
            let mut tree: Octree<i32> = Octree::new(create_test_bounds());
            tree.insert(Vec3::new(-2.0, -2.0, -2.0), 1);
            let range = Bounds::new(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(0.0, 0.0, 0.0));
            let results = tree.query_range(&range);
            assert!(results.is_empty());
        }
    }
}
