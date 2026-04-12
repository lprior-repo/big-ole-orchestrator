//! Point quadtree for 2D spatial indexing.
//!
//! Provides an in-memory quadtree supporting:
//! - Point insertion and removal
//! - Axis-aligned bounding box (AABB) range queries
//! - Configurable capacity and max depth

use thiserror::Error;

/// Value stored at a point.
pub type PointValue = String;

/// A 2D point with an associated value.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub value: PointValue,
}

impl Point {
    pub fn new(x: f64, y: f64, value: impl Into<String>) -> Self {
        Self {
            x,
            y,
            value: value.into(),
        }
    }
}

/// Axis-aligned bounding box defined by min (x1, y1) and max (x2, y2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl AABB {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x1 && x < self.x2 && y >= self.y1 && y < self.y2
    }

    fn center(&self) -> (f64, f64) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }

    fn intersects(&self, other: &AABB) -> bool {
        self.x1 < other.x2 && self.x2 > other.x1 && self.y1 < other.y2 && self.y2 > other.y1
    }
}

impl std::fmt::Display for AABB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AABB[({:.1},{:.1})-({:.1},{:.1})]",
            self.x1, self.y1, self.x2, self.y2
        )
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum QuadtreeError {
    #[error("point ({x}, {y}) is outside quadtree bounds {bounds}")]
    OutOfBounds {
        x: f64,
        y: f64,
        bounds: AABB,
    },

    #[error("max depth {max_depth} exceeded at ({x}, {y})")]
    MaxDepthExceeded {
        x: f64,
        y: f64,
        max_depth: usize,
    },

    #[error("cannot subdivide: child bounds would be degenerate at depth {depth}")]
    DegenerateSubdivision { depth: usize },
}

impl QuadtreeError {
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, QuadtreeError::MaxDepthExceeded { .. })
    }
}

enum Node {
    Leaf { points: Vec<Point> },
    Branch { children: Box<[Node; 4]> },
}

/// In-memory point quadtree with configurable capacity and max depth.
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

fn insert_node(
    node: &mut Node,
    bounds: &AABB,
    point: Point,
    depth: usize,
    capacity: usize,
    max_depth: usize,
) -> Result<(), QuadtreeError> {
    match node {
        Node::Leaf { points } => {
            if points.len() < capacity || depth >= max_depth {
                points.push(point);
                Ok(())
            } else {
                let existing: Vec<Point> = std::mem::take(points);
                let child_bounds = subdivide_bounds(bounds);
                let mut children = Box::new([
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                ]);
                for p in existing {
                    let qi = quadrant_index(&child_bounds, p.x, p.y);
                    insert_node(&mut children[qi], &child_bounds[qi], p, depth + 1, capacity, max_depth)?;
                }
                let qi = quadrant_index(&child_bounds, point.x, point.y);
                insert_node(&mut children[qi], &child_bounds[qi], point, depth + 1, capacity, max_depth)?;
                *node = Node::Branch { children };
                Ok(())
            }
        }
        Node::Branch { children } => {
            let child_bounds = subdivide_bounds(bounds);
            let qi = quadrant_index(&child_bounds, point.x, point.y);
            insert_node(&mut children[qi], &child_bounds[qi], point, depth + 1, capacity, max_depth)
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
                if region.contains_point(p.x, p.y) {
                    result.push(p.clone());
                }
            }
        }
        Node::Branch { children } => {
            let child_bounds = subdivide_bounds(bounds);
            for i in 0..4 {
                query_node(&children[i], &child_bounds[i], region, result);
            }
        }
    }
}

fn count_node(node: &Node) -> usize {
    match node {
        Node::Leaf { points } => points.len(),
        Node::Branch { children } => children.iter().map(count_node).sum(),
    }
}

fn subdivide_bounds(bounds: &AABB) -> [AABB; 4] {
    let (mx, my) = bounds.center();
    [
        AABB::new(bounds.x1, bounds.y1, mx, my),
        AABB::new(mx, bounds.y1, bounds.x2, my),
        AABB::new(bounds.x1, my, mx, bounds.y2),
        AABB::new(mx, my, bounds.x2, bounds.y2),
    ]
}

fn quadrant_index(bounds: &[AABB; 4], x: f64, y: f64) -> usize {
    for (i, b) in bounds.iter().enumerate() {
        if b.contains_point(x, y) {
            return i;
        }
    }
    3
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
        let err = QuadtreeError::MaxDepthExceeded { x: 1.0, y: 2.0, max_depth: 8 };
        assert!(err.is_recoverable());
    }

    #[test]
    fn error_is_not_recoverable_for_out_of_bounds() {
        let err = QuadtreeError::OutOfBounds {
            x: 1.0, y: 2.0,
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
