//! Internal helper functions for octree operations.

use crate::structures::octree::{Bounds, Octree, Vec3};
use std::cmp::Ordering;

#[inline]
pub fn bounds_center(b: &Bounds) -> Vec3 {
    Vec3::new(
        (b.min.x + b.max.x) / 2.0,
        (b.min.y + b.max.y) / 2.0,
        (b.min.z + b.max.z) / 2.0,
    )
}

#[inline]
pub fn bounds_extent(b: &Bounds) -> Vec3 {
    Vec3::new(b.max.x - b.min.x, b.max.y - b.min.y, b.max.z - b.min.z)
}

#[inline]
pub fn child_index(parent: &Bounds, point: Vec3) -> usize {
    let center = bounds_center(parent);
    usize::from(point.x >= center.x)
        | (usize::from(point.y >= center.y) << 1)
        | (usize::from(point.z >= center.z) << 2)
}

/// Candidate for nearest-neighbor search, sorted by distance (max-heap via Reverse).
#[derive(Debug, Clone)]
pub struct NnCandidate<T: Clone> {
    pub distance: f64,
    pub point: Vec3,
    pub value: T,
}

impl<T: Clone> PartialEq for NnCandidate<T> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<T: Clone> Eq for NnCandidate<T> {}

impl<T: Clone> PartialOrd for NnCandidate<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.distance.partial_cmp(&other.distance)
    }
}

impl<T: Clone> Ord for NnCandidate<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Bounded priority queue that keeps only the k smallest entries (max-heap based).
/// Uses a max-heap so we can efficiently evict the farthest candidate.
#[derive(Debug, Clone)]
pub struct BoundedQueue<T: Clone> {
    capacity: usize,
    heap: std::collections::BinaryHeap<NnCandidate<T>>,
}

impl<T: Clone> BoundedQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            heap: std::collections::BinaryHeap::new(),
        }
    }

    pub fn try_push(&mut self, candidate: NnCandidate<T>) {
        if self.capacity == 0 {
            return;
        }
        if self.heap.len() < self.capacity {
            self.heap.push(candidate);
        } else if let Some(top) = self.heap.peek() {
            if candidate.distance < top.distance {
                self.heap.pop();
                self.heap.push(candidate);
            }
        }
    }

    pub fn worst_distance(&self) -> f64 {
        self.heap
            .peek()
            .map(|c| c.distance)
            .unwrap_or(f64::INFINITY)
    }

    pub fn into_sorted_vec(self) -> Vec<(Vec3, T)> {
        let mut v: Vec<_> = self.heap.into_iter().collect();
        v.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
        });
        v.into_iter().map(|c| (c.point, c.value)).collect()
    }
}

/// Priority-guided depth-first traversal for k-nearest-neighbor queries.
/// Visits children closest to the query point first, pruning branches whose
/// minimum distance exceeds the current worst candidate distance.
pub fn k_nearest_search<T: Clone + serde::Serialize>(
    tree: &Octree<T>,
    query: Vec3,
    k: usize,
) -> Vec<(Vec3, T)> {
    let mut queue = BoundedQueue::new(k);
    search_recursive(tree, query, &mut queue);
    queue.into_sorted_vec()
}

fn search_recursive<T: Clone + serde::Serialize>(
    tree: &Octree<T>,
    query: Vec3,
    queue: &mut BoundedQueue<T>,
) {
    let min_dist = tree.bounds().min_distance_to(query);
    if min_dist > queue.worst_distance() {
        return;
    }

    // Check local data points
    for (pt, val) in tree.local_data() {
        let dist = pt.distance_to(query);
        queue.try_push(NnCandidate {
            distance: dist,
            point: *pt,
            value: val.clone(),
        });
    }

    if tree.is_degenerate() {
        return;
    }

    if let Some(children) = tree.children() {
        // Sort children by minimum distance to query (closest first)
        let mut indexed: Vec<(f64, usize)> = children
            .iter()
            .enumerate()
            .map(|(i, c)| (c.bounds().min_distance_to(query), i))
            .collect();
        indexed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

        for (_, idx) in indexed {
            search_recursive(&children[idx], query, queue);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_center_calculation() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, 5.0);
        assert_eq!(c.y, 5.0);
        assert_eq!(c.z, 5.0);
    }

    #[test]
    fn bounds_center_negative_range() {
        let b = Bounds::new(Vec3::new(-10.0, -10.0, -10.0), Vec3::new(0.0, 0.0, 0.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, -5.0);
        assert_eq!(c.y, -5.0);
        assert_eq!(c.z, -5.0);
    }

    #[test]
    fn bounds_center_asymmetric() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(20.0, 10.0, 5.0));
        let c = bounds_center(&b);
        assert_eq!(c.x, 10.0);
        assert_eq!(c.y, 5.0);
        assert_eq!(c.z, 2.5);
    }

    #[test]
    fn bounds_extent_calculation() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 20.0, 30.0));
        let e = bounds_extent(&b);
        assert_eq!(e.x, 10.0);
        assert_eq!(e.y, 20.0);
        assert_eq!(e.z, 30.0);
    }

    #[test]
    fn bounds_extent_negative_range() {
        let b = Bounds::new(Vec3::new(-10.0, -20.0, -30.0), Vec3::new(0.0, 0.0, 0.0));
        let e = bounds_extent(&b);
        assert_eq!(e.x, 10.0);
        assert_eq!(e.y, 20.0);
        assert_eq!(e.z, 30.0);
    }

    #[test]
    fn child_index_all_corners() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let idx_000 = child_index(&b, Vec3::new(0.0, 0.0, 0.0));
        let idx_111 = child_index(&b, Vec3::new(9.0, 9.0, 9.0));
        let idx_100 = child_index(&b, Vec3::new(9.0, 0.0, 0.0));
        let idx_010 = child_index(&b, Vec3::new(0.0, 9.0, 0.0));
        let idx_001 = child_index(&b, Vec3::new(0.0, 0.0, 9.0));
        let idx_110 = child_index(&b, Vec3::new(9.0, 9.0, 0.0));
        let idx_101 = child_index(&b, Vec3::new(9.0, 0.0, 9.0));
        let idx_011 = child_index(&b, Vec3::new(0.0, 9.0, 9.0));
        assert_eq!(idx_000, 0);
        assert_eq!(idx_111, 7);
        assert_eq!(idx_100, 1);
        assert_eq!(idx_010, 2);
        assert_eq!(idx_001, 4);
        assert_eq!(idx_110, 3);
        assert_eq!(idx_101, 5);
        assert_eq!(idx_011, 6);
    }

    #[test]
    fn child_index_point_on_center() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        let idx = child_index(&b, c);
        assert!(idx < 8);
    }

    #[test]
    fn bounds_center_debug() {
        let b = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        let c = bounds_center(&b);
        let debug_str = format!("{:?}", c);
        assert!(debug_str.contains("5"));
    }
}
