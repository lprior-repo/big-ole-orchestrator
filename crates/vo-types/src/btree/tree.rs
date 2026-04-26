use serde::{Deserialize, Serialize};

use super::node::{BTreeNode, DEFAULT_ORDER};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BTreeError {
    #[error("key not found")]
    KeyNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTree<K, V> {
    root: Option<BTreeNode<K, V>>,
    order: usize,
    len: usize,
}

enum InsertResult<K, V> {
    Done(BTreeNode<K, V>),
    Updated(BTreeNode<K, V>),
    Split(BTreeNode<K, V>, K, V, BTreeNode<K, V>),
}

impl<K, V> BTree<K, V> {
    #[must_use]
    pub fn new() -> Self {
        Self::with_order(DEFAULT_ORDER)
    }

    #[must_use]
    pub fn with_order(order: usize) -> Self {
        assert!(order >= 3, "B-tree order must be at least 3");
        Self {
            root: None,
            order,
            len: 0,
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

    fn max_keys(&self) -> usize {
        self.order - 1
    }

    fn min_keys(&self) -> usize {
        (self.order - 1) / 2
    }
}

impl<K: Ord + Clone, V: Clone> BTree<K, V> {
    #[must_use]
    pub fn search(&self, key: &K) -> Option<&V> {
        self.root
            .as_ref()
            .and_then(|node| Self::search_node(node, key))
    }

    fn search_node<'a>(node: &'a BTreeNode<K, V>, key: &K) -> Option<&'a V> {
        let idx = node.search_index(key);
        if idx < node.keys.len() && &node.keys[idx] == key {
            return Some(&node.values[idx]);
        }
        if node.is_leaf() {
            return None;
        }
        Self::search_node(&node.children[idx], key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.root.is_none() {
            self.root = Some(BTreeNode::leaf(vec![key], vec![value]));
            self.len = 1;
            return;
        }

        let root = self
            .root
            .take()
            .expect("btree root missing after is_none check");
        let split = self.insert_recursive(root, key, value);

        let is_new_key = !matches!(split, InsertResult::Updated(_));
        match split {
            InsertResult::Done(node) | InsertResult::Updated(node) => {
                self.root = Some(node);
            }
            InsertResult::Split(left, median_key, median_val, right) => {
                let new_root = BTreeNode {
                    keys: vec![median_key],
                    values: vec![median_val],
                    children: vec![left, right],
                };
                self.root = Some(new_root);
            }
        }
        if is_new_key {
            self.len += 1;
        }
    }

    fn insert_recursive(&self, mut node: BTreeNode<K, V>, key: K, value: V) -> InsertResult<K, V> {
        if node.is_leaf() {
            let idx = node.search_index(&key);
            if idx < node.keys.len() && node.keys[idx] == key {
                node.values[idx] = value;
                return InsertResult::Updated(node);
            }
            node.keys.insert(idx, key);
            node.values.insert(idx, value);

            if node.keys.len() <= self.max_keys() {
                return InsertResult::Done(node);
            }

            let mid = node.keys.len() / 2;
            let median_key = node.keys.remove(mid);
            let median_val = node.values.remove(mid);
            let right = BTreeNode::leaf(node.keys.split_off(mid), node.values.split_off(mid));
            InsertResult::Split(node, median_key, median_val, right)
        } else {
            let idx = node.search_index(&key);
            if idx < node.keys.len() && node.keys[idx] == key {
                node.values[idx] = value;
                return InsertResult::Updated(node);
            }
            let child = node.children.remove(idx);
            let result = self.insert_recursive(child, key, value);

            match result {
                InsertResult::Done(updated_child) => {
                    node.children.insert(idx, updated_child);
                    InsertResult::Done(node)
                }
                InsertResult::Updated(updated_child) => {
                    node.children.insert(idx, updated_child);
                    InsertResult::Updated(node)
                }
                InsertResult::Split(left, median_key, median_val, right) => {
                    node.keys.insert(idx, median_key);
                    node.values.insert(idx, median_val);
                    node.children.insert(idx, left);
                    node.children.insert(idx + 1, right);

                    if node.keys.len() <= self.max_keys() {
                        return InsertResult::Done(node);
                    }

                    let mid = node.keys.len() / 2;
                    let median_key = node.keys.remove(mid);
                    let median_val = node.values.remove(mid);
                    let right_node = BTreeNode {
                        keys: node.keys.split_off(mid),
                        values: node.values.split_off(mid),
                        children: node.children.split_off(mid + 1),
                    };
                    InsertResult::Split(node, median_key, median_val, right_node)
                }
            }
        }
    }

    pub fn delete(&mut self, key: &K) -> Result<V, BTreeError> {
        if self.root.is_none() {
            return Err(BTreeError::KeyNotFound);
        }

        // Search first to avoid losing root on KeyNotFound error.
        // delete_recursive takes ownership of the node, so a failed
        // call would leave self.root as None (taken but never restored).
        if self.search(key).is_none() {
            return Err(BTreeError::KeyNotFound);
        }

        let root = self
            .root
            .take()
            .expect("btree root missing after search check");
        let (updated_root, removed) = self
            .delete_recursive(root, key)
            .expect("search confirmed key exists");
        self.len = self.len.saturating_sub(1);

        if updated_root.keys.is_empty() {
            if updated_root.is_leaf() {
                self.root = None;
            } else {
                self.root = Some(
                    updated_root
                        .children
                        .into_iter()
                        .next()
                        .expect("btree children missing despite internal node"),
                );
            }
        } else {
            self.root = Some(updated_root);
        }

        Ok(removed)
    }

    fn delete_recursive(
        &self,
        mut node: BTreeNode<K, V>,
        key: &K,
    ) -> Result<(BTreeNode<K, V>, V), BTreeError> {
        let idx = node.search_index(key);
        let found_key = idx < node.keys.len() && &node.keys[idx] == key;

        if node.is_leaf() {
            if !found_key {
                return Err(BTreeError::KeyNotFound);
            }
            let removed = node.values.remove(idx);
            node.keys.remove(idx);
            return Ok((node, removed));
        }

        if found_key {
            let removed_val = node.values[idx].clone();

            if node.children[idx].keys.len() > self.min_keys() {
                let (pred_key, pred_val, updated_child) =
                    self.remove_predecessor(node.children.remove(idx))?;
                node.keys[idx] = pred_key;
                node.values[idx] = pred_val;
                self.maybe_split_child(&mut node, idx, updated_child);
                return Ok((node, removed_val));
            }

            if node.children[idx + 1].keys.len() > self.min_keys() {
                let (succ_key, succ_val, updated_child) =
                    self.remove_successor(node.children.remove(idx + 1))?;
                node.keys[idx] = succ_key;
                node.values[idx] = succ_val;
                self.maybe_split_child(&mut node, idx + 1, updated_child);
                return Ok((node, removed_val));
            }

            let left = node.children.remove(idx);
            let parent_key = node.keys.remove(idx);
            let parent_val = node.values.remove(idx);
            let right = node.children.remove(idx);
            let merged = Self::merge_nodes(left, parent_key, parent_val, right);
            let (updated, _) = self.delete_recursive(merged, key)?;
            self.maybe_split_child(&mut node, idx, updated);
            return Ok((node, removed_val));
        }

        let mut child_idx = idx;
        if node.children[child_idx].keys.len() <= self.min_keys() {
            self.ensure_child_has_minimum(&mut node, child_idx);
            child_idx = node.search_index(key);
        }

        let child = node.children.remove(child_idx);
        let (updated_child, removed) = self.delete_recursive(child, key)?;
        self.maybe_split_child(&mut node, child_idx, updated_child);
        Ok((node, removed))
    }

    fn remove_predecessor(
        &self,
        mut node: BTreeNode<K, V>,
    ) -> Result<(K, V, BTreeNode<K, V>), BTreeError> {
        if node.is_leaf() {
            let key = node.keys.pop().ok_or(BTreeError::KeyNotFound)?;
            let val = node.values.pop().ok_or(BTreeError::KeyNotFound)?;
            return Ok((key, val, node));
        }

        let last_idx = node.children.len() - 1;
        if node.children[last_idx].keys.len() <= self.min_keys() {
            self.ensure_child_has_minimum(&mut node, last_idx);
        }
        let last_idx = node.children.len() - 1;
        let child = node.children.remove(last_idx);
        let (pred_key, pred_val, updated) = self.remove_predecessor(child)?;
        node.children.insert(last_idx, updated);
        Ok((pred_key, pred_val, node))
    }

    fn remove_successor(
        &self,
        mut node: BTreeNode<K, V>,
    ) -> Result<(K, V, BTreeNode<K, V>), BTreeError> {
        if node.is_leaf() {
            let key = node.keys.remove(0);
            let val = node.values.remove(0);
            return Ok((key, val, node));
        }

        if node.children[0].keys.len() <= self.min_keys() {
            self.ensure_child_has_minimum(&mut node, 0);
        }
        let child = node.children.remove(0);
        let (succ_key, succ_val, updated) = self.remove_successor(child)?;
        node.children.insert(0, updated);
        Ok((succ_key, succ_val, node))
    }

    fn ensure_child_has_minimum(&self, node: &mut BTreeNode<K, V>, idx: usize) {
        if idx > 0 && node.children[idx - 1].keys.len() > self.min_keys() {
            self.borrow_from_left(node, idx);
        } else if idx < node.children.len() - 1
            && node.children[idx + 1].keys.len() > self.min_keys()
        {
            self.borrow_from_right(node, idx);
        } else if idx > 0 {
            let left = node.children.remove(idx - 1);
            let parent_key = node.keys.remove(idx - 1);
            let parent_val = node.values.remove(idx - 1);
            let right = node.children.remove(idx - 1);
            let merged = Self::merge_nodes(left, parent_key, parent_val, right);
            node.children.insert(idx - 1, merged);
        } else {
            let left = node.children.remove(0);
            let parent_key = node.keys.remove(0);
            let parent_val = node.values.remove(0);
            let right = node.children.remove(0);
            let merged = Self::merge_nodes(left, parent_key, parent_val, right);
            node.children.insert(0, merged);
        }
    }

    fn borrow_from_left(&self, node: &mut BTreeNode<K, V>, idx: usize) {
        let left_idx = idx - 1;
        let parent_key = node.keys.remove(left_idx);
        let parent_val = node.values.remove(left_idx);
        let donor_key = node.children[left_idx]
            .keys
            .pop()
            .expect("btree donor node keys empty despite invariant");
        let donor_val = node.children[left_idx]
            .values
            .pop()
            .expect("btree donor node values empty despite invariant");
        let donor_child = if !node.children[left_idx].children.is_empty() {
            Some(
                node.children[left_idx]
                    .children
                    .pop()
                    .expect("btree donor children empty despite check"),
            )
        } else {
            None
        };
        node.keys.insert(left_idx, donor_key);
        node.values.insert(left_idx, donor_val);
        node.children[idx].keys.insert(0, parent_key);
        node.children[idx].values.insert(0, parent_val);
        if let Some(child) = donor_child {
            node.children[idx].children.insert(0, child);
        }
    }

    fn borrow_from_right(&self, node: &mut BTreeNode<K, V>, idx: usize) {
        let right_idx = idx + 1;
        let parent_key = node.keys.remove(idx);
        let parent_val = node.values.remove(idx);
        let donor_key = node.children[right_idx].keys.remove(0);
        let donor_val = node.children[right_idx].values.remove(0);
        let donor_child = if !node.children[right_idx].children.is_empty() {
            Some(node.children[right_idx].children.remove(0))
        } else {
            None
        };
        node.keys.insert(idx, donor_key);
        node.values.insert(idx, donor_val);
        node.children[idx].keys.push(parent_key);
        node.children[idx].values.push(parent_val);
        if let Some(child) = donor_child {
            node.children[idx].children.push(child);
        }
    }

    /// Insert a child back into a parent, splitting it if it exceeds max_keys.
    /// This handles the case where ensure_child_has_minimum merges two min_keys
    /// children with a separator, producing 2*min_keys+1 keys which can exceed
    /// max_keys for odd-order B-trees (e.g., order 3: 1+1+1=3 > max_keys=2).
    fn maybe_split_child(&self, parent: &mut BTreeNode<K, V>, idx: usize, child: BTreeNode<K, V>) {
        if child.keys.len() <= self.max_keys() {
            parent.children.insert(idx, child);
            return;
        }
        let mid = child.keys.len() / 2;
        let mut left = child;
        let median_key = left.keys.remove(mid);
        let median_val = left.values.remove(mid);
        let right = if left.is_leaf() {
            BTreeNode::leaf(left.keys.split_off(mid), left.values.split_off(mid))
        } else {
            BTreeNode {
                keys: left.keys.split_off(mid),
                values: left.values.split_off(mid),
                children: left.children.split_off(mid + 1),
            }
        };
        // Parent cannot overflow here: if ensure_child_has_minimum merged (removing
        // 1 parent key), the split puts 1 key back, netting zero change. If it
        // borrowed, no overflow occurs in the child.
        parent.keys.insert(idx, median_key);
        parent.values.insert(idx, median_val);
        parent.children.insert(idx, left);
        parent.children.insert(idx + 1, right);
    }

    fn merge_nodes(
        left: BTreeNode<K, V>,
        parent_key: K,
        parent_val: V,
        right: BTreeNode<K, V>,
    ) -> BTreeNode<K, V> {
        let mut keys = left.keys;
        keys.push(parent_key);
        keys.extend(right.keys);
        let mut values = left.values;
        values.push(parent_val);
        values.extend(right.values);
        let mut children = left.children;
        children.extend(right.children);
        BTreeNode {
            keys,
            values,
            children,
        }
    }

    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.search(key).is_some()
    }

    #[must_use]
    pub fn min(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(|node| Self::find_min(node))
    }

    fn find_min(node: &BTreeNode<K, V>) -> (&K, &V) {
        if node.is_leaf() {
            (&node.keys[0], &node.values[0])
        } else {
            Self::find_min(&node.children[0])
        }
    }

    #[must_use]
    pub fn max(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(|node| Self::find_max(node))
    }

    fn find_max(node: &BTreeNode<K, V>) -> (&K, &V) {
        if node.is_leaf() {
            let last = node.keys.len() - 1;
            (&node.keys[last], &node.values[last])
        } else {
            let last = node.children.len() - 1;
            Self::find_max(&node.children[last])
        }
    }

    #[must_use]
    pub fn range<R>(&self, range: R) -> Vec<(&K, &V)>
    where
        R: std::ops::RangeBounds<K>,
    {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            Self::collect_range(root, &range, &mut results);
        }
        results
    }

    fn collect_range<'a, R>(node: &'a BTreeNode<K, V>, range: &R, results: &mut Vec<(&'a K, &'a V)>)
    where
        R: std::ops::RangeBounds<K>,
    {
        for i in 0..node.keys.len() {
            if !node.is_leaf() {
                Self::collect_range(&node.children[i], range, results);
            }
            if range.contains(&node.keys[i]) {
                results.push((&node.keys[i], &node.values[i]));
            }
        }
        if !node.is_leaf() {
            Self::collect_range(&node.children[node.children.len() - 1], range, results);
        }
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.root.as_ref().map_or(0, |node| Self::node_height(node))
    }

    fn node_height(node: &BTreeNode<K, V>) -> usize {
        if node.is_leaf() {
            1
        } else {
            1 + Self::node_height(&node.children[0])
        }
    }

    #[must_use]
    pub fn verify(&self) -> bool {
        self.verify_reason().is_ok()
    }

    fn verify_reason(&self) -> Result<(), String> {
        match self.root.as_ref() {
            None => Ok(()),
            Some(root) => {
                let h = Self::node_height(root);
                Self::verify_node(root, self.min_keys(), self.max_keys(), h, true)
            }
        }
    }

    fn verify_node(
        node: &BTreeNode<K, V>,
        min_keys: usize,
        max_keys: usize,
        expected_height: usize,
        is_root: bool,
    ) -> Result<(), String> {
        if node.keys.len() > max_keys {
            return Err(format!(
                "keys.len {} > max_keys {}",
                node.keys.len(),
                max_keys
            ));
        }
        // Root is exempt from minimum keys constraint (B-tree invariant)
        if !is_root && !node.is_leaf() && node.keys.len() < min_keys {
            return Err(format!(
                "non-root keys.len {} < min_keys {}",
                node.keys.len(),
                min_keys
            ));
        }
        if !node.children.is_empty() && node.children.len() != node.keys.len() + 1 {
            return Err(format!(
                "children {} != keys+1 {}",
                node.children.len(),
                node.keys.len() + 1
            ));
        }
        if node.is_leaf() && expected_height != 1 {
            return Err(format!("leaf height {} != 1", expected_height));
        }
        if !node.is_leaf() && expected_height <= 1 {
            return Err(format!("internal height {} <= 1", expected_height));
        }
        if !node.is_leaf() {
            for child in &node.children {
                Self::verify_node(child, min_keys, max_keys, expected_height - 1, false)?;
            }
        }
        Ok(())
    }
}

impl<K, V> Default for BTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + Clone, V: Clone> From<Vec<(K, V)>> for BTree<K, V> {
    fn from(pairs: Vec<(K, V)>) -> Self {
        let mut tree = Self::new();
        for (k, v) in pairs {
            tree.insert(k, v);
        }
        tree
    }
}
