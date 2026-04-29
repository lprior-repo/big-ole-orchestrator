use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval<T: Ord, V> {
    pub start: T,
    pub end: T,
    pub value: V,
}

impl<T: Ord, V> Interval<T, V> {
    pub fn new(start: T, end: T, value: V) -> Self {
        Self { start, end, value }
    }

    pub fn contains_point(&self, point: &T) -> bool {
        &self.start <= point && point < &self.end
    }

    pub fn overlaps_interval(&self, other: &Interval<T, V>) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn overlaps_point(&self, point: &T) -> bool {
        self.contains_point(point)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalNode<T: Ord, V> {
    pub interval: Interval<T, V>,
    pub max_end: T,
    pub left: Option<Box<IntervalNode<T, V>>>,
    pub right: Option<Box<IntervalNode<T, V>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntervalTreeError {
    #[error("tree is empty")]
    EmptyTree,

    #[error("interval not found")]
    NotFound,

    #[error("invalid interval: start ({start}) >= end ({end})")]
    InvalidInterval { start: T, end: T },
}

impl<T: Ord, V> IntervalTreeError {
    pub const fn is_recoverable(&self) -> bool {
        false
    }
}
