//! QA-MANUAL: segment_tree error handling verification (ve-m05cr)

use vo_core::segment_tree::SegmentTree;

// ST-12: update panics on out-of-bounds index with descriptive message
#[test]
#[should_panic(expected = "update: index")]
fn segment_tree_update_out_of_bounds() {
    let data = vec![1i64, 2, 3];
    let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
    let _ = tree.update(3, 10);
}

// ST-13: try_update returns error for out-of-bounds index
#[test]
fn segment_tree_try_update_returns_error() {
    let data = vec![1i64, 2, 3];
    let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
    let result = tree.try_update(3, 10);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        vo_core::segment_tree::SegmentTreeError::IndexOutOfBounds { index: 3, len: 3 }
    );
}

// ST-14: try_update succeeds for valid index
#[test]
fn segment_tree_try_update_valid_index() {
    let data = vec![1i64, 2, 3];
    let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
    let result = tree.try_update(1, 10);
    assert!(result.is_ok());
    assert_eq!(tree.get(1), 10);
}

// ST-15: try_update rejects out-of-bounds range
#[test]
fn segment_tree_try_update_out_of_range() {
    let data = vec![1i64, 2, 3];
    let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
    let result = tree.try_update(5, 10);
    assert!(result.is_err());
}
