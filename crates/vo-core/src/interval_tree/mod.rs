//! Interval tree for efficient interval overlap queries.
//!
//! An interval tree supports:
//! - Insert/delete intervals
//! - Find all intervals overlapping a point
//! - Find all intervals overlapping another interval
//!
//! This implementation uses an augmented BST with max-end tracking.

mod ops;
mod tests;
mod types;

pub use types::{Interval, IntervalNode, IntervalTreeError};

use ops::{merge_nodes, recalculate_max, update_max_end};
use std::cmp::Ordering;
use types::{Interval, IntervalNode, IntervalTreeError};

pub struct IntervalTree<T: Ord, V> {
    root: Option<Box<IntervalNode<T, V>>>,
    len: usize,
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
        if let Some(ref mut n) = node {
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
                    *current = merge_nodes(left, right);
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
