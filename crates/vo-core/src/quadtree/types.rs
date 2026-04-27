use thiserror::Error;

pub type PointValue = String;

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

    pub(crate) fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x1 && x < self.x2 && y >= self.y1 && y < self.y2
    }

    fn center(&self) -> (f64, f64) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }

    pub(crate) fn intersects(&self, other: &AABB) -> bool {
        self.x1 < other.x2 && self.x2 > other.x1 && self.y1 < other.y2 && self.y2 > other.y1
    }

    pub(crate) fn subdivide(&self) -> [AABB; 4] {
        let (mx, my) = self.center();
        [
            AABB::new(self.x1, self.y1, mx, my),
            AABB::new(mx, self.y1, self.x2, my),
            AABB::new(self.x1, my, mx, self.y2),
            AABB::new(mx, my, self.x2, self.y2),
        ]
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
    OutOfBounds { x: f64, y: f64, bounds: AABB },

    #[error("max depth {max_depth} exceeded at ({x}, {y})")]
    MaxDepthExceeded { x: f64, y: f64, max_depth: usize },

    #[error("cannot subdivide: child bounds would be degenerate at depth {depth}")]
    DegenerateSubdivision { depth: usize },
}

impl QuadtreeError {
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, QuadtreeError::MaxDepthExceeded { .. })
    }
}
