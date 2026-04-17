<<<<<<< HEAD
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clique {
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliqueTree {
    _cliques: Vec<Clique>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliqueTreeError {
    #[error("empty tree")]
    EmptyTree,
    #[error("invalid graph")]
    InvalidGraph,
}
=======
#![allow(dead_code)]

pub struct Clique;
pub struct CliqueTree;
pub struct CliqueTreeError;
>>>>>>> origin/polecat/guzzle-veloxide-4wc
