//! EttAggregate trait and EttError type for the Euler Tour Tree.

pub use crate::monoid::Monoid;

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
