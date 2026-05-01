//! Type compatibility checking for DAG step connections.
//!
//! Validates that the output type of one step is compatible with the input
//! type of the next step in a workflow DAG.
//!
//! # Rules
//!
//! - Exact type match passes (e.g. `String` → `String`)
//! - `Vec<u8>` → `Vec<u8>` passes (binary data flow)
//! - `impl Into<T>` → `T` passes (trait compatibility)
//! - Mismatched types return `Err(TypeMismatch{expected, got})`
//!
//! # Examples
//!
//! ```
//! use vo_linter::type_checks::{check_type_compatibility, DataType};
//!
//! // Exact match passes
//! assert!(check_type_compatibility(
//!     &DataType::String, &DataType::String
//! ).is_ok());
//!
//! // Mismatch fails
//! let err = check_type_compatibility(
//!     &DataType::Vec(DataType::U8), &DataType::String
//! ).unwrap_err();
//! assert_eq!(err.got, DataType::Vec(DataType::U8));
//! assert_eq!(err.expected, DataType::String);
//! ```

use std::fmt;
use thiserror::Error;

/// Represents a Rust data type in the workflow type system.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataType {
    /// Unit type ()
    Unit,
    /// bool
    Bool,
    /// u8
    U8,
    /// u16
    U16,
    /// u32
    U32,
    /// u64
    U64,
    /// u128
    U128,
    /// i8
    I8,
    /// i16
    I16,
    /// i32
    I32,
    /// i64
    I64,
    /// i128
    I128,
    /// f32
    F32,
    /// f64
    F64,
    /// String
    String,
    /// Vec<T> where T is a DataType
    Vec(Box<DataType>),
    /// Option<T> where T is a DataType
    Option(Box<DataType>),
    /// impl Into<T> — accepts any value that can be converted to T
    Into(Box<DataType>),
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Unit => write!(f, "()"),
            DataType::Bool => write!(f, "bool"),
            DataType::U8 => write!(f, "u8"),
            DataType::U16 => write!(f, "u16"),
            DataType::U32 => write!(f, "u32"),
            DataType::U64 => write!(f, "u64"),
            DataType::U128 => write!(f, "u128"),
            DataType::I8 => write!(f, "i8"),
            DataType::I16 => write!(f, "i16"),
            DataType::I32 => write!(f, "i32"),
            DataType::I64 => write!(f, "i64"),
            DataType::I128 => write!(f, "i128"),
            DataType::F32 => write!(f, "f32"),
            DataType::F64 => write!(f, "f64"),
            DataType::String => write!(f, "String"),
            DataType::Vec(inner) => write!(f, "Vec<{}>", inner),
            DataType::Option(inner) => write!(f, "Option<{}>", inner),
            DataType::Into(inner) => write!(f, "impl Into<{}>", inner),
        }
    }
}

/// Error returned when two types are incompatible for a DAG edge.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("type mismatch: expected {expected}, got {got}")]
pub struct TypeMismatch {
    /// The type expected by the receiving step (input type).
    pub expected: DataType,
    /// The type produced by the sending step (output type).
    pub got: DataType,
}

