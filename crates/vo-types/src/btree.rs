//! B-tree: balanced search tree with O(log n) insert, search, delete, and range queries.
//!
//! Maintains sorted key-value pairs with guaranteed logarithmic depth through
//! node splitting and merging. Internal nodes hold keys and child pointers;
//! leaf nodes hold keys and values.
//!
//! # Invariants
//! - Every node (except root) has at least `min_keys()` entries.
//! - Every node has at most `max_keys()` entries.
//! - All leaves are at the same depth.
//! - Internal node keys partition child subtrees: keys[i] < all keys in subtree[i+1].
//!
//! # Complexity
//! - `search`: O(log_b n)
//! - `insert`: O(log_b n)
//! - `delete`: O(log_b n)
//! - `range_scan`: O(k + log_b n) where k is the number of results

use serde::{Deserialize, Serialize};

const DEFAULT_ORDER: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTreeNode<K, V> {
    pub keys: Vec<K>,
    pub values: Vec<V>,
    pub children: Vec<BTreeNode<K, V>>,
}

impl<K, V> BTreeNode<K, V> {
    fn leaf(keys: Vec<K>, values: Vec<V>) -> Self {
        Self {
            keys,
            values,
            children: Vec::new(),
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn search_index(&self, key: &K) -> usize
    where
        K: Ord,
    {
        self.keys
            .iter()
            .position(|k| k >= key)
            .unwrap_or(self.keys.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BTree<K, V> {
    root: Option<BTreeNode<K, V>>,
    order: usize,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BTreeError {
    #[error("key not found")]
    KeyNotFound,
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
            let child = node.children.remove(idx);
            let result = self.insert_recursive(child, key, value);

            match result {
                InsertResult::Done(updated_child) => {
                    node.children.insert(idx, updated_child);
                    InsertResult::Done(node)
                }
                InsertResult::Updated(updated_child) => {
                    node.children.insert(idx, updated_child);
                    InsertResult::Done(node)
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

        let root = self
            .root
            .take()
            .expect("btree root missing after is_none check");
        let (updated_root, removed) = self.delete_recursive(root, key)?;
        self.len -= 1;

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
        let mut idx = node.search_index(key);
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
                node.children.insert(idx, updated_child);
                return Ok((node, removed_val));
            }

            if node.children[idx + 1].keys.len() > self.min_keys() {
                let (succ_key, succ_val, updated_child) =
                    self.remove_successor(node.children.remove(idx + 1))?;
                node.keys[idx] = succ_key;
                node.values[idx] = succ_val;
                node.children.insert(idx + 1, updated_child);
                return Ok((node, removed_val));
            }

            let left = node.children.remove(idx);
            let parent_key = node.keys.remove(idx);
            let parent_val = node.values.remove(idx);
            let right = node.children.remove(idx);
            let merged = Self::merge_nodes(left, parent_key, parent_val, right);
            let (updated, _) = self.delete_recursive(merged, key)?;
            node.children.insert(idx, updated);
            return Ok((node, removed_val));
        }

        if node.children[idx].keys.len() <= self.min_keys() {
            self.ensure_child_has_minimum(&mut node, idx);
            // After merge/borrow, the child index may have changed — re-search.
            idx = node.search_index(key);
        }

        let child = node.children.remove(idx);
        let (updated_child, removed) = self.delete_recursive(child, key)?;
        node.children.insert(idx, updated_child);
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
        match self.root.as_ref() {
            None => true,
            Some(root) => {
                let h = Self::node_height(root);
                // Root may have fewer than min_keys entries
                if root.keys.len() > self.max_keys() {
                    return false;
                }
                if !root.is_leaf() && root.children.len() != root.keys.len() + 1 {
                    return false;
                }
                if !root.is_leaf() {
                    for child in &root.children {
                        if !Self::verify_node(child, self.min_keys(), self.max_keys(), h - 1) {
                            return false;
                        }
                    }
                } else if h != 1 {
                    return false;
                }
                true
            }
        }
    }

    fn verify_node(
        node: &BTreeNode<K, V>,
        min_keys: usize,
        max_keys: usize,
        expected_height: usize,
    ) -> bool {
        if node.keys.len() > max_keys {
<<<<<<< HEAD
            return false;
        }
        if node.keys.len() < min_keys {
            return false;
        }
        if !node.children.is_empty() && node.children.len() != node.keys.len() + 1 {
            return false;
=======
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
>>>>>>> 7e356012 (style: apply consistent rustfmt formatting)
        }
        if node.is_leaf() && expected_height != 1 {
            return false;
        }
        if !node.is_leaf() && expected_height <= 1 {
            return false;
        }
        if !node.is_leaf() {
            for child in &node.children {
                if !Self::verify_node(child, min_keys, max_keys, expected_height - 1) {
                    return false;
                }
            }
        }
        true
    }
}

enum InsertResult<K, V> {
    Done(BTreeNode<K, V>),
    Updated(BTreeNode<K, V>),
    Split(BTreeNode<K, V>, K, V, BTreeNode<K, V>),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_is_empty() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn default_is_empty() {
        let tree: BTree<i32, i32> = BTree::default();
        assert!(tree.is_empty());
    }

    #[test]
    fn insert_single_element() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"a".to_string()));
    }

    #[test]
    fn search_missing_key_returns_none() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert_eq!(tree.search(&99), None);
    }

    #[test]
    fn search_empty_tree_returns_none() {
        let tree: BTree<i32, String> = BTree::new();
        assert_eq!(tree.search(&1), None);
    }

    #[test]
    fn insert_updates_existing_key() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        tree.insert(1, "b".to_string());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.search(&1), Some(&"b".to_string()));
    }

    #[test]
    fn insert_many_maintains_order() {
        let mut tree = BTree::new();
        for i in (0..50).rev() {
            tree.insert(i, format!("val_{i}"));
        }
        assert_eq!(tree.len(), 50);
        assert!(tree.verify());

        for i in 0..50 {
            assert_eq!(tree.search(&i), Some(&format!("val_{i}")));
        }
    }

    #[test]
    fn delete_existing_key() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        tree.insert(2, "b".to_string());
        tree.insert(3, "c".to_string());

        let removed = tree.delete(&2).unwrap();
        assert_eq!(removed, "b".to_string());
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.search(&2), None);
        assert!(tree.verify());
    }

    #[test]
    fn delete_missing_key_returns_error() {
        let mut tree = BTree::new();
        tree.insert(1, "a".to_string());
        assert!(matches!(tree.delete(&99), Err(BTreeError::KeyNotFound)));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn delete_from_empty_tree_returns_error() {
        let mut tree: BTree<i32, String> = BTree::new();
        assert!(matches!(tree.delete(&1), Err(BTreeError::KeyNotFound)));
    }

    #[test]
    fn delete_all_elements() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i);
        }
        for i in 0..20 {
            tree.delete(&i).unwrap();
        }
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn contains_key() {
        let mut tree = BTree::new();
        tree.insert(42, "answer");
        assert!(tree.contains(&42));
        assert!(!tree.contains(&1));
    }

    #[test]
    fn min_returns_smallest() {
        let mut tree = BTree::new();
        tree.insert(5, "e");
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(4, "d");
        tree.insert(2, "b");

        let (k, v) = tree.min().unwrap();
        assert_eq!(k, &1);
        assert_eq!(v, &"a");
    }

    #[test]
    fn max_returns_largest() {
        let mut tree = BTree::new();
        tree.insert(5, "e");
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(4, "d");
        tree.insert(2, "b");

        let (k, v) = tree.max().unwrap();
        assert_eq!(k, &5);
        assert_eq!(v, &"e");
    }

    #[test]
    fn min_max_on_empty_returns_none() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.min().is_none());
        assert!(tree.max().is_none());
    }

    #[test]
    fn range_query() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10);
        }

        let results = tree.range(5..15);
        assert_eq!(results.len(), 10);
        for (k, v) in &results {
            assert!(**k >= 5 && **k < 15);
            assert_eq!(**v, **k * 10);
        }
    }

    #[test]
    fn range_query_empty_result() {
        let mut tree = BTree::new();
        tree.insert(1, 10);
        tree.insert(5, 50);

        let results = tree.range(2..4);
        assert!(results.is_empty());
    }

    #[test]
    fn range_query_inclusive() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(3..=7);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn range_query_unbounded_start() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(..5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn range_query_unbounded_end() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        let results = tree.range(5..);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn height_increases_with_size() {
        let mut tree = BTree::with_order(4);
        assert_eq!(tree.height(), 0);

        for i in 0..10 {
            tree.insert(i, i);
        }
        assert!(tree.height() >= 2);
        assert!(tree.verify());
    }

    #[test]
    fn height_of_single_element() {
        let mut tree = BTree::new();
        tree.insert(1, "a");
        assert_eq!(tree.height(), 1);
    }

    #[test]
    fn verify_empty_tree() {
        let tree: BTree<i32, String> = BTree::new();
        assert!(tree.verify());
    }

    #[test]
    fn verify_after_inserts() {
        let mut tree = BTree::with_order(4);
        for i in 0..100 {
            tree.insert(i, i);
            assert!(tree.verify(), "tree invalid after inserting {i}");
        }
    }

    #[test]
    fn verify_after_deletes() {
        let mut tree = BTree::with_order(4);
        for i in 0..100 {
            tree.insert(i, i);
        }
        for i in (0..100).rev() {
            tree.delete(&i).unwrap();
            assert!(tree.verify(), "tree invalid after deleting {i}");
        }
    }

    #[test]
    fn delete_triggers_node_merge() {
        let mut tree = BTree::with_order(4);
        for i in 0..20 {
            tree.insert(i, i);
        }
        for i in 0..15 {
            tree.delete(&i).unwrap();
        }
        assert_eq!(tree.len(), 5);
        assert!(tree.verify());
        for i in 15..20 {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn delete_triggers_borrow_from_left() {
        let mut tree = BTree::with_order(4);
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&9).unwrap();
        tree.delete(&8).unwrap();
        tree.delete(&7).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 7);
    }

    #[test]
    fn delete_triggers_borrow_from_right() {
        let mut tree = BTree::with_order(4);
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&0).unwrap();
        tree.delete(&1).unwrap();
        tree.delete(&2).unwrap();
        assert!(tree.verify());
        assert_eq!(tree.len(), 7);
    }

    #[test]
    fn from_vec_builds_correct_tree() {
        let pairs = vec![(3, "c"), (1, "a"), (2, "b")];
        let tree: BTree<i32, &str> = BTree::from(pairs);
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.search(&1), Some(&"a"));
        assert_eq!(tree.search(&2), Some(&"b"));
        assert_eq!(tree.search(&3), Some(&"c"));
        assert!(tree.verify());
    }

    #[test]
    fn root_split_creates_new_root() {
        let mut tree = BTree::with_order(3);
        tree.insert(1, "a");
        tree.insert(2, "b");
        assert_eq!(tree.height(), 1);

        tree.insert(3, "c");
        assert_eq!(tree.height(), 2);
        assert!(tree.verify());
    }

    #[test]
    fn sequential_insert_delete_cycle() {
        let mut tree = BTree::with_order(4);
        for round in 0..5 {
            for i in 0..50 {
                tree.insert(i, format!("r{round}_v{i}"));
            }
            assert!(tree.verify());
            for i in 0..50 {
                tree.delete(&i).unwrap();
            }
            assert!(tree.is_empty());
        }
    }

    #[test]
    fn string_keys() {
        let mut tree = BTree::new();
        tree.insert("banana".to_string(), 2);
        tree.insert("apple".to_string(), 1);
        tree.insert("cherry".to_string(), 3);

        assert_eq!(tree.min().unwrap().0, &"apple".to_string());
        assert_eq!(tree.max().unwrap().0, &"cherry".to_string());
        assert!(tree.verify());
    }

    #[test]
    fn serde_roundtrip() {
        let mut tree = BTree::new();
        for i in 0..20 {
            tree.insert(i, i * 10);
        }
        let json = serde_json::to_string(&tree).unwrap();
        let back: BTree<i32, i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(tree.len(), back.len());
        for i in 0..20 {
            assert_eq!(back.search(&i), Some(&(i * 10)));
        }
        assert!(back.verify());
    }

    #[test]
    fn btree_node_leaf_is_leaf() {
        let node = BTreeNode::leaf(vec![1, 2], vec!["a", "b"]);
        assert!(node.is_leaf());
    }

    #[test]
    fn btree_node_search_index() {
        let node = BTreeNode::leaf(vec![1, 3, 5], vec!["a", "b", "c"]);
        assert_eq!(node.search_index(&0), 0);
        assert_eq!(node.search_index(&1), 0);
        assert_eq!(node.search_index(&2), 1);
        assert_eq!(node.search_index(&3), 1);
        assert_eq!(node.search_index(&4), 2);
        assert_eq!(node.search_index(&5), 2);
        assert_eq!(node.search_index(&6), 3);
    }

    #[test]
    fn with_order_custom() {
        let tree = BTree::<i32, i32>::with_order(5);
        assert_eq!(tree.max_keys(), 4);
        assert_eq!(tree.min_keys(), 2);
    }

    #[test]
    fn large_scale_insert_and_search() {
        let mut tree = BTree::with_order(32);
        let n = 1000;
        for i in 0..n {
            tree.insert(i, i * 2);
        }
        assert_eq!(tree.len(), n);
        assert!(tree.verify());
        for i in 0..n {
            assert_eq!(tree.search(&i), Some(&(i * 2)));
        }
    }

    #[test]
    fn large_scale_delete_and_verify() {
        let mut tree = BTree::with_order(32);
        let n = 500;
        for i in 0..n {
            tree.insert(i, i);
        }
        for i in (0..n).step_by(2) {
            tree.delete(&i).unwrap();
        }
        assert_eq!(tree.len(), n / 2);
        assert!(tree.verify());
        for i in (0..n).step_by(2) {
            assert!(tree.search(&i).is_none());
        }
        for i in (1..n).step_by(2) {
            assert_eq!(tree.search(&i), Some(&i));
        }
    }

    #[test]
    fn delete_root_key_when_root_is_leaf() {
        let mut tree = BTree::new();
        tree.insert(1, "a");
        tree.delete(&1).unwrap();
        assert!(tree.is_empty());
        assert_eq!(tree.height(), 0);
    }

    #[test]
    fn interleaved_insert_delete() {
        let mut tree = BTree::with_order(4);
        tree.insert(10, 10);
        tree.insert(20, 20);
        tree.insert(5, 5);
        tree.delete(&10).unwrap();
        tree.insert(15, 15);
        tree.delete(&5).unwrap();
        tree.insert(25, 25);
        tree.delete(&20).unwrap();

        assert!(tree.verify());
        assert_eq!(tree.len(), 2);
        assert!(tree.contains(&15));
        assert!(tree.contains(&25));
    }

    #[test]
    fn range_after_deletes() {
        let mut tree = BTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }
        tree.delete(&3).unwrap();
        tree.delete(&7).unwrap();

        let results = tree.range(2..=8);
        let keys: Vec<&i32> = results.iter().map(|(k, _)| k).copied().collect();
        assert_eq!(keys, vec![&2, &4, &5, &6, &8]);
    }

    #[test]
    fn insert_delete_interleaved_stress() {
        let mut tree = BTree::with_order(4);
        let mut values: Vec<i32> = Vec::new();

        for i in 0..200 {
            tree.insert(i, i);
            values.push(i);
            assert!(tree.verify());
        }

        let mut rng_seed = 42u64;
        for _ in 0..100 {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let idx = (rng_seed >> 33) as usize % values.len();
            let val = values.remove(idx);
            tree.delete(&val).unwrap();
            assert!(tree.verify());
        }

        assert_eq!(tree.len(), 100);
        for &v in &values {
            assert_eq!(tree.search(&v), Some(&v));
        }
    }
}
