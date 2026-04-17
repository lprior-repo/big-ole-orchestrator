use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("invalid input item")]
    InvalidInputItem,
    #[error("parse failure")]
    ParseFailure,
    #[error("unsupported signature")]
    UnsupportedSignature,
    #[error("generic functions are not supported")]
    GenericFunction,
}
