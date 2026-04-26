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
mod traits;
mod tree;

#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "proptest"))]
mod proptests;

pub use tree::EulerTourTree;
pub use traits::{EttAggregate, EttError, Monoid};
