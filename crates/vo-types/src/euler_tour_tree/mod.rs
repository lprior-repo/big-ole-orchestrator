//! Euler Tour Tree — dynamic forest data structure.
//!
//! Represents a tree as a sequence (Euler tour) where each node appears twice.
//! Uses a balanced BST (Treap) to maintain the tour sequence, enabling O(log n)
//! link, cut, and subtree aggregate operations. Good for subtree queries.
//!
//! # Differences from Link-Cut Tree
//!
//! - LCT: Good for PATH queries (aggregate along paths)
//! - ETT: Good for SUBTREE queries (aggregate within subtrees)
//!
//! Reference: Henzinger & King (1995), "Randomized dynamic graph algorithms"

mod node;
mod tree;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "proptest"))]
mod proptests;

pub use tree::EulerTourTree;

pub trait Monoid: Clone {
    fn identity() -> Self;
    fn combine(&self, other: &Self) -> Self;
}

impl Monoid for () {
    fn identity() -> Self {}
    fn combine(&self, _other: &Self) -> Self {}
}

impl Monoid for u64 {
    fn identity() -> Self {
        0
    }
    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}

pub trait EttAggregate<A: Monoid>: Clone {
    fn ett_aggregate(&self) -> A;
}

impl EttAggregate<()> for () {
    fn ett_aggregate(&self) {}
}

impl EttAggregate<u64> for u64 {
    fn ett_aggregate(&self) -> u64 {
        *self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EttError {
    #[error("invalid node index: {0}")]
    InvalidNode(usize),
    #[error("nodes {a} and {b} are already connected")]
    AlreadyConnected { a: usize, b: usize },
    #[error("nodes {a} and {b} are not connected")]
    NotConnected { a: usize, b: usize },
}
