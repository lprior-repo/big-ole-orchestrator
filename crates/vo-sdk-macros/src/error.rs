//! Error taxonomy for the `#[task]` procedural macro.
//!
//! # Error Categories
//!
//! | Category | Variants | Phase |
//! |----------|----------|-------|
//! | Input validation | `InvalidInputItem`, `UnsupportedSignature`, `GenericFunction` | Parsing |
//! | Attribute errors | `EmptyAttribute`, `TooManyAttributes`, `UnsupportedAttribute` | Attribute parsing |
//! | Code generation | `IdentParsingFailed`, `AsyncReturnTypeMismatch`, `GenerationFailed` | Generation |
//! | Parse errors | `ParseFailure` | Parsing |

use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum Error {
    #[error("invalid input item: expected a function, found a non-function item")]
    InvalidInputItem,
    #[error("parse failure: failed to parse token stream as valid Rust syntax")]
    ParseFailure,
    #[error("unsupported signature: task functions cannot accept arguments")]
    UnsupportedSignature,
    #[error("generic functions are not supported: task functions must be concrete")]
    GenericFunction,
    #[error("macro attribute is empty: remove the attribute or provide a valid value")]
    EmptyAttribute,
    #[error("too many macro attributes: found {count} attributes, max is 255")]
    TooManyAttributes { count: usize },
    #[error("failed to parse function identifier: '{ident}' is not a valid Rust identifier")]
    IdentParsingFailed { ident: String },
    #[error("async function '{ident}' cannot have a return type '{return_type}'; async tasks must return ()")]
    AsyncReturnTypeMismatch { ident: String, return_type: String },
    #[error("unsupported attribute: '{attribute}' is not a recognized #[task] attribute")]
    UnsupportedAttribute { attribute: String },
    #[error("code generation failed for function '{ident}'")]
    GenerationFailed { ident: String },
    #[error("invalid attribute value for '{ident}': {message}")]
    InvalidAttributeValue { ident: String, message: String },
    #[error("negative retries value: {value}")]
    NegativeRetries { value: i64 },
    #[error("unknown attribute: '{ident}'")]
    UnknownAttribute { ident: String },
}
