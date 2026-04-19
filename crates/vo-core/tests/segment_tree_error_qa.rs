//! QA-MANUAL: segment_tree error handling verification (ve-m05cr)

use vo_core::segment_tree::SegmentTree;

// ST-12: update panics on out-of-bounds index with descriptive message
#[test]
#[should_panic(expected = "update: index")]
fn segment_tree_update_out_of_bounds() {
    let data = vec![1i64, 2, 3];
    let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
    tree.update(3, 10);
}
