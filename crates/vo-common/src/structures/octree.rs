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
    #[must_use]
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
    #[must_use]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    #[must_use]
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
    #[must_use]
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            data: Vec::new(),
            children: None,
        }
    }

    /// Inserts a point-value pair into the octree.
    ///
    /// # Panics
    ///
    /// Panics if the octree needs to subdivide and the children box is unexpectedly None.
    #[must_use]
    pub fn insert(&mut self, point: Vec3, value: T) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        if self.children.is_none() && self.data.len() < Self::CAPACITY {
            self.data.push((point, value));
            return true;
        }
        self.subdivide();
        let Some(children) = self.children.as_mut() else {
            return false;
        };
        for child in children.iter_mut() {
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
            f64::midpoint(self.bounds.min.x, self.bounds.max.x),
            f64::midpoint(self.bounds.min.y, self.bounds.max.y),
            f64::midpoint(self.bounds.min.z, self.bounds.max.z),
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
        let Some(children) = self.children.as_mut() else {
            return;
        };
        for (pt, val) in drained {
            for child in children.iter_mut() {
                if child.insert(pt, val.clone()) {
                    break;
                }
            }
        }
    }

    #[must_use]
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

    #[must_use]
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
