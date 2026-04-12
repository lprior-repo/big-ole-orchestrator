//! Rope — arbitrary measurables stored in a Rope data structure.
//!
//! A rope is a self-balancing tree data structure for storing sequences of elements
//! where each element has an arbitrary "measure". Ropes are particularly efficient
//! for text editing operations (insert, delete, split) as most operations complete
//! in O(log n) time.
//!
//! Based on the AnyRope crate.

use std::fmt;

pub use any_rope::Measurable;
pub use any_rope::Rope;
pub use any_rope::RopeBuilder;
pub use any_rope::RopeSlice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RopeError {
    InvalidChunk,
    InvalidIndex,
    BuildError(String),
}

impl fmt::Display for RopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RopeError::InvalidChunk => write!(f, "invalid chunk"),
            RopeError::InvalidIndex => write!(f, "invalid index"),
            RopeError::BuildError(s) => write!(f, "rope build error: {s}"),
        }
    }
}

impl std::error::Error for RopeError {}

pub type Result<T> = std::result::Result<T, RopeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Char(char);

    impl Measurable for Char {
        type Measure = usize;
        fn measure(&self) -> Self::Measure {
            1
        }
    }

    #[test]
    fn error_display() {
        assert_eq!(RopeError::InvalidChunk.to_string(), "invalid chunk");
        assert_eq!(RopeError::InvalidIndex.to_string(), "invalid index");
        assert_eq!(
            RopeError::BuildError("test".to_string()).to_string(),
            "rope build error: test"
        );
    }

    #[test]
    fn error_eq() {
        assert_eq!(RopeError::InvalidChunk, RopeError::InvalidChunk);
        assert_eq!(RopeError::InvalidIndex, RopeError::InvalidIndex);
        assert_ne!(RopeError::InvalidChunk, RopeError::InvalidIndex);
    }

    #[test]
    fn basic_rope_operations() {
        let rope = Rope::<Char>::from_slice(&[Char('h'), Char('i')]);
        assert_eq!(rope.len(), 2);
    }

    #[test]
    fn char_measurable() {
        let c = Char('x');
        assert_eq!(c.measure(), 1);
    }
}
