use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SpscError {
    #[error("queue is full")]
    Full,
    #[error("queue is empty")]
    Empty,
}
