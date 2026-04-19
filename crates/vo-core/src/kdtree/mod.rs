//! K-dimensional tree for spatial indexing of points.
//!
//! Provides an in-memory KD-tree supporting:
//! - Point insertion and removal
//! - Axis-aligned bounding box (AABB) range queries
//! - Configurable capacity and max depth
//! - Generic over number of dimensions

use std::fmt::Display;
use thiserror::Error;

pub type PointValue = String;

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    coords: Vec<f64>,
    value: PointValue,
}

impl Point {
    pub fn new(coords: &[f64], value: impl Into<String>) -> Self {
        Self {
            coords: coords.to_vec(),
            value: value.into(),
        }
    }

    pub fn coordinates(&self) -> &[f64] {
        &self.coords
    }

    pub fn get_coord(&self, dim: usize) -> f64 {
        self.coords[dim]
    }

    pub fn dim(&self) -> usize {
        self.coords.len()
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Point({})",
            self.coords
                .iter()
                .map(|c| format!("{:.1}", c))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AABB {
    mins: Vec<f64>,
    maxs: Vec<f64>,
}

impl AABB {
    pub fn new(mins: &[f64], maxs: &[f64]) -> Self {
        Self {
            mins: mins.to_vec(),
            maxs: maxs.to_vec(),
        }
    }

    pub fn from_point(point: &Point, margin: f64) -> Self {
        let dim = point.dim();
        let mins: Vec<f64> = point.coordinates().iter().map(|c| c - margin).collect();
        let maxs: Vec<f64> = point.coordinates().iter().map(|c| c + margin).collect();
        Self { mins, maxs }
    }

    pub fn dimension(&self) -> usize {
        self.mins.len()
    }

    fn contains_point(&self, point: &Point) -> bool {
        if point.dim() != self.dimension() {
            return false;
        }
        for i in 0..self.dimension() {
            if point.coords[i] < self.mins[i] || point.coords[i] >= self.maxs[i] {
                return false;
            }
        }
        true
    }

    fn intersects(&self, other: &AABB) -> bool {
        if self.dimension() != other.dimension() {
            return false;
        }
        for i in 0..self.dimension() {
            if self.mins[i] >= other.maxs[i] || self.maxs[i] <= other.mins[i] {
                return false;
            }
        }
        true
    }

    fn split(&self, dim: usize, split_val: f64) -> (AABB, AABB) {
        let mut left_maxs = self.maxs.clone();
        left_maxs[dim] = split_val;
        let mut right_mins = self.mins.clone();
        right_mins[dim] = split_val;

        (
            AABB::new(&self.mins, &left_maxs),
            AABB::new(&right_mins, &self.maxs),
        )
    }
}

impl Display for AABB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AABB[({})-({})]",
            self.mins
                .iter()
                .map(|c| format!("{:.1}", c))
                .collect::<Vec<_>>()
                .join(", "),
            self.maxs
                .iter()
                .map(|c| format!("{:.1}", c))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum KdtreeError {
    #[error("point {point} is outside kdtree bounds {bounds}")]
    OutOfBounds { point: Point, bounds: AABB },

    #[error("max depth {max_depth} exceeded at point {point}")]
    MaxDepthExceeded { point: Point, max_depth: usize },

    #[error("dimension mismatch: point has {point_dim} dims, bounds has {bounds_dim} dims")]
    DimensionMismatch { point_dim: usize, bounds_dim: usize },

    #[error("cannot subdivide: split would be degenerate at dimension {dim}")]
    DegenerateSubdivision { dim: usize },
}

impl KdtreeError {
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, KdtreeError::MaxDepthExceeded { .. })
    }
}

enum Node {
    Leaf {
        points: Vec<Point>,
    },
    Branch {
        dim: usize,
        split_val: f64,
        left: Box<Node>,
        right: Box<Node>,
    },
}

pub struct Kdtree {
    bounds: AABB,
    capacity: usize,
    max_depth: usize,
    dim: usize,
    root: Node,
}

impl Kdtree {
    pub fn new(bounds: AABB, capacity: usize, max_depth: usize) -> Self {
        let dim = bounds.dimension();
        Self {
            bounds,
            capacity,
            max_depth,
            dim,
            root: Node::Leaf { points: Vec::new() },
        }
    }

