//! # vo-ds
//!
//! Generic data structures for Veloxide. Pure, reusable collections with no
//! domain coupling — trees, heaps, spatial indices, and utility types.
//!
//! ## Structures
//!
//! - **BTree** — B-tree with configurable order
//! - **BinomialHeap** — mergeable priority queue
//! - **Rope** — efficient text manipulation
//! - **EulerTourTree** — dynamic forest with subtree queries
//! - **LinkCutTree** — dynamic forest with path queries
//! - **RedBlackTree** — balanced binary search tree
//! - **FenwickTree** — binary indexed tree for prefix sums
//! - **SegmentTree / LazySegmentTree** — range query trees
//! - **IntervalTree** — augmented BST for interval overlap
//! - **Kdtree** — k-dimensional spatial index
//! - **Quadtree** — 2D spatial index
//! - **NonEmptyVec** — `Vec<T>` guaranteed non-empty

pub mod btree;
pub mod binomial_heap;
pub mod clique_tree;
pub mod euler_tour_tree;
pub mod fenwick;
pub mod interval_tree;
pub mod kdtree;
pub mod link_cut_tree;
pub mod non_empty_vec;
pub mod octree;
pub mod pairing_heap;
pub mod quadtree;
pub mod red_black_tree;
pub mod rope;
pub mod segment_tree;
