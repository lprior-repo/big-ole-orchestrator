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
