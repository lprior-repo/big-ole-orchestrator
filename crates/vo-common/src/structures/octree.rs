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

    #[inline]
    pub fn distance_to(&self, other: Vec3) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
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

    /// Minimum Euclidean distance from a point to this bounding box.
    /// Returns 0.0 if the point is inside the bounds.
    #[inline]
    pub fn min_distance_to(&self, p: Vec3) -> f64 {
        let dx = if p.x < self.min.x {
            self.min.x - p.x
        } else if p.x > self.max.x {
            p.x - self.max.x
        } else {
            0.0
        };
        let dy = if p.y < self.min.y {
            self.min.y - p.y
        } else if p.y > self.max.y {
            p.y - self.max.y
        } else {
            0.0
        };
        let dz = if p.z < self.min.z {
            self.min.z - p.z
        } else if p.z > self.max.z {
            p.z - self.max.z
        } else {
            0.0
        };
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Octree<T: Clone + Serialize> {
    bounds: Bounds,
    data: Vec<(Vec3, T)>,
    children: Option<Box<[Octree<T>; 8]>>,
    empty_child_streak: u8,
    degenerate: bool,
}

impl<T: Clone + Serialize> Octree<T> {
    pub const CAPACITY: usize = 8;

    #[inline]
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            data: Vec::new(),
            children: None,
            empty_child_streak: 0,
            degenerate: false,
        }
    }

    pub fn insert(&mut self, point: Vec3, value: T) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        if self.degenerate {
            self.data.push((point, value));
            return true;
        }
        if self.children.is_none() && self.data.len() < Self::CAPACITY {
            self.data.push((point, value));
            return true;
        }
        self.subdivide();
        if let Some(children) = self.children.as_mut() {
            for child in children.iter_mut() {
                if child.insert(point, value.clone()) {
                    return true;
                }
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
        if let Some(children) = self.children.as_mut() {
            for (pt, val) in drained {
                let mut inserted = false;
                for child in children.iter_mut() {
                    if child.insert(pt, val.clone()) {
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    self.empty_child_streak = 0;
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
        if self.degenerate {
            return;
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

    // -- Accessors for internal traversal (used by octree_internal) --

    #[inline]
    pub fn local_data(&self) -> &[(Vec3, T)] {
        &self.data
    }

    #[inline]
    pub fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    #[inline]
    pub fn children(&self) -> Option<&[Octree<T>; 8]> {
        self.children.as_deref()
    }

    #[inline]
    pub fn is_degenerate(&self) -> bool {
        self.degenerate
    }

    // -- Nearest-neighbor queries --

    /// Returns the closest point and its value to the query point, or None if the tree is empty.
    pub fn nearest(&self, point: Vec3) -> Option<(&Vec3, &T)> {
        let mut best: Option<(f64, &Vec3, &T)> = None;
        self.nearest_recursive(point, &mut best);
        best.map(|(_, p, v)| (p, v))
    }

    fn nearest_recursive<'a>(&'a self, point: Vec3, best: &mut Option<(f64, &'a Vec3, &'a T)>) {
        let min_dist = self.bounds.min_distance_to(point);
        if let Some((bd, _, _)) = best {
            if min_dist > *bd {
                return;
            }
        }

        for (pt, val) in &self.data {
            let dist = pt.distance_to(point);
            match best {
                None => *best = Some((dist, pt, val)),
                Some((bd, _, _)) if dist < *bd => *best = Some((dist, pt, val)),
                _ => {}
            }
        }

        if self.degenerate {
            return;
        }

        if let Some(children) = &self.children {
            let mut indexed: Vec<(f64, usize)> = children
                .iter()
                .enumerate()
                .map(|(i, c)| (c.bounds.min_distance_to(point), i))
                .collect();
            indexed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            for (_, idx) in indexed {
                children[idx].nearest_recursive(point, best);
            }
        }
    }

    /// Returns the k closest points to the query point, sorted by ascending distance.
    /// Returns fewer than k results if the tree contains fewer than k points.
    pub fn k_nearest(&self, point: Vec3, k: usize) -> Vec<(Vec3, T)> {
        crate::structures::octree_internal::k_nearest_search(self, point, k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree_with_capacity(bounds: Bounds, points: &[(Vec3, i32)]) -> Octree<i32> {
        let mut tree = Octree::new(bounds);
        for &(pt, val) in points {
            assert!(tree.insert(pt, val));
        }
        tree
    }

    #[test]
    fn test_nearest_single_point() {
        let tree = make_tree_with_capacity(
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0)),
            &[(Vec3::new(1.0, 2.0, 3.0), 42)],
        );
        let (pt, val) = tree.nearest(Vec3::new(1.0, 2.0, 3.0)).unwrap();
        assert_eq!(*val, 42);
        assert_eq!(*pt, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_nearest_picks_closest() {
        let mut points = Vec::new();
        for i in 0..100i32 {
            let x = (i * 7 % 100) as f64;
            let y = (i * 13 % 100) as f64;
            let z = (i * 3 % 100) as f64;
            points.push((Vec3::new(x, y, z), i));
        }
        let tree = make_tree_with_capacity(
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0)),
            &points,
        );

        let query = Vec3::new(50.0, 50.0, 50.0);
        let result = tree.nearest(query).unwrap();

        // Brute-force verify
        let mut best_dist = f64::INFINITY;
        let mut best_val = 0;
        for &(pt, val) in &points {
            let d = pt.distance_to(query);
            if d < best_dist {
                best_dist = d;
                best_val = val;
            }
        }
        assert_eq!(*result.1, best_val);
    }

    #[test]
    fn test_k_nearest_returns_k() {
        let mut points = Vec::new();
        for i in 0..50i32 {
            points.push((Vec3::new(i as f64, i as f64, i as f64), i));
        }
        let tree = make_tree_with_capacity(
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0)),
            &points,
        );
        let results = tree.k_nearest(Vec3::new(0.0, 0.0, 0.0), 5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_k_nearest_less_than_k() {
        let tree = make_tree_with_capacity(
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0)),
            &[
                (Vec3::new(1.0, 0.0, 0.0), 1),
                (Vec3::new(2.0, 0.0, 0.0), 2),
                (Vec3::new(3.0, 0.0, 0.0), 3),
            ],
        );
        let results = tree.k_nearest(Vec3::new(0.0, 0.0, 0.0), 10);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_nearest_empty_tree() {
        let tree: Octree<i32> = Octree::new(Bounds::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 100.0, 100.0),
        ));
        assert!(tree.nearest(Vec3::new(50.0, 50.0, 50.0)).is_none());
    }

    #[test]
    fn test_k_nearest_correct_ordering() {
        let tree = make_tree_with_capacity(
            Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0)),
            &[
                (Vec3::new(10.0, 0.0, 0.0), 10),
                (Vec3::new(1.0, 0.0, 0.0), 1),
                (Vec3::new(5.0, 0.0, 0.0), 5),
                (Vec3::new(20.0, 0.0, 0.0), 20),
                (Vec3::new(3.0, 0.0, 0.0), 3),
            ],
        );
        let query = Vec3::new(0.0, 0.0, 0.0);
        let results = tree.k_nearest(query, 5);
        assert_eq!(results.len(), 5);
        for window in results.windows(2) {
            let d0 = window[0].0.distance_to(query);
            let d1 = window[1].0.distance_to(query);
            assert!(d0 <= d1, "Results not sorted: {} > {}", d0, d1);
        }
        // Verify specific ordering by value (distance from origin)
        assert_eq!(results[0].1, 1);
        assert_eq!(results[1].1, 3);
        assert_eq!(results[2].1, 5);
        assert_eq!(results[3].1, 10);
        assert_eq!(results[4].1, 20);
    }
}
