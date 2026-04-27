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
        self.children.as_mut().is_some_and(|children| {
            children
                .iter_mut()
                .any(|child| child.insert(point, value.clone()))
        })
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
        if let Some(children) = self.children.as_mut() {
            drained.into_iter().for_each(|(pt, val)| {
                let _ = children
                    .iter_mut()
                    .any(|child| child.insert(pt, val.clone()));
            });
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec3_new_constructs() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn vec3_new_negative_values() {
        let v = Vec3::new(-1.0, -2.0, -3.0);
        assert_eq!(v.x, -1.0);
        assert_eq!(v.y, -2.0);
        assert_eq!(v.z, -3.0);
    }

    #[test]
    fn vec3_new_zero() {
        let v = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.z, 0.0);
    }

    #[test]
    fn vec3_new_fractional() {
        let v = Vec3::new(0.5, -0.25, 1e-10);
        assert_eq!(v.x, 0.5);
        assert_eq!(v.y, -0.25);
        assert_eq!(v.z, 1e-10);
    }

    #[test]
    fn vec3_clone_equals() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let c = v;
        assert_eq!(v, c);
    }

    #[test]
    fn vec3_debug_format() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let debug_str = format!("{:?}", v);
        assert!(debug_str.contains("1"));
        assert!(debug_str.contains("2"));
        assert!(debug_str.contains("3"));
    }

    #[test]
    fn bounds_new_constructs() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        assert_eq!(b.min.x, 0.0);
        assert_eq!(b.max.x, 10.0);
    }

    #[test]
    fn bounds_contains_point_inside() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(5.0, 5.0, 5.0);
        assert!(b.contains(p));
    }

    #[test]
    fn bounds_contains_point_on_min_boundary() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(0.0, 0.0, 0.0);
        assert!(b.contains(p));
    }

    #[test]
    fn bounds_contains_point_on_max_boundary() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(10.0, 10.0, 10.0);
        assert!(b.contains(p));
    }

    #[test]
    fn bounds_contains_point_outside() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(15.0, 5.0, 5.0);
        assert!(!b.contains(p));
    }

    #[test]
    fn bounds_contains_point_below_min() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(-1.0, 5.0, 5.0);
        assert!(!b.contains(p));
    }

    #[test]
    fn bounds_contains_negative_coordinates() {
        let b = Bounds::new(Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0));
        let p = Vec3::new(-5.0, -5.0, -5.0);
        assert!(b.contains(p));
    }

    #[test]
    fn bounds_contains_zero_volume() {
        let b = Bounds::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(5.0, 5.0, 5.0));
        let p = Vec3::new(5.0, 5.0, 5.0);
        assert!(b.contains(p));
    }

    #[test]
    fn bounds_debug_format() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0));
        let debug_str = format!("{:?}", b);
        assert!(debug_str.contains("Vec3"));
    }

    #[test]
    fn octree_new_is_empty() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let tree = Octree::<i32>::new(b);
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn octree_insert_single_point() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        let p = Vec3::new(5.0, 5.0, 5.0);
        assert!(tree.insert(p, 42));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn octree_insert_outside_bounds_rejected() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        let p = Vec3::new(15.0, 15.0, 15.0);
        assert!(!tree.insert(p, 1));
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn octree_insert_multiple_points() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        tree.insert(Vec3::new(1.0, 1.0, 1.0), 10);
        tree.insert(Vec3::new(2.0, 2.0, 2.0), 20);
        tree.insert(Vec3::new(3.0, 3.0, 3.0), 30);
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn octree_query_range_finds_inserted() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        tree.insert(Vec3::new(5.0, 5.0, 5.0), 42);
        let range = Bounds::new(Vec3::new(4.0, 4.0, 4.0), Vec3::new(6.0, 6.0, 6.0));
        let found = tree.query_range(&range);
        assert!(found.contains(&&42));
    }

    #[test]
    fn octree_query_range_empty_when_no_overlap() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        tree.insert(Vec3::new(5.0, 5.0, 5.0), 42);
        let range = Bounds::new(Vec3::new(100.0, 100.0, 100.0), Vec3::new(200.0, 200.0, 200.0));
        assert!(tree.query_range(&range).is_empty());
    }

    #[test]
    fn octree_capacity_before_subdivision() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::<i32>::new(b);
        for i in 0..8 {
            let x = 1.0 + (i as f64) * 0.5;
            tree.insert(Vec3::new(x, 1.0, 1.0), i as i32);
        }
        assert_eq!(tree.len(), 8);
    }

    #[test]
    fn octree_subdivision_triggers_after_capacity() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::<i32>::new(b);
        for i in 0..9 {
            let x = 1.0 + (i as f64) * 0.5;
            tree.insert(Vec3::new(x, 1.0, 1.0), i as i32);
        }
        assert_eq!(tree.len(), 9);
    }

    #[test]
    fn octree_query_range_partial_overlap() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let mut tree = Octree::new(b);
        tree.insert(Vec3::new(1.0, 1.0, 1.0), 10);
        tree.insert(Vec3::new(8.0, 8.0, 8.0), 80);
        let range = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0));
        let found = tree.query_range(&range);
        assert_eq!(found.len(), 1);
        assert!(found.contains(&&10));
    }
}
