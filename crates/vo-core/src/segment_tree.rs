//! Segment tree for efficient range queries and updates.

/// A segment tree for range aggregation with point updates.
#[derive(Debug, Clone)]
pub struct SegmentTree<T> {
    tree: Vec<T>,
    len: usize,
    n: usize,
    merge: fn(&T, &T) -> T,
    identity: T,
}

impl<T: Clone> SegmentTree<T> {
    /// Build a segment tree from a slice with the given merge function and identity element.
    ///
    /// # Panics
    /// Panics if `data` is empty.
    #[must_use]
    pub fn from_slice(data: &[T], merge: fn(&T, &T) -> T, identity: T) -> Self {
        assert!(
            !data.is_empty(),
            "SegmentTree requires at least one element"
        );

        let len = data.len();
        let n = len.next_power_of_two();
        let mut tree = vec![identity.clone(); 2 * n];

        for (i, val) in data.iter().enumerate() {
            tree[n + i] = val.clone();
        }
        for i in (1..n).rev() {
            tree[i] = merge(&tree[2 * i], &tree[2 * i + 1]);
        }

        Self {
            tree,
            len,
            n,
            merge,
            identity,
        }
    }

    /// Fallible constructor: builds a segment tree from a slice, returning an error
    /// instead of panicking on empty data.
    pub fn try_from_slice(
        data: &[T],
        merge: fn(&T, &T) -> T,
        identity: T,
    ) -> Result<Self, SegmentTreeError> {
        if data.is_empty() {
            return Err(SegmentTreeError::EmptyData);
        }
        Ok(Self::from_slice(data, merge, identity))
    }

    /// Fallible query: validates bounds and range, returning an error instead of
    /// panicking.
    pub fn try_query(&self, left: usize, right: usize) -> Result<T, SegmentTreeError> {
        if left > right {
            return Err(SegmentTreeError::InvalidRange { left, right });
        }
        if right > self.len {
            return Err(SegmentTreeError::RangeOutOfBounds {
                right,
                len: self.len,
            });
        }
        Ok(self.query(left, right))
    }

    /// Fallible get: validates index bounds, returning an error instead of
    /// panicking.
    pub fn try_get(&self, index: usize) -> Result<T, SegmentTreeError> {
        if index >= self.len {
            return Err(SegmentTreeError::IndexOutOfBounds {
                index,
                len: self.len,
            });
        }
        Ok(self.get(index))
    }

    /// Query the aggregate value over range `[left, right)`.
    #[must_use]
    pub fn query(&self, left: usize, right: usize) -> T {
        assert!(
            left <= right,
            "query: left ({left}) must be <= right ({right})"
        );
        assert!(
            right <= self.len,
            "query: right ({right}) out of bounds (len={})",
            self.len
        );

        let mut left = left + self.n;
        let mut right = right + self.n;
        let mut result_left = self.identity.clone();
        let mut result_right = self.identity.clone();

        while left < right {
            if left % 2 == 1 {
                result_left = (self.merge)(&result_left, &self.tree[left]);
                left += 1;
            }
            if right % 2 == 1 {
                right -= 1;
                result_right = (self.merge)(&self.tree[right], &result_right);
            }
            left /= 2;
            right /= 2;
        }

        (self.merge)(&result_left, &result_right)
    }

    /// Update the value at a single position.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    #[must_use]
    pub fn update(&mut self, index: usize, value: T) -> Self {
        assert!(
            index < self.len,
            "update: index ({index}) out of bounds (len={})",
            self.len
        );
        let mut pos = index + self.n;
        self.tree[pos] = value;
        while pos > 1 {
            pos /= 2;
            self.tree[pos] = (self.merge)(&self.tree[2 * pos], &self.tree[2 * pos + 1]);
        }
        self.clone()
    }

    /// Fallible update: validates index bounds, returning an error instead of
    /// panicking.
    pub fn try_update(&mut self, index: usize, value: T) -> Result<(), SegmentTreeError> {
        if index >= self.len {
            return Err(SegmentTreeError::IndexOutOfBounds {
                index,
                len: self.len,
            });
        }
        let mut pos = index + self.n;
        self.tree[pos] = value;
        while pos > 1 {
            pos /= 2;
            self.tree[pos] = (self.merge)(&self.tree[2 * pos], &self.tree[2 * pos + 1]);
        }
        Ok(())
    }

