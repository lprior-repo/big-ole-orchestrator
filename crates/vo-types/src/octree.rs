#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct OctreeError(pub String);

#[derive(Debug, Clone, Copy)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

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
}
