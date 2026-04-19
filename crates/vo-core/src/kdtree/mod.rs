//! K-dimensional tree for spatial indexing of points.
//!
//! Provides an in-memory KD-tree supporting:
//! - Point insertion and removal
//! - Axis-aligned bounding box (AABB) range queries
//! - Configurable capacity and max depth
//! - Generic over number of dimensions

mod ops;
mod types;

pub use types::{KdtreeError, Point, PointValue, AABB};

use ops::{count_node, insert_node, query_node};
use types::{KdtreeError, Point, AABB};

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
