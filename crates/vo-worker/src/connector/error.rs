//! Connector error classification (retryable vs terminal).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(tag = "variant", content = "message")]
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

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal(_))
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
    fn test_is_terminal_for_retryable() {
        let err = ConnectorError::retryable("timeout");
        assert!(!err.is_terminal());
    }

    #[test]
    fn test_is_terminal_for_terminal() {
        let err = ConnectorError::terminal("auth failed");
        assert!(err.is_terminal());
    }

    #[test]
    fn test_is_terminal_for_compensation_not_supported() {
        let err = ConnectorError::compensation_not_supported("http");
        assert!(!err.is_terminal());
    }

    #[test]
    fn test_is_retryable_and_is_terminal_are_exclusive() {
        let retryable_err = ConnectorError::retryable("test");
        let terminal_err = ConnectorError::terminal("test");
        let compensation_err = ConnectorError::compensation_not_supported("http");

        assert!(retryable_err.is_retryable() ^ retryable_err.is_terminal());
        assert!(terminal_err.is_retryable() ^ terminal_err.is_terminal());
        assert!(compensation_err.is_retryable() ^ compensation_err.is_terminal());
    }

    #[test]
    fn test_serialization_retryable_roundtrip() {
        let err = ConnectorError::retryable("timeout");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
        assert!(deserialized.is_retryable());
    }

    #[test]
    fn test_serialization_terminal_roundtrip() {
        let err = ConnectorError::terminal("bad request");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
        assert!(deserialized.is_terminal());
    }

    #[test]
    fn test_serialization_compensation_not_supported_roundtrip() {
        let err = ConnectorError::compensation_not_supported("sqs");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
        assert!(deserialized.is_retryable());
        assert!(!deserialized.is_terminal());
    }

    #[test]
    fn test_serialization_preserves_message() {
        let original = ConnectorError::retryable("NATS connection timeout: network partition");
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original.to_string(), deserialized.to_string());
        assert_eq!(format!("{:?}", original), format!("{:?}", deserialized));
    }

    #[test]
    fn test_serialization_empty_message() {
        let err = ConnectorError::retryable("");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn test_serialization_long_message() {
        let long_msg = "a".repeat(10000);
        let err = ConnectorError::terminal(long_msg.clone());
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.to_string().contains(&long_msg));
    }

    #[test]
    fn test_serialization_special_characters() {
        let err = ConnectorError::retryable("unicode: \u{1F600} quotes: \"test\" backslash: \\");
        let serialized = serde_json::to_string(&err).unwrap();
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn test_serialization_compensation_not_supported_preserves_connector_type() {
        let err = ConnectorError::compensation_not_supported("http");
        let serialized = serde_json::to_string(&err).unwrap();
        assert!(serialized.contains("http"));
        let deserialized: ConnectorError = serde_json::from_str(&serialized).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn test_all_variants_serialize_to_object() {
        let retryable = ConnectorError::retryable("x");
        let terminal = ConnectorError::terminal("x");
        let compensation = ConnectorError::compensation_not_supported("x");

        let retryable_json = serde_json::to_value(&retryable).unwrap();
        let terminal_json = serde_json::to_value(&terminal).unwrap();
        let compensation_json = serde_json::to_value(&compensation).unwrap();

        assert!(retryable_json.is_object());
        assert!(terminal_json.is_object());
        assert!(compensation_json.is_object());
    }

    #[test]
    fn test_serialization_json_structure() {
        let err = ConnectorError::retryable("test message");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"variant\":\"Retryable\""));
        assert!(json.contains("\"message\":\"test message\""));
    }
}
