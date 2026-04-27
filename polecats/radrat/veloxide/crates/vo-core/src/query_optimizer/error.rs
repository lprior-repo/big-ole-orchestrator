#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OptimizationError {
    #[error("invalid plan: {0}")]
    InvalidPlan(String),
    #[error("no valid plan found")]
    NoPlanFound,
    #[error("cost estimate overflowed")]
    CostOverflow,
    #[error("missing statistics for: {0}")]
    MissingStatistics(String),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
}

pub type OptimizationResult<T> = Result<T, OptimizationError>;