/// Check if an output type is compatible with an input type for a DAG edge.
///
/// Compatibility rules:
/// - Exact type match: passes
/// - `impl Into<T>` output: passes if the inner type matches the input type
/// - `Option<T>` output: passes if `T` matches the input type (unwrap compatibility)
/// - `Vec<T>` where T is the same: passes
///
/// # Errors
///
/// Returns `Err(TypeMismatch)` when the output type cannot satisfy the input type.
pub fn check_type_compatibility(
    output: &DataType,
    input: &DataType,
) -> Result<(), TypeMismatch> {
    match (output, input) {
        // Exact matches always pass
        (a, b) if a == b => Ok(()),

        // impl Into<T> can produce T — check if inner matches input
        (DataType::Into(inner), _) => {
            if inner.as_ref() == input {
                Ok(())
            } else {
                Err(TypeMismatch {
                    expected: input.clone(),
                    got: output.clone(),
                })
            }
        }

        // Option<T> → T: unwrap compatibility
        (DataType::Option(inner), _) => {
            if inner.as_ref() == input {
                Ok(())
            } else {
                Err(TypeMismatch {
                    expected: input.clone(),
                    got: output.clone(),
                })
            }
        }

        // Everything else is a mismatch
        (out, in_) => Err(TypeMismatch {
            expected: in_.clone(),
            got: out.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_string_match_passes() {
        assert!(check_type_compatibility(&DataType::String, &DataType::String).is_ok());
    }

    #[test]
    fn exact_u8_match_passes() {
        assert!(check_type_compatibility(&DataType::U8, &DataType::U8).is_ok());
    }

    #[test]
    fn exact_vec_u8_match_passes() {
        assert!(check_type_compatibility(
            &DataType::Vec(Box::new(DataType::U8)),
            &DataType::Vec(Box::new(DataType::U8))
        )
        .is_ok());
    }

    #[test]
    fn vec_u8_to_string_returns_type_mismatch() {
        let err = check_type_compatibility(
            &DataType::Vec(Box::new(DataType::U8)),
            &DataType::String,
        )
        .unwrap_err();
        assert_eq!(err.expected, DataType::String);
        assert_eq!(err.got, DataType::Vec(Box::new(DataType::U8)));
    }

    #[test]
    fn string_to_vec_u8_returns_type_mismatch() {
        let err = check_type_compatibility(
            &DataType::String,
            &DataType::Vec(Box::new(DataType::U8)),
        )
        .unwrap_err();
        assert_eq!(err.expected, DataType::Vec(Box::new(DataType::U8)));
        assert_eq!(err.got, DataType::String);
    }

    #[test]
    fn impl_into_string_to_string_passes() {
        assert!(check_type_compatibility(
            &DataType::Into(Box::new(DataType::String)),
            &DataType::String
        )
        .is_ok());
    }

    #[test]
    fn impl_into_u32_to_u32_passes() {
        assert!(check_type_compatibility(
            &DataType::Into(Box::new(DataType::U32)),
            &DataType::U32
        )
        .is_ok());
    }

    #[test]
    fn impl_into_string_to_u32_fails() {
        let err = check_type_compatibility(
            &DataType::Into(Box::new(DataType::String)),
            &DataType::U32,
        )
        .unwrap_err();
        assert_eq!(err.expected, DataType::U32);
        assert_eq!(err.got, DataType::Into(Box::new(DataType::String)));
    }

    #[test]
    fn option_string_to_string_passes() {
        assert!(check_type_compatibility(
            &DataType::Option(Box::new(DataType::String)),
            &DataType::String
        )
        .is_ok());
    }

    #[test]
    fn option_string_to_u32_fails() {
        let err = check_type_compatibility(
            &DataType::Option(Box::new(DataType::String)),
            &DataType::U32,
        )
        .unwrap_err();
        assert_eq!(err.expected, DataType::U32);
        assert_eq!(err.got, DataType::Option(Box::new(DataType::String)));
    }

    #[test]
    fn bool_to_string_fails() {
        let err = check_type_compatibility(&DataType::Bool, &DataType::String).unwrap_err();
        assert_eq!(err.expected, DataType::String);
        assert_eq!(err.got, DataType::Bool);
    }

    #[test]
    fn f64_to_u32_fails() {
        let err = check_type_compatibility(&DataType::F64, &DataType::U32).unwrap_err();
        assert_eq!(err.expected, DataType::U32);
        assert_eq!(err.got, DataType::F64);
    }

    #[test]
    fn type_mismatch_display() {
        let err = TypeMismatch {
            expected: DataType::String,
            got: DataType::Vec(Box::new(DataType::U8)),
        };
        let msg = err.to_string();
        assert!(msg.contains("expected"));
        assert!(msg.contains("got"));
        assert!(msg.contains("String"));
        assert!(msg.contains("Vec<u8>"));
    }

    #[test]
    fn data_type_display_formatting() {
        assert_eq!(format!("{}", DataType::Unit), "()");
        assert_eq!(format!("{}", DataType::Bool), "bool");
        assert_eq!(format!("{}", DataType::String), "String");
        assert_eq!(format!("{}", DataType::Vec(Box::new(DataType::U8))), "Vec<u8>");
        assert_eq!(
            format!("{}", DataType::Into(Box::new(DataType::String))),
            "impl Into<String>"
        );
        assert_eq!(
            format!("{}", DataType::Option(Box::new(DataType::U32))),
            "Option<u32>"
        );
    }

    #[test]
    fn data_type_clone_equality() {
        let a = DataType::Vec(Box::new(DataType::U8));
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn data_type_hash_consistent() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(DataType::String);
        set.insert(DataType::String);
        set.insert(DataType::Vec(Box::new(DataType::U8)));
        assert_eq!(set.len(), 2);
    }
}
