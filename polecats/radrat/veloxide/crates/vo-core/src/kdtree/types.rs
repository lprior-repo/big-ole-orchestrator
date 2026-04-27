use std::fmt::Display;

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

    pub(crate) fn contains_point(&self, point: &Point) -> bool {
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

    pub(crate) fn intersects(&self, other: &AABB) -> bool {
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

    pub(crate) fn split(&self, dim: usize, split_val: f64) -> (AABB, AABB) {
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

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
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
