use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("invalid input item")]
    InvalidInputItem,
    #[error("parse failure")]
    ParseFailure,
    #[error("unsupported signature")]
    UnsupportedSignature,
    #[error("macro attribute is empty")]
    EmptyAttribute,
    #[error("too many macro attributes (max 255)")]
    TooManyAttributes,
    #[error("failed to parse function identifier")]
    IdentParsingFailed,
    #[error("async functions cannot have a return type")]
    AsyncReturnTypeMismatch,
    #[error("unknown attribute: {0}")]
    UnknownAttribute(String),
    #[error("invalid attribute value for {0}: {1}")]
    InvalidAttributeValue(String, String),
    #[error("retries must be non-negative, got {0}")]
    NegativeRetries(i64),
}
