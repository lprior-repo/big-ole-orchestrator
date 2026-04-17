<<<<<<< HEAD
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingHeap<T: Ord + Clone> {
    _root: Option<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingHeapError {
    #[error("empty heap")]
    EmptyHeap,
}
=======
#![allow(dead_code)]

pub struct PairingHeap;
pub struct PairingHeapError;
>>>>>>> origin/polecat/guzzle-veloxide-4wc