    #[must_use]
    pub fn get(&self, index: usize) -> T {
        assert!(
            index < self.len,
            "get: index ({index}) out of bounds (len={})",
            self.len
        );
        self.tree[self.n + index].clone()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LAZY SEGMENT TREE (Range Updates, Range Queries)
// ═══════════════════════════════════════════════════════════════════════════════

/// A segment tree with lazy propagation for range updates and range queries.
#[derive(Debug, Clone)]
pub struct LazySegmentTree<T, U> {
    tree: Vec<T>,
    lazy: Vec<Option<U>>,
    len: usize,
    n: usize,
    merge: fn(&T, &T) -> T,
    identity: T,
    apply: fn(&T, &U, usize) -> T,
    compose: fn(&U, &U) -> U,
}

impl<T: Clone, U: Clone> LazySegmentTree<T, U> {
    #[must_use]
    pub fn from_slice(
        data: &[T],
        merge: fn(&T, &T) -> T,
        identity: T,
        apply: fn(&T, &U, usize) -> T,
        compose: fn(&U, &U) -> U,
    ) -> Self {
        assert!(
            !data.is_empty(),
            "LazySegmentTree requires at least one element"
        );
        let len = data.len();
        let n = len.next_power_of_two();
        let mut tree = vec![identity.clone(); 2 * n];
        for (i, val) in data.iter().enumerate() {
            tree[n + i] = val.clone();
        }
        for i in (1..n).rev() {
            tree[i] = merge(&tree[2 * i], &tree[2 * i + 1]);
        }
        Self {
            tree,
            lazy: vec![None; 2 * n],
            len,
            n,
            merge,
            identity,
            apply,
            compose,
        }
    }

    #[must_use]
    pub fn query(&mut self, left: usize, right: usize) -> T {
        assert!(
            left <= right,
            "query: left ({left}) must be <= right ({right})"
        );
        assert!(
            right <= self.len,
            "query: right ({right}) out of bounds (len={})",
            self.len
        );
        self.query_inner(1, 0, self.n, left, right)
    }

    fn query_inner(&mut self, node: usize, nl: usize, nr: usize, ql: usize, qr: usize) -> T {
        self.push_down(node, nr - nl);
        if ql <= nl && nr <= qr {
            return self.tree[node].clone();
        }
        if nr <= ql || qr <= nl {
            return self.identity.clone();
        }
        let mid = (nl + nr) / 2;
        let l = self.query_inner(2 * node, nl, mid, ql, qr);
        let r = self.query_inner(2 * node + 1, mid, nr, ql, qr);
        (self.merge)(&l, &r)
    }

    pub fn update_range(&mut self, left: usize, right: usize, update: U) {
        assert!(left <= right);
        assert!(right <= self.len);
        if left < right {
            self.update_range_inner(1, 0, self.n, left, right, &update);
        }
    }

    fn update_range_inner(
        &mut self,
        node: usize,
        nl: usize,
        nr: usize,
        ql: usize,
        qr: usize,
        update: &U,
    ) {
        self.push_down(node, nr - nl);
        if ql <= nl && nr <= qr {
            let seg_len = nr - nl;
            self.tree[node] = (self.apply)(&self.tree[node], update, seg_len);
            self.lazy[node] = Some(match self.lazy[node].take() {
                Some(e) => (self.compose)(&e, update),
                None => update.clone(),
            });
            return;
        }
        if nr <= ql || qr <= nl {
            return;
        }
        let mid = (nl + nr) / 2;
        self.update_range_inner(2 * node, nl, mid, ql, qr, update);
        self.update_range_inner(2 * node + 1, mid, nr, ql, qr, update);
        self.tree[node] = (self.merge)(&self.tree[2 * node], &self.tree[2 * node + 1]);
    }

    pub fn update_point(&mut self, index: usize, value: T) {
        assert!(index < self.len);
        self.update_point_inner(1, 0, self.n, index, value);
    }

    fn update_point_inner(&mut self, node: usize, nl: usize, nr: usize, index: usize, value: T) {
        self.push_down(node, nr - nl);
        if nr - nl == 1 {
            self.tree[node] = value;
            return;
        }
        let mid = (nl + nr) / 2;
        if index < mid {
            self.update_point_inner(2 * node, nl, mid, index, value);
        } else {
            self.update_point_inner(2 * node + 1, mid, nr, index, value);
        }
        self.tree[node] = (self.merge)(&self.tree[2 * node], &self.tree[2 * node + 1]);
    }

    fn push_down(&mut self, node: usize, seg_len: usize) {
        if let Some(pending) = self.lazy[node].take() {
            if seg_len > 1 {
                let left = 2 * node;
                let right = 2 * node + 1;
                let child_len = seg_len / 2;
                self.tree[left] = (self.apply)(&self.tree[left], &pending, child_len);
                self.lazy[left] = Some(match self.lazy[left].take() {
                    Some(e) => (self.compose)(&e, &pending),
                    None => pending.clone(),
                });
                self.tree[right] = (self.apply)(&self.tree[right], &pending, child_len);
                self.lazy[right] = Some(match self.lazy[right].take() {
                    Some(e) => (self.compose)(&e, &pending),
                    None => pending,
                });
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Errors for segment tree operations.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ST-00a: try_from_slice rejects empty data
    #[test]
    fn segment_tree_try_from_slice_rejects_empty() {
        let result = SegmentTree::try_from_slice(&[], |a: &i64, b: &i64| a + b, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SegmentTreeError::EmptyData);
    }

    // ST-00b: try_query rejects out-of-bounds range
    #[test]
    fn segment_tree_try_query_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::try_from_slice(&data, |a, b| a + b, 0).unwrap();
        let result = tree.try_query(0, 4);
        assert!(result.is_err());
    }

    // ST-00c: try_get rejects out-of-bounds index
    #[test]
    fn segment_tree_try_get_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::try_from_slice(&data, |a, b| a + b, 0).unwrap();
        let result = tree.try_get(3);
        assert!(result.is_err());
    }

    // ST-01: Build from slice and query full range (sum)
    #[test]
    fn segment_tree_query_full_range_sum() {
        let data = vec![1i64, 3, 5, 7, 9, 11];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(0, 6), 36);
    }

    // ST-02: Point update changes query result
    #[test]
    fn segment_tree_point_update_changes_query() {
        let data = vec![1i64, 2, 3, 4, 5];
        let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        let _ = tree.update(2, 10);
        assert_eq!(tree.query(0, 5), 22);
    }

    // ST-03: Range query returns correct partial sum
    #[test]
    fn segment_tree_range_query_partial() {
        let data = vec![1i64, 2, 3, 4, 5, 6, 7, 8];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(2, 5), 12);
    }

    // ST-04: Out-of-bounds panics
    #[test]
    #[should_panic(expected = "out of bounds")]
    fn segment_tree_query_out_of_bounds() {
        let data = vec![1i64, 2, 3];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        let _ = tree.query(0, 4);
    }

    // ST-05: Single element tree
    #[test]
    fn segment_tree_single_element() {
        let data = vec![42i64];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(0, 1), 42);
    }

