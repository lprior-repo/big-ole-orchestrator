use crate::interval_tree::IntervalTree;

type It = IntervalTree<i32, String>;

#[test]
fn it_001_new_tree_is_empty() {
    let tree: It = It::new();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
}

#[test]
fn it_002_insert_single_interval() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    assert_eq!(tree.len(), 1);
    assert!(!tree.is_empty());
}

#[test]
fn it_003_insert_multiple_intervals() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    tree.insert(20, 30, "twenty-thirty".to_string()).unwrap();
    tree.insert(10, 20, "ten-twenty".to_string()).unwrap();
    assert_eq!(tree.len(), 3);
}

#[test]
fn it_004_find_point_overlap_single() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    let overlaps = tree.find_point_overlaps(&5);
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0], "zero-ten");
}

#[test]
fn it_005_find_point_no_overlap() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    let overlaps = tree.find_point_overlaps(&15);
    assert!(overlaps.is_empty());
}

#[test]
fn it_006_find_multiple_overlaps() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    tree.insert(5, 15, "five-fifteen".to_string()).unwrap();
    tree.insert(20, 30, "twenty-thirty".to_string()).unwrap();
    let overlaps = tree.find_point_overlaps(&7);
    assert_eq!(overlaps.len(), 2);
}

#[test]
fn it_007_find_interval_overlap() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    tree.insert(20, 30, "twenty-thirty".to_string()).unwrap();
    let overlaps = tree.find_interval_overlaps(&5, 25);
    assert_eq!(overlaps.len(), 2);
}

#[test]
fn it_008_find_interval_partial_overlap() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    tree.insert(5, 15, "five-fifteen".to_string()).unwrap();
    let overlaps = tree.find_interval_overlaps(&3, 7);
    assert_eq!(overlaps.len(), 2);
}

#[test]
fn it_009_remove_existing() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    assert_eq!(tree.len(), 1);
    let removed = tree.remove(&0, &10);
    assert_eq!(removed, Some("zero-ten".to_string()));
    assert_eq!(tree.len(), 0);
}

#[test]
fn it_010_remove_nonexistent() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    let removed = tree.remove(&5, &15);
    assert_eq!(removed, None);
    assert_eq!(tree.len(), 1);
}

#[test]
fn it_011_contains() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    assert!(tree.contains(&0, &10));
    assert!(!tree.contains(&0, &5));
    assert!(!tree.contains(&5, &10));
}

#[test]
fn it_012_invalid_interval() {
    let mut tree = It::new();
    let result = tree.insert(10, 5, "invalid".to_string());
    assert!(result.is_err());
}

#[test]
fn it_013_point_at_boundary() {
    let mut tree = It::new();
    tree.insert(0, 10, "zero-ten".to_string()).unwrap();
    let overlaps_start = tree.find_point_overlaps(&0);
    assert_eq!(overlaps_start.len(), 1);
    let overlaps_end = tree.find_point_overlaps(&10);
    assert!(overlaps_end.is_empty());
}

#[test]
fn it_014_empty_tree_operations() {
    let tree: It = It::new();
    assert!(tree.find_point_overlaps(&5).is_empty());
    assert!(tree.find_interval_overlaps(&0, &10).is_empty());
    assert!(!tree.contains(&0, &10));
}

#[test]
fn it_015_update_existing_interval() {
    let mut tree = It::new();
    tree.insert(0, 10, "original".to_string()).unwrap();
    tree.insert(0, 10, "updated".to_string()).unwrap();
    assert_eq!(tree.len(), 1);
    let overlaps = tree.find_point_overlaps(&5);
    assert_eq!(overlaps[0], "updated");
}

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn interval_insert_and_find_point(
            intervals in prop::collection::vec(
                (0i32..1000, 1i32..1000), 1..20
            ),
            query_point in 0i32..1500,
        ) {
            let mut tree: IntervalTree<i32, i32> = IntervalTree::new();
            for (i, (start, len)) in intervals.iter().enumerate() {
                let end = start + len;
                tree.insert(*start, end, i).unwrap();
            }

            let expected_count = intervals.iter()
                .filter(|(start, len)| {
                    let end = start + len;
                    *start <= query_point && query_point < end
                })
                .count();

            let overlaps = tree.find_point_overlaps(&query_point);
            prop_assert_eq!(overlaps.len(), expected_count);
        }

        #[test]
        fn interval_insert_and_find_overlaps(
            intervals in prop::collection::vec(
                (0i32..1000, 1i32..1000), 1..20
            ),
            query_start in 0i32..1500,
            query_len in 1i32..500,
        ) {
            let mut tree: IntervalTree<i32, i32> = IntervalTree::new();
            for (i, (start, len)) in intervals.iter().enumerate() {
                let end = start + len;
                tree.insert(*start, end, i).unwrap();
            }

            let query_end = query_start + query_len;
            let expected_count = intervals.iter()
                .filter(|(start, len)| {
                    let end = start + len;
                    *start < query_end && query_start < end
                })
                .count();

            let overlaps = tree.find_interval_overlaps(&query_start, &query_end);
            prop_assert_eq!(overlaps.len(), expected_count);
        }
    }
}
