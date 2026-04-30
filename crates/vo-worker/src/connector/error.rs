//! Connector error classification (retryable vs terminal).

/// Trait for error types that carry retryability semantics.
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectorError {
    #[error("retryable connector error: {0}")]
    Retryable(String),
    #[error("terminal connector error: {0}")]
    Terminal(String),
    #[error("connector '{0}' does not support compensation")]
    CompensationNotSupported(String),
    #[error("unexpected content type: {0}")]
    UnexpectedContentType(String),
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

    pub fn unexpected_content_type(content_type: impl Into<String>) -> Self {
        Self::UnexpectedContentType(content_type.into())
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_) | Self::CompensationNotSupported(_))
    }
}

impl Retryable for ConnectorError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_) | Self::CompensationNotSupported(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_error_is_retryable() {
        let err = ConnectorError::retryable("timeout");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_terminal_error_is_not_retryable() {
        let err = ConnectorError::terminal("bad request");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_compensation_not_supported_is_retryable() {
        let err = ConnectorError::compensation_not_supported("http");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_error_display() {
        let err = ConnectorError::retryable("connection timeout");
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_terminal_error_display() {
        let err = ConnectorError::terminal("authentication failed");
        assert!(err.to_string().contains("authentication"));
    }

    #[test]
    fn test_compensation_not_supported_display() {
        let err = ConnectorError::compensation_not_supported("sqs");
        assert!(err.to_string().contains("sqs"));
    }

    #[test]
    fn test_connector_error_debug() {
        let err = ConnectorError::retryable("test");
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Retryable"));
    }

    #[test]
    fn test_connector_error_terminal_debug() {
        let err = ConnectorError::terminal("test");
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Terminal"));
    }

    #[test]
    fn test_retryable_empty_message() {
        let err = ConnectorError::retryable("");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_terminal_empty_message() {
        let err = ConnectorError::terminal("");
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_unexpected_content_type_error() {
        let err = ConnectorError::unexpected_content_type("text/html");
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("text/html"));
    }

    #[test]
    fn test_unexpected_content_type_display() {
        let err = ConnectorError::unexpected_content_type("application/xml");
        assert!(err.to_string().contains("unexpected content type"));
        assert!(err.to_string().contains("application/xml"));
    }
}
