//! Octree - 3D spatial partitioning tree.

use serde::{Deserialize, Serialize};

/// A point in 3D space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub const ORIGIN: Self = Self::new(0.0, 0.0, 0.0);

    pub fn distance_sq(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn distance(&self, other: &Self) -> f64 {
        self.distance_sq(other).sqrt()
    }
}

/// An axis-aligned bounding box in 3D.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Point3,
    pub max: Point3,
}

impl BoundingBox {
    pub const fn new(min: Point3, max: Point3) -> Self {
        Self { min, max }
    }

    pub const fn centered(half_extent: f64) -> Self {
        Self {
            min: Point3::new(-half_extent, -half_extent, -half_extent),
            max: Point3::new(half_extent, half_extent, half_extent),
        }
    }

    pub fn contains(&self, p: &Point3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    pub fn center(&self) -> Point3 {
        Point3::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
            (self.min.z + self.max.z) / 2.0,
        )
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Child octant bounding box for index 0-7.
    /// Bit flags: bit 2 = +x, bit 1 = +y, bit 0 = +z.
    pub fn octant(&self, index: u8) -> BoundingBox {
        debug_assert!(index < 8);
        let c = self.center();
        let (min_x, max_x) = if index & 4 != 0 {
            (c.x, self.max.x)
        } else {
            (self.min.x, c.x)
        };
        let (min_y, max_y) = if index & 2 != 0 {
            (c.y, self.max.y)
        } else {
            (self.min.y, c.y)
        };
        let (min_z, max_z) = if index & 1 != 0 {
            (c.z, self.max.z)
        } else {
            (self.min.z, c.z)
        };
        BoundingBox {
            min: Point3::new(min_x, min_y, min_z),
            max: Point3::new(max_x, max_y, max_z),
        }
    }

    pub fn octant_index(&self, p: &Point3) -> Option<u8> {
        if !self.contains(p) {
            return None;
        }
        let c = self.center();
        let mut idx = 0u8;
        if p.x >= c.x {
            idx |= 4;
        }
        if p.y >= c.y {
            idx |= 2;
        }
        if p.z >= c.z {
            idx |= 1;
        }
        Some(idx)
    }
}

/// Octree configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OctreeConfig {
    pub max_depth: u32,
    pub bucket_size: usize,
}

impl Default for OctreeConfig {
    fn default() -> Self {
        Self {
            max_depth: 8,
            bucket_size: 16,
        }
    }
}

impl OctreeConfig {
    pub const fn new(max_depth: u32, bucket_size: usize) -> Self {
        Self {
            max_depth,
            bucket_size,
        }
    }
}

/// Error returned by Octree operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OctreeError {
    #[error("point {point:?} is outside the octree bounds")]
    OutOfBounds { point: Point3 },
}

/// An entry stored in the octree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OctreeEntry<T> {
    pub point: Point3,
    pub value: T,
}

impl<T> OctreeEntry<T> {
    pub const fn new(point: Point3, value: T) -> Self {
        Self { point, value }
    }
}

/// A node in the octree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OctreeNode<T> {
    Leaf {
        entries: Vec<OctreeEntry<T>>,
    },
    Interior {
        children: Box<[Option<OctreeNode<T>>; 8]>,
    },
}

impl<T> OctreeNode<T> {
    fn empty_leaf() -> Self {
        OctreeNode::Leaf {
            entries: Vec::new(),
        }
    }
}

/// A point octree for spatial queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Octree<T> {
    root: OctreeNode<T>,
    bounds: BoundingBox,
    config: OctreeConfig,
    len: usize,
}

impl<T> Octree<T> {
    pub fn new(bounds: BoundingBox, config: OctreeConfig) -> Self {
        Self {
            root: OctreeNode::empty_leaf(),
            bounds,
            config,
            len: 0,
        }
    }

    pub fn bounds(&self) -> &BoundingBox {
        &self.bounds
    }

    pub fn config(&self) -> &OctreeConfig {
        &self.config
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, point: Point3, value: T) -> Result<(), OctreeError> {
        if !self.bounds.contains(&point) {
            return Err(OctreeError::OutOfBounds { point });
        }
        self.root = Self::insert_rec(
            std::mem::replace(&mut self.root, OctreeNode::empty_leaf()),
            self.bounds,
            point,
            value,
            0,
            self.config,
        );
        self.len += 1;
        Ok(())
    }