    pub fn insert(&mut self, point: Point) -> Result<(), KdtreeError> {
        if point.dim() != self.dim {
            return Err(KdtreeError::DimensionMismatch {
                point_dim: point.dim(),
                bounds_dim: self.dim,
            });
        }
        if !self.bounds.contains_point(&point) {
            return Err(KdtreeError::OutOfBounds {
                point,
                bounds: self.bounds.clone(),
            });
        }
        let bounds = self.bounds.clone();
        let capacity = self.capacity;
        let max_depth = self.max_depth;
        insert_node(&mut self.root, &bounds, point, 0, capacity, max_depth)
    }

    pub fn query(&self, region: &AABB) -> Vec<Point> {
        let mut result = Vec::new();
        query_node(&self.root, &self.bounds, region, &mut result);
        result
    }

    pub fn len(&self) -> usize {
        count_node(&self.root)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn insert_node(
    node: &mut Node,
    bounds: &AABB,
    point: Point,
    depth: usize,
    capacity: usize,
    max_depth: usize,
) -> Result<(), KdtreeError> {
    let splitting_dim = depth % bounds.dimension();

    match node {
        Node::Leaf { points } => {
            if points.len() < capacity || depth >= max_depth {
                points.push(point);
                Ok(())
            } else {
                let existing: Vec<Point> = std::mem::take(points);
                let mid_val = existing
                    .iter()
                    .map(|p| p.get_coord(splitting_dim))
                    .sum::<f64>()
                    / existing.len() as f64;

                let mut children = Box::new((
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                ));

                for p in existing {
                    let child_idx = if p.get_coord(splitting_dim) < mid_val {
                        0
                    } else {
                        1
                    };
                    let (left_bounds, right_bounds) = bounds.split(splitting_dim, mid_val);
                    let target_bounds = if child_idx == 0 {
                        &left_bounds
                    } else {
                        &right_bounds
                    };
                    insert_node(
                        &mut children[child_idx],
                        target_bounds,
                        p,
                        depth + 1,
                        capacity,
                        max_depth,
                    )?;
                }

                let (left_bounds, right_bounds) = bounds.split(splitting_dim, mid_val);
                let child_idx = if point.get_coord(splitting_dim) < mid_val {
                    0
                } else {
                    1
                };
                let target_bounds = if child_idx == 0 {
                    &left_bounds
                } else {
                    &right_bounds
                };
                insert_node(
                    &mut children[child_idx],
                    target_bounds,
                    point,
                    depth + 1,
                    capacity,
                    max_depth,
                )?;

                *node = Node::Branch {
                    dim: splitting_dim,
                    split_val: mid_val,
                    left: children.0,
                    right: children.1,
                };
                Ok(())
            }
        }
        Node::Branch {
            dim,
            split_val,
            left,
            right,
        } => {
            let (left_bounds, right_bounds) = bounds.split(*dim, *split_val);
            if point.get_coord(*dim) < *split_val {
                insert_node(left, &left_bounds, point, depth + 1, capacity, max_depth)
            } else {
                insert_node(right, &right_bounds, point, depth + 1, capacity, max_depth)
            }
        }
    }
}

fn query_node(node: &Node, bounds: &AABB, region: &AABB, result: &mut Vec<Point>) {
    if !bounds.intersects(region) {
        return;
    }

    match node {
        Node::Leaf { points } => {
            for p in points {
                if region.contains_point(p) {
                    result.push(p.clone());
                }
            }
        }
        Node::Branch {
            dim,
            split_val,
            left,
            right,
        } => {
            let (left_bounds, right_bounds) = bounds.split(*dim, *split_val);
            query_node(left, &left_bounds, region, result);
            query_node(right, &right_bounds, region, result);
        }
    }
}

fn count_node(node: &Node) -> usize {
    match node {
        Node::Leaf { points } => points.len(),
        Node::Branch { left, right, .. } => count_node(left) + count_node(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kdtree_insert_single_point_and_query_it() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        tree.insert(Point::new(&[10.0, 20.0], "event-1")).unwrap();

        let query_region = AABB::new(&[0.0, 0.0], &[50.0, 50.0]);
        let found = tree.query(&query_region);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].value, "event-1");
    }

    #[test]
    fn kdtree_insert_rejects_out_of_bounds() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        let result = tree.insert(Point::new(&[200.0, 300.0], "bad"));
        assert!(result.is_err());
        match result {
            Err(KdtreeError::OutOfBounds { .. }) => {}
            _ => panic!("expected OutOfBounds error"),
        }
    }

