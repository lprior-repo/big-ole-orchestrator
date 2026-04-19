//! Interval tree for efficient interval overlap queries.
//!
//! An interval tree supports:
//! - Insert/delete intervals
//! - Find all intervals overlapping a point
//! - Find all intervals overlapping another interval
//!
//! This implementation uses an augmented BST with max-end tracking.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval<T: Ord, V> {
    pub start: T,
    pub end: T,
    pub value: V,
}

impl<T: Ord, V> Interval<T, V> {
    pub fn new(start: T, end: T, value: V) -> Self {
        Self { start, end, value }
    }

    pub fn contains_point(&self, point: &T) -> bool {
        &self.start <= point && point < &self.end
    }

    pub fn overlaps_interval(&self, other: &Interval<T, V>) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn overlaps_point(&self, point: &T) -> bool {
        self.contains_point(point)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalNode<T: Ord, V> {
    pub interval: Interval<T, V>,
    pub max_end: T,
    pub left: Option<Box<IntervalNode<T, V>>>,
    pub right: Option<Box<IntervalNode<T, V>>>,
}

#[derive(Debug, Clone)]
pub struct IntervalTree<T: Ord, V> {
    root: Option<Box<IntervalNode<T, V>>>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntervalTreeError {
    #[error("tree is empty")]
    EmptyTree,

    #[error("interval not found")]
    NotFound,

    #[error("invalid interval: start ({start}) >= end ({end})")]
    InvalidInterval { start: T, end: T },
}

impl<T: Ord, V> IntervalTree<T, V> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn update_max_end(node: &mut Box<IntervalNode<T, V>>) {
        let max_left = node
            .left
            .as_ref()
            .map_or(&node.interval.end, |n| &n.max_end);
        let max_right = node
            .right
            .as_ref()
            .map_or(&node.interval.end, |n| &n.max_end);
        node.max_end = std::cmp::max(
            node.interval.end.clone(),
            std::cmp::max(max_left.clone(), max_right.clone()),
        );
    }

    fn rotate_right(node: &mut Option<Box<IntervalNode<T, V>>>) {
        let mut n = node.take().unwrap();
        let mut left = n.left.take().unwrap();
        n.left = left.right.take();
        if let Some(ref mut l) = n.left {
            l.parent = None;
        }
        left.right = Some(n);
        *node = Some(left);
    }

    fn rotate_left(node: &mut Option<Box<IntervalNode<T, V>>>) {
        let mut n = node.take().unwrap();
        let mut right = n.right.take().unwrap();
        n.right = right.left.take();
        if let Some(ref mut r) = n.right {
            r.parent = None;
        }
        right.left = Some(n);
        *node = Some(right);
    }

    fn recalculate_max(&mut self, node: &mut Box<IntervalNode<T, V>>) {
        let left_max = node
            .left
            .as_mut()
            .map_or(&node.interval.end, |n| &n.max_end);
        let right_max = node
            .right
            .as_mut()
            .map_or(&node.interval.end, |n| &n.max_end);
        node.max_end = std::cmp::max(
            node.interval.end.clone(),
            std::cmp::max(left_max.clone(), right_max.clone()),
        );
    }

    pub fn insert(&mut self, start: T, end: T, value: V) -> Result<(), IntervalTreeError>
    where
        T: Clone,
    {
        if start >= end {
            return Err(IntervalTreeError::InvalidInterval { start, end });
        }

        let interval = Interval { start, end, value };

        if self.root.is_none() {
            self.root = Some(Box::new(IntervalNode {
                interval: interval.clone(),
                max_end: interval.end.clone(),
                left: None,
                right: None,
            }));
            self.len = 1;
            return Ok(());
        }

        let mut current = &mut self.root;
        loop {
            current.as_mut().unwrap().max_end = std::cmp::max(
                current.as_ref().unwrap().max_end.clone(),
                interval.end.clone(),
            );

            match interval
                .start
                .cmp(&current.as_ref().unwrap().interval.start)
            {
                Ordering::Less => {
                    if current.as_ref().unwrap().left.is_some() {
                        current = &mut current.as_mut().unwrap().left;
                    } else {
                        current.as_mut().unwrap().left = Some(Box::new(IntervalNode {
                            interval: interval.clone(),
                            max_end: interval.end.clone(),
                            left: None,
                            right: None,
                        }));
                        self.len += 1;
                        break;
                    }
                }
                Ordering::Greater => {
                    if current.as_ref().unwrap().right.is_some() {
                        current = &mut current.as_ref().unwrap().right;
                    } else {
                        current.as_mut().unwrap().right = Some(Box::new(IntervalNode {
                            interval: interval.clone(),
                            max_end: interval.end.clone(),
                            left: None,
                            right: None,
                        }));
                        self.len += 1;
                        break;
                    }
                }
                Ordering::Equal => {
                    current.as_mut().unwrap().interval.value = interval.value;
                    break;
                }
            }
        }

        self.rebalance_on_insert(&mut self.root);
        Ok(())
    }

    fn rebalance_on_insert(&mut self, node: &mut Option<Box<IntervalNode<T, V>>>) {
        // Simple AVL-like rebalancing after insert
        // This is a simplified version - for production, full AVL rotations would be used
        if let Some(ref mut n) = node {
            // Update max_end after any structural changes
            let left_max = n.left.as_mut().map_or(&n.interval.end, |l| &l.max_end);
            let right_max = n.right.as_mut().map_or(&n.interval.end, |r| &r.max_end);
            n.max_end = std::cmp::max(
                n.interval.end.clone(),
                std::cmp::max(left_max.clone(), right_max.clone()),
            );
        }
    }

    pub fn find_point_overlaps(&self, point: &T) -> Vec<&V>
    where
        T: Clone,
    {
        let mut results = Vec::new();
        self.find_point_overlaps_recursive(self.root.as_deref(), point, &mut results);
        results
    }

    fn find_point_overlaps_recursive<'a>(
        &'a self,
        node: Option<&'a Box<IntervalNode<T, V>>>,
        point: &T,
        results: &mut Vec<&'a V>,
    ) {
        if let Some(n) = node {
            if n.interval.start <= *point && *point < n.interval.end {
                results.push(&n.interval.value);
            }

            if n.left.is_some() && n.left.as_ref().map_or(false, |l| l.max_end > *point) {
                self.find_point_overlaps_recursive(n.left.as_deref(), point, results);
            }

            if n.right.is_some() && n.interval.start <= *point {
                self.find_point_overlaps_recursive(n.right.as_deref(), point, results);
            }
        }
    }

    pub fn find_interval_overlaps(&self, start: &T, end: &T) -> Vec<&V>
    where
        T: Clone,
    {
        let mut results = Vec::new();
        self.find_interval_overlaps_recursive(self.root.as_deref(), start, end, &mut results);
        results
    }

    fn find_interval_overlaps_recursive<'a>(
        &'a self,
        node: Option<&'a Box<IntervalNode<T, V>>>,
        start: &T,
        end: &T,
        results: &mut Vec<&'a V>,
    ) {
        if let Some(n) = node {
            if n.interval.start < *end && *start < n.interval.end {
                results.push(&n.interval.value);
            }

            if n.left.is_some() && n.left.as_ref().map_or(false, |l| l.max_end > *start) {
                self.find_interval_overlaps_recursive(n.left.as_deref(), start, end, results);
            }

            if n.right.is_some() && n.right.as_ref().map_or(false, |r| r.interval.start < *end) {
                self.find_interval_overlaps_recursive(n.right.as_deref(), start, end, results);
            }
        }
    }