    fn insert_rec(
        mut node: OctreeNode<T>,
        bounds: BoundingBox,
        point: Point3,
        value: T,
        depth: u32,
        config: OctreeConfig,
    ) -> OctreeNode<T> {
        match &mut node {
            OctreeNode::Leaf { entries } => {
                entries.push(OctreeEntry::new(point, value));
                if entries.len() > config.bucket_size && depth < config.max_depth {
                    let taken = std::mem::take(entries);
                    let mut children: Box<[Option<OctreeNode<T>>; 8]> =
                        Box::new([None, None, None, None, None, None, None, None]);
                    for entry in taken {
                        let idx = bounds.octant_index(&entry.point).expect("entry in bounds");
                        let child_bounds = bounds.octant(idx);
                        children[idx as usize] = Some(Self::insert_rec(
                            children[idx as usize]
                                .take()
                                .unwrap_or_else(OctreeNode::empty_leaf),
                            child_bounds,
                            entry.point,
                            entry.value,
                            depth + 1,
                            config,
                        ));
                    }
                    OctreeNode::Interior { children }
                } else {
                    node
                }
            }
            OctreeNode::Interior { children } => {
                let idx = bounds.octant_index(&point).expect("point in bounds");
                let child_bounds = bounds.octant(idx);
                children[idx as usize] = Some(Self::insert_rec(
                    children[idx as usize]
                        .take()
                        .unwrap_or_else(OctreeNode::empty_leaf),
                    child_bounds,
                    point,
                    value,
                    depth + 1,
                    config,
                ));
                node
            }
        }
    }

    pub fn query(&self, query_bounds: BoundingBox) -> Vec<&OctreeEntry<T>> {
        let mut results = Vec::new();
        Self::query_rec(&self.root, &self.bounds, &query_bounds, &mut results);
        results
    }

    fn query_rec<'a>(
        node: &'a OctreeNode<T>,
        node_bounds: &BoundingBox,
        query_bounds: &BoundingBox,
        results: &mut Vec<&'a OctreeEntry<T>>,
    ) {
        if !node_bounds.intersects(query_bounds) {
            return;
        }
        match node {
            OctreeNode::Leaf { entries } => {
                for entry in entries {
                    if query_bounds.contains(&entry.point) {
                        results.push(entry);
                    }
                }
            }
            OctreeNode::Interior { children } => {
                for (i, child) in children.iter().enumerate() {
                    if let Some(child_node) = child {
                        let child_bounds = node_bounds.octant(i as u8);
                        Self::query_rec(child_node, &child_bounds, query_bounds, results);
                    }
                }
            }
        }
    }

    pub fn nearest(&self, point: &Point3) -> Option<&OctreeEntry<T>> {
        if self.is_empty() {
            return None;
        }
        let mut best: Option<(&OctreeEntry<T>, f64)> = None;
        Self::nearest_rec(&self.root, &self.bounds, point, &mut best);
        best.map(|(entry, _)| entry)
    }

    fn nearest_rec<'a>(
        node: &'a OctreeNode<T>,
        node_bounds: &BoundingBox,
        point: &Point3,
        best: &mut Option<(&'a OctreeEntry<T>, f64)>,
    ) {
        if let Some((_, best_dist_sq)) = best {
            let nearest_in_box = Self::nearest_point_in_box(point, node_bounds);
            if nearest_in_box.distance_sq(point) >= *best_dist_sq {
                return;
            }
        }
        match node {
            OctreeNode::Leaf { entries } => {
                for entry in entries {
                    let dist_sq = entry.point.distance_sq(point);
                    match best {
                        None => *best = Some((entry, dist_sq)),
                        Some((_, bds)) if dist_sq < *bds => *best = Some((entry, dist_sq)),
                        _ => {}
                    }
                }
            }
            OctreeNode::Interior { children } => {
                let mut order: [(f64, usize); 8] = [(0.0f64, 0usize); 8];
                for (i, child) in children.iter().enumerate() {
                    if child.is_some() {
                        let cb = node_bounds.octant(i as u8);
                        let np = Self::nearest_point_in_box(point, &cb);
                        order[i] = (np.distance_sq(point), i);
                    } else {
                        order[i] = (f64::INFINITY, i);
                    }
                }
                order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                for (dist_sq, i) in order {
                    if dist_sq.is_infinite() {
                        continue;
                    }
                    if let Some((_, bds)) = best {
                        if dist_sq >= *bds {
                            continue;
                        }
                    }
                    if let Some(child_node) = &children[i] {
                        let child_bounds = node_bounds.octant(i as u8);
                        Self::nearest_rec(child_node, &child_bounds, point, best);
                    }
                }
            }
        }
    }

    fn nearest_point_in_box(point: &Point3, bb: &BoundingBox) -> Point3 {
        Point3::new(
            point.x.clamp(bb.min.x, bb.max.x),
            point.y.clamp(bb.min.y, bb.max.y),
            point.z.clamp(bb.min.z, bb.max.z),
        )
    }

    pub fn root(&self) -> &OctreeNode<T> {
        &self.root
    }

    pub fn entries(&self) -> Vec<&OctreeEntry<T>> {
        let mut result = Vec::with_capacity(self.len);
        Self::collect_entries(&self.root, &mut result);
        result
    }

    fn collect_entries<'a>(node: &'a OctreeNode<T>, result: &mut Vec<&'a OctreeEntry<T>>) {
        match node {
            OctreeNode::Leaf { entries } => result.extend(entries.iter()),
            OctreeNode::Interior { children } => {
                for c in children.iter().flatten() {
                    Self::collect_entries(c, result);
                }
            }
        }
    }
}
