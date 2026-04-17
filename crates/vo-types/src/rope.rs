<<<<<<< HEAD
use serde::{Deserialize, Serialize};

pub trait Measurable {
    fn measure(&self) -> usize;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rope {
    _len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RopeBuilder {
    _chunk_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RopeSlice<'a> {
    _inner: &'a Rope,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RopeError {
    #[error("index out of bounds")]
    IndexOutOfBounds,
    #[error("empty rope")]
    EmptyRope,
}
=======
#![allow(dead_code)]

pub struct Rope;
pub struct RopeSlice;
pub struct RopeBuilder;
pub struct RopeError;

pub trait Measurable {}
>>>>>>> origin/polecat/guzzle-veloxide-4wc