    #[test]
    fn kdtree_insert_rejects_dimension_mismatch() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        let result = tree.insert(Point::new(&[10.0, 20.0, 30.0], "bad"));
        assert!(result.is_err());
        match result {
            Err(KdtreeError::DimensionMismatch { .. }) => {}
            _ => panic!("expected DimensionMismatch error"),
        }
    }

    #[test]
    fn kdtree_query_empty_region_returns_nothing() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        tree.insert(Point::new(&[10.0, 10.0], "a")).unwrap();

        let query_region = AABB::new(&[50.0, 50.0], &[100.0, 100.0]);
        let found = tree.query(&query_region);
        assert!(found.is_empty());
    }

    #[test]
    fn kdtree_subdivides_on_capacity() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 2, 8);
        tree.insert(Point::new(&[10.0, 10.0], "a")).unwrap();
        tree.insert(Point::new(&[80.0, 80.0], "b")).unwrap();
        tree.insert(Point::new(&[15.0, 15.0], "c")).unwrap();
        assert_eq!(tree.len(), 3);

        let query_region = AABB::new(&[0.0, 0.0], &[50.0, 50.0]);
        let found = tree.query(&query_region);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn kdtree_len_and_is_empty() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let tree = Kdtree::new(bounds, 4, 8);
        assert!(tree.is_empty());
    }

    #[test]
    fn kdtree_query_multiple_results() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        tree.insert(Point::new(&[10.0, 10.0], "a")).unwrap();
        tree.insert(Point::new(&[20.0, 20.0], "b")).unwrap();
        tree.insert(Point::new(&[80.0, 80.0], "c")).unwrap();

        let query_region = AABB::new(&[0.0, 0.0], &[50.0, 50.0]);
        let found = tree.query(&query_region);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn kdtree_respects_max_depth() {
        let bounds = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 1, 2);
        tree.insert(Point::new(&[10.0, 10.0], "a")).unwrap();
        tree.insert(Point::new(&[20.0, 20.0], "b")).unwrap();
        tree.insert(Point::new(&[30.0, 30.0], "c")).unwrap();
        assert_eq!(tree.len(), 3);
    }

    #[test]
    fn kdtree_3d_point_operations() {
        let bounds = AABB::new(&[0.0, 0.0, 0.0], &[100.0, 100.0, 100.0]);
        let mut tree = Kdtree::new(bounds, 4, 8);
        tree.insert(Point::new(&[10.0, 20.0, 30.0], "3d-point"))
            .unwrap();

        let query_region = AABB::new(&[0.0, 0.0, 0.0], &[50.0, 50.0, 50.0]);
        let found = tree.query(&query_region);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].get_coord(0), 10.0);
        assert_eq!(found[0].get_coord(1), 20.0);
        assert_eq!(found[0].get_coord(2), 30.0);
    }

    #[test]
    fn kdtree_error_is_recoverable_for_max_depth() {
        let err = KdtreeError::MaxDepthExceeded {
            point: Point::new(&[1.0, 2.0], "test"),
            max_depth: 8,
        };
        assert!(err.is_recoverable());
    }

    #[test]
    fn kdtree_error_is_not_recoverable_for_out_of_bounds() {
        let err = KdtreeError::OutOfBounds {
            point: Point::new(&[1.0, 2.0], "test"),
            bounds: AABB::new(&[0.0, 0.0], &[10.0, 10.0]),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn kdtree_aabb_display_format() {
        let aabb = AABB::new(&[0.0, 0.0], &[100.0, 100.0]);
        let s = aabb.to_string();
        assert!(s.contains("AABB"));
    }

    #[test]
    fn kdtree_point_display_format() {
        let point = Point::new(&[1.0, 2.0, 3.0], "test");
        let s = point.to_string();
        assert!(s.contains("Point"));
        assert!(s.contains("1.0"));
    }
}