    pub fn remove(&mut self, start: &T, end: &T) -> Option<V>
    where
        T: Clone + PartialEq,
    {
        let mut current = &mut self.root;
        while let Some(ref mut n) = current {
            if start < &n.interval.start {
                current = &mut n.left;
            } else if start > &n.interval.start {
                current = &mut n.right;
            } else {
                if end == &n.interval.end {
                    let value = n.interval.value.clone();
                    let left = n.left.take();
                    let right = n.right.take();
                    *current = Self::merge_nodes(left, right);
                    self.len = self.len.saturating_sub(1);
                    return Some(value);
                } else if end < &n.interval.end {
                    current = &mut n.left;
                } else {
                    current = &mut n.right;
                }
            }
        }
        None
    }

    fn merge_nodes(
        left: Option<Box<IntervalNode<T, V>>>,
        right: Option<Box<IntervalNode<T, V>>>,
    ) -> Option<Box<IntervalNode<T, V>>> {
        match (left, right) {
            (None, None) => None,
            (Some(l), None) => Some(l),
            (None, Some(r)) => Some(r),
            (Some(mut l), Some(r)) => {
                let mut current = &mut Some(l);
                while current.as_ref().map_or(false, |n| n.right.is_some()) {
                    let right_max = current
                        .as_mut()
                        .unwrap()
                        .right
                        .as_mut()
                        .map_or(&current.as_ref().unwrap().interval.end, |n| &n.max_end);
                    current.as_mut().unwrap().max_end = std::cmp::max(
                        current.as_ref().unwrap().interval.end.clone(),
                        right_max.clone(),
                    );
                    current = &mut current.as_mut().unwrap().right;
                }
                *current = Some(r);
                l
            }
        }
    }

    pub fn contains(&self, start: &T, end: &T) -> bool
    where
        T: Clone + PartialEq,
    {
        let mut current = &self.root;
        while let Some(n) = current {
            if start < &n.interval.start {
                current = &n.left;
            } else if start > &n.interval.start {
                current = &n.right;
            } else {
                return end == &n.interval.end;
            }
        }
        false
    }
}

impl<T, V> Default for IntervalTree<T, V>
where
    T: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
