//! Point quadtree for 2D spatial indexing.
//!
//! Provides an in-memory quadtree supporting:
//! - Point insertion and removal
//! - Axis-aligned bounding box (AABB) range queries
//! - Configurable capacity and max depth

mod ops;
mod types;

pub use types::{Point, PointValue, QuadtreeError, AABB};

use ops::{count_node, insert_node, query_node, Node};

pub struct Quadtree {
    bounds: AABB,
    capacity: usize,
    max_depth: usize,
    root: Node,
}

impl Quadtree {
    pub fn new(bounds: AABB, capacity: usize, max_depth: usize) -> Self {
        Self {
            bounds,
            capacity,
            max_depth,
            root: Node::Leaf { points: Vec::new() },
        }
    }

    pub fn insert(&mut self, point: Point) -> Result<(), QuadtreeError> {
        if !self.bounds.contains_point(point.x, point.y) {
            return Err(QuadtreeError::OutOfBounds {
                x: point.x,
                y: point.y,
                bounds: self.bounds,
            });
        }
        let bounds = self.bounds;
        let capacity = self.capacity;
        let max_depth = self.max_depth;
        insert_node(&mut self.root, &bounds, point, 0, capacity, max_depth)
    }

    pub fn query(&self, region: AABB) -> Vec<Point> {
        let mut result = Vec::new();
        query_node(&self.root, &self.bounds, &region, &mut result);
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
    fn quadtree_insert_single_point_and_query_it() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 4, 8);
        qt.insert(Point::new(10.0, 20.0, "event-1")).unwrap();
        let found = qt.query(AABB::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].value, "event-1");
    }

    #[test]
    fn quadtree_insert_rejects_out_of_bounds() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 4, 8);
        let result = qt.insert(Point::new(200.0, 300.0, "bad"));
        assert!(result.is_err());
        match result {
            Err(QuadtreeError::OutOfBounds { x, y, .. }) => {
                assert_eq!(x, 200.0);
                assert_eq!(y, 300.0);
            }
            _ => panic!("expected OutOfBounds error"),
        }
    }

    #[test]
    fn quadtree_query_empty_region_returns_nothing() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 4, 8);
        qt.insert(Point::new(10.0, 10.0, "a")).unwrap();
        let found = qt.query(AABB::new(50.0, 50.0, 100.0, 100.0));
        assert!(found.is_empty());
    }

    #[test]
    fn quadtree_subdivides_on_capacity() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 2, 8);
        qt.insert(Point::new(10.0, 10.0, "a")).unwrap();
        qt.insert(Point::new(80.0, 80.0, "b")).unwrap();
        qt.insert(Point::new(15.0, 15.0, "c")).unwrap();
        assert_eq!(qt.len(), 3);
        let found = qt.query(AABB::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn quadtree_len_and_is_empty() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 4, 8);
        assert!(qt.is_empty());
        qt.insert(Point::new(10.0, 10.0, "a")).unwrap();
        assert_eq!(qt.len(), 1);
        assert!(!qt.is_empty());
    }

    #[test]
    fn quadtree_query_multiple_results() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 4, 8);
        qt.insert(Point::new(10.0, 10.0, "a")).unwrap();
        qt.insert(Point::new(20.0, 20.0, "b")).unwrap();
        qt.insert(Point::new(80.0, 80.0, "c")).unwrap();
        let found = qt.query(AABB::new(0.0, 0.0, 50.0, 50.0));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn quadtree_respects_max_depth() {
        let bounds = AABB::new(0.0, 0.0, 100.0, 100.0);
        let mut qt = Quadtree::new(bounds, 1, 1);
        qt.insert(Point::new(1.0, 1.0, "a")).unwrap();
        qt.insert(Point::new(2.0, 2.0, "b")).unwrap();
        qt.insert(Point::new(3.0, 3.0, "c")).unwrap();
        assert_eq!(qt.len(), 3);
    }

    #[test]
    fn error_is_recoverable_for_max_depth() {
        let err = QuadtreeError::MaxDepthExceeded {
            x: 1.0,
            y: 2.0,
            max_depth: 8,
        };
        assert!(err.is_recoverable());
    }

    #[test]
    fn error_is_not_recoverable_for_out_of_bounds() {
        let err = QuadtreeError::OutOfBounds {
            x: 1.0,
            y: 2.0,
            bounds: AABB::new(0.0, 0.0, 10.0, 10.0),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn aabb_display_format() {
        let aabb = AABB::new(0.0, 0.0, 100.0, 100.0);
        let s = aabb.to_string();
        assert!(s.contains("AABB"));
    }
}
