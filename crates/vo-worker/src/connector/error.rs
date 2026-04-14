//! Connector error classification (retryable vs terminal).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectorError {
    #[error("retryable connector error: {0}")]
    Retryable(String),
    #[error("terminal connector error: {0}")]
    Terminal(String),
    #[error("connector '{0}' does not support compensation")]
    CompensationNotSupported(String),
}

impl ConnectorError {
    pub fn retryable(msg: impl Into<String>) -> Self {
        Self::Retryable(msg.into())
    }

    pub fn terminal(msg: impl Into<String>) -> Self {
        Self::Terminal(msg.into())
    }

    pub fn compensation_not_supported(connector_type: &str) -> Self {
        Self::CompensationNotSupported(connector_type.to_string())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_) | Self::CompensationNotSupported(_))
    }
}
