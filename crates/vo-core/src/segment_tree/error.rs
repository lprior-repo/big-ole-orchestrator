#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SegmentTreeError {
    #[error("empty data: SegmentTree requires at least one element")]
    EmptyData,
    #[error("invalid range: left ({left}) > right ({right})")]
    InvalidRange { left: usize, right: usize },
    #[error("index out of bounds: {index} >= len {len}")]
    IndexOutOfBounds { index: usize, len: usize },
    #[error("range out of bounds: right ({right}) > len {len}")]
    RangeOutOfBounds { right: usize, len: usize },
}
