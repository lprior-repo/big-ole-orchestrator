<<<<<<< HEAD
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
=======
#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct OctreeError(pub String);

#[derive(Debug, Clone, Copy)]
>>>>>>> origin/polecat/guzzle-veloxide-4wc
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

<<<<<<< HEAD
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub center: Point3,
    pub half_size: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OctreeConfig {
    pub max_depth: u32,
    pub max_entries: usize,
}

impl Default for OctreeConfig {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_entries: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OctreeEntry<T: Clone> {
    pub position: Point3,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OctreeNode<T: Clone> {
    _entries: Vec<OctreeEntry<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Octree<T: Clone> {
    _root: Option<OctreeNode<T>>,
    _config: OctreeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OctreeError {
    #[error("position out of bounds")]
    OutOfBounds,
    #[error("max depth exceeded")]
    MaxDepthExceeded,
    #[error("empty tree")]
    EmptyTree,
=======
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub _p: (),
}

#[derive(Debug, Clone)]
pub struct OctreeNode {
    pub _p: (),
}

#[derive(Debug, Clone)]
pub struct OctreeEntry {
    pub _p: (),
}

#[derive(Debug, Clone)]
pub struct OctreeConfig {
    pub _p: (),
}

#[derive(Debug, Clone)]
pub struct Octree {
    pub _p: (),
>>>>>>> origin/polecat/guzzle-veloxide-4wc
}
