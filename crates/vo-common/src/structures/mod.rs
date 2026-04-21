//! Spatial and geometric data structures.

pub(crate) mod octree_internal;
pub mod octree;

pub use octree::{Bounds, Octree, Vec3};
