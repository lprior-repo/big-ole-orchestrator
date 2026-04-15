use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SearchError {
    #[error("empty query")]
    EmptyQuery,

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("workspace not found: {0:?}")]
    WorkspaceNotFound(String),
}
