//! Segment tree for efficient range queries and updates.

mod error;
mod lazy;
mod tree;

pub use error::SegmentTreeError;
pub use lazy::LazySegmentTree;
pub use tree::SegmentTree;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_tree_try_from_slice_rejects_empty() {
        let result = SegmentTree::try_from_slice(&[], |a: &i64, b: &i64| a + b, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SegmentTreeError::EmptyData);
    }

    #[test]
    fn segment_tree_try_query_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::try_from_slice(&data, |a, b| a + b, 0).unwrap();
        let result = tree.try_query(0, 4);
        assert!(result.is_err());
    }

    #[test]
    fn segment_tree_try_get_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::try_from_slice(&data, |a, b| a + b, 0).unwrap();
        let result = tree.try_get(3);
        assert!(result.is_err());
    }

    #[test]
    fn segment_tree_query_full_range_sum() {
        let data = vec![1i64, 3, 5, 7, 9, 11];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(0, 6), 36);
    }

    #[test]
    fn segment_tree_point_update_changes_query() {
        let data = vec![1i64, 2, 3, 4, 5];
        let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        tree.update(2, 10);
        assert_eq!(tree.query(0, 5), 22);
    }

    #[test]
    fn segment_tree_range_query_partial() {
        let data = vec![1i64, 2, 3, 4, 5, 6, 7, 8];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(2, 5), 12);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn segment_tree_query_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        let _ = tree.query(0, 4);
    }

    #[test]
    fn segment_tree_single_element() {
        let data = vec![42i64];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(0, 1), 42);
    }

    #[test]
    fn segment_tree_identity_property() {
        let data = vec![5i64, 10, 15];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(1, 2), 10);
    }

    #[test]
    fn lazy_segment_tree_range_update_additive() {
        let data = vec![1i64, 2, 3, 4, 5];
        let mut tree = LazySegmentTree::from_slice(
            &data,
            |a, b| a + b,
            0,
            |val, upd, len| val + upd * len as i64,
            |old, new| old + new,
        );
        tree.update_range(1, 4, 10);
        assert_eq!(tree.query(0, 5), 45);
    }

    #[test]
    fn lazy_segment_tree_overlapping_updates() {
        let data = vec![0i64; 6];
        let mut tree = LazySegmentTree::from_slice(
            &data,
            |a, b| a + b,
            0,
            |val, upd, len| val + upd * len as i64,
            |old, new| old + new,
        );
        tree.update_range(0, 4, 1);
        tree.update_range(2, 6, 5);
        assert_eq!(tree.query(0, 6), 24);
    }

    #[test]
    fn lazy_segment_tree_point_update() {
        let data = vec![1i64, 2, 3, 4, 5];
        let mut tree = LazySegmentTree::from_slice(
            &data,
            |a, b| a + b,
            0,
            |val, upd, len| val + upd * len as i64,
            |old, new| old + new,
        );
        tree.update_point(2, 100);
        assert_eq!(tree.query(0, 5), 112);
    }

    #[test]
    fn lazy_segment_tree_multiple_range_updates() {
        let data = vec![0i64; 8];
        let mut tree = LazySegmentTree::from_slice(
            &data,
            |a, b| a + b,
            0,
            |val, upd, len| val + upd * len as i64,
            |old, new| old + new,
        );
        tree.update_range(0, 8, 1);
        tree.update_range(0, 4, 2);
        tree.update_range(4, 8, 3);
        assert_eq!(tree.query(0, 4), 12);
    }

    #[test]
    fn segment_tree_get_returns_value() {
        let data = vec![10i64, 20, 30];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.get(1), 20);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn segment_tree_sum_matches_brute_force(
                data in prop::collection::vec(0i64..100, 1..20),
                left in 0usize..19,
                right in 1usize..20,
            ) {
                let right = right.min(data.len());
                let left = left.min(right);
                if left < right {
                    let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
                    let expected: i64 = data[left..right].iter().sum();
                    prop_assert_eq!(tree.query(left, right), expected);
                }
            }

            #[test]
            fn lazy_segment_tree_range_update_matches_brute_force(
                mut data in prop::collection::vec(0i64..50, 1..15),
                range_left in 0usize..14,
                range_right in 1usize..15,
                update_val in -10i64..20,
                query_left in 0usize..14,
                query_right in 1usize..15,
            ) {
                let range_right = range_right.min(data.len());
                let range_left = range_left.min(range_right);
                let query_right = query_right.min(data.len());
                let query_left = query_left.min(query_right);

                let mut tree = LazySegmentTree::from_slice(
                    &data, |a, b| a + b, 0,
                    |val, upd, len| val + upd * len as i64,
                    |old, new| old + new,
                );
                tree.update_range(range_left, range_right, update_val);

                for i in range_left..range_right {
                    data[i] += update_val;
                }
                let expected: i64 = data[query_left..query_right].iter().sum();
                let actual = tree.query(query_left, query_right);
                prop_assert_eq!(actual, expected);
            }
        }
    }
}