    // ST-06: Identity property - single element queries return that element
    #[test]
    fn segment_tree_identity_property() {
        let data = vec![5i64, 10, 15];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.query(1, 2), 10);
    }

    // ST-07: Lazy range update correctness (additive)
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

    // ST-08: Overlapping lazy updates compose correctly
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

    // ST-09: Point update on lazy tree
    #[test]
    fn lazy_segment_tree_point_update() {
        let data = vec![1i64, 2, 3, 4, 5];
        let mut tree = LazySegmentTree::from_slice(
            &data,
            |a, b| a + b,
            0,
            |val, upd: &i64, len| val + upd * len as i64,
            |old, new| old + new,
        );
        tree.update_point(2, 100);
        assert_eq!(tree.query(0, 5), 112);
    }

    // ST-10: Multiple range updates then query
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

    // ST-11: get returns value at position
    #[test]
    fn segment_tree_get_returns_value() {
        let data = vec![10i64, 20, 30];
        let tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        assert_eq!(tree.get(1), 20);
    }

    // Proptest: range query matches brute force
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

            // Proptest: lazy range update matches brute force
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
            |val, upd: &i64, len| val + upd * len as i64,
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

#[cfg(test)]
mod manual_test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_update_panic() {
        let data = vec![1i64, 2, 3];
        let mut tree = SegmentTree::from_slice(&data, |a, b| a + b, 0);
        tree.update(3, 10);
    }
}
