use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationError {
    InvalidPlan(String),
    NoPlanFound,
    CostOverflow,
    MissingStatistics(String),
    UnsupportedOperation(String),
}

impl fmt::Display for OptimizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(msg) => write!(f, "invalid plan: {msg}"),
            Self::NoPlanFound => write!(f, "no valid plan found"),
            Self::CostOverflow => write!(f, "cost estimate overflowed"),
            Self::MissingStatistics(table) => write!(f, "missing statistics for: {table}"),
            Self::UnsupportedOperation(op) => write!(f, "unsupported operation: {op}"),
        }
    }
}

impl std::error::Error for OptimizationError {}

pub type OptimizationResult<T> = Result<T, OptimizationError>;
