//! Spatial and geometric data structures.

pub mod octree;
pub(crate) mod octree_internal;

pub use octree::{Bounds, Octree, Vec3};
