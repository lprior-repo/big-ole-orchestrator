//! Fenwick Tree (Binary Indexed Tree) for prefix sum queries and point updates.

#[derive(Debug, Clone)]
pub struct FenwickTree<T> {
    tree: Vec<T>,
    len: usize,
}

impl<T: Clone + std::ops::Add<Output = T> + Default + Copy> FenwickTree<T> {
    #[must_use]
    pub fn from_slice(data: &[T]) -> Self {
        let len = data.len();
        let mut tree = vec![T::default(); len + 1];
        for (i, val) in data.iter().enumerate() {
            let mut j = i + 1;
            while j <= len {
                tree[j] = tree[j] + *val;
                j += j & j.wrapping_neg();
            }
        }
        Self { tree, len }
    }

    #[must_use]
    pub fn prefix_sum(&self, mut idx: usize) -> T {
        assert!(
            idx <= self.len,
            "prefix_sum: idx ({idx}) out of bounds (len={})",
            self.len
        );
        let mut result = T::default();
        idx += 1;
        while idx > 0 {
            result = result + self.tree[idx];
            idx -= idx & idx.wrapping_neg();
        }
        result
    }

    #[must_use]
    pub fn query(&self, left: usize, right: usize) -> T
    where
        T: std::ops::Sub<Output = T>,
    {
        assert!(
            left <= right,
            "query: left ({left}) must be <= right ({right})"
        );
        assert!(
            right <= self.len,
            "query: right ({right}) out of bounds (len={})",
            self.len
        );
        if left == 0 {
            self.prefix_sum(right - 1)
        } else {
            self.prefix_sum(right - 1) - self.prefix_sum(left - 1)
        }
    }

    pub fn update(&mut self, mut idx: usize, delta: T)
    where
        T: std::ops::AddAssign,
    {
        assert!(
            idx < self.len,
            "update: idx ({idx}) out of bounds (len={})",
            self.len
        );
        idx += 1;
        while idx <= self.len {
            self.tree[idx] = self.tree[idx] + delta;
            idx += idx & idx.wrapping_neg();
        }
    }

    #[must_use]
    pub fn get(&self, idx: usize) -> T
    where
        T: std::ops::Sub<Output = T>,
    {
        assert!(
            idx < self.len,
            "get: idx ({idx}) out of bounds (len={})",
            self.len
        );
        self.prefix_sum(idx)
            - if idx == 0 {
                T::default()
            } else {
                self.prefix_sum(idx - 1)
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

impl<
        T: Clone
            + Default
            + Copy
            + std::ops::Add<Output = T>
            + std::ops::Sub<Output = T>
            + std::ops::AddAssign,
    > FenwickTree<T>
{
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            tree: vec![T::default(); size + 1],
            len: size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Ft = FenwickTree<i64>;

    #[test]
    fn ft_001_from_slice_and_prefix_sum_full() {
        let data = vec![1i64, 3, 5, 7, 9, 11];
        let tree = Ft::from_slice(&data);
        assert_eq!(tree.prefix_sum(5), 36);
    }

    #[test]
    fn ft_002_single_element() {
        let tree = Ft::from_slice(&[42i64]);
        assert_eq!(tree.prefix_sum(0), 42);
    }

    #[test]
    fn ft_003_update_changes_prefix() {
        let mut tree = Ft::from_slice(&[1i64, 2, 3, 4, 5]);
        tree.update(2, 10);
        assert_eq!(tree.prefix_sum(4), 25);
    }

    #[test]
    fn ft_004_query_range() {
        let tree = Ft::from_slice(&[1i64, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(tree.query(2, 5), 12);
    }

    #[test]
    fn ft_005_query_full_range() {
        let data = vec![1i64, 3, 5, 7, 9, 11];
        let tree = Ft::from_slice(&data);
        assert_eq!(tree.query(0, 6), 36);
    }

    #[test]
    fn ft_006_query_single_element() {
        let tree = Ft::from_slice(&[1i64, 2, 3]);
        assert_eq!(tree.query(1, 2), 2);
    }

    #[test]
    fn ft_007_update_then_query() {
        let mut tree = Ft::from_slice(&[1i64, 2, 3, 4, 5]);
        tree.update(0, 9);
        assert_eq!(tree.query(0, 3), 15);
    }

    #[test]
    fn ft_008_get_returns_value() {
        let tree = Ft::from_slice(&[10i64, 20, 30, 40]);
        assert_eq!(tree.get(2), 30);
    }

    #[test]
    fn ft_009_len_and_empty() {
        let tree = Ft::from_slice(&[1i64, 2, 3]);
        assert_eq!(tree.len(), 3);
        assert!(!tree.is_empty());
    }

    #[test]
    fn ft_010_empty_tree() {
        let tree: Ft = Ft::from_slice(&[]);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn ft_011_prefix_sum_oob() {
        let tree = Ft::from_slice(&[1i64, 2, 3]);
        tree.prefix_sum(3);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn ft_012_update_oob() {
        let mut tree = Ft::from_slice(&[1i64, 2, 3]);
        tree.update(3, 1);
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn ft_013_get_oob() {
        let tree = Ft::from_slice(&[1i64, 2, 3]);
        tree.get(3);
    }

    #[test]
    fn ft_014_new_constructor() {
        let mut tree = Ft::new(5);
        tree.update(0, 1);
        tree.update(1, 2);
        tree.update(2, 3);
        assert_eq!(tree.prefix_sum(2), 6);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn fenwick_prefix_sum_matches_brute_force(
                data in prop::collection::vec(0i64..100, 1..20),
                idx in 0usize..19,
            ) {
                let idx = idx.min(data.len() - 1);
                let tree = Ft::from_slice(&data);
                let expected: i64 = data[..=idx].iter().sum();
                prop_assert_eq!(tree.prefix_sum(idx), expected);
            }

            #[test]
            fn fenwick_query_matches_brute_force(
                data in prop::collection::vec(0i64..100, 1..20),
                left in 0usize..19,
                right in 1usize..20,
            ) {
                let right = right.min(data.len());
                let left = left.min(right);
                if left < right {
                    let tree = Ft::from_slice(&data);
                    let expected: i64 = data[left..right].iter().sum();
                    prop_assert_eq!(tree.query(left, right), expected);
                }
            }

            #[test]
            fn fenwick_update_then_query_matches_brute_force(
                mut data in prop::collection::vec(0i64..50, 1..15),
                idx in 0usize..14,
                delta in -20i64..20,
            ) {
                let idx = idx.min(data.len() - 1);
                let mut tree = Ft::from_slice(&data);
                tree.update(idx, delta);
                data[idx] += delta;
                let expected: i64 = data.iter().sum();
                prop_assert_eq!(tree.prefix_sum(data.len() - 1), expected);
            }
        }
    }
}
