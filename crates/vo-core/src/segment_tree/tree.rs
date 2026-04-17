use crate::segment_tree::error::SegmentTreeError;

#[derive(Debug, Clone)]
pub struct SegmentTree<T> {
    tree: Vec<T>,
    len: usize,
    n: usize,
    merge: fn(&T, &T) -> T,
    identity: T,
}

impl<T: Clone> SegmentTree<T> {
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

    pub fn try_get(&self, index: usize) -> Result<T, SegmentTreeError> {
        if index >= self.len {
            return Err(SegmentTreeError::IndexOutOfBounds {
                index,
                len: self.len,
            });
        }
        Ok(self.get(index))
    }

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

    pub fn update(&mut self, index: usize, value: T) {
        let mut pos = index + self.n;
        self.tree[pos] = value;
        while pos > 1 {
            pos /= 2;
            self.tree[pos] = (self.merge)(&self.tree[2 * pos], &self.tree[2 * pos + 1]);
        }
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
