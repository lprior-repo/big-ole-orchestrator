use super::node::{BTreeNode, BTreeError};

impl<K: Ord + Clone, V: Clone> super::BTree<K, V> {
    pub(crate) fn delete_recursive(
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
    pub(crate) fn maybe_split_child(
        &self,
        parent: &mut BTreeNode<K, V>,
        idx: usize,
        child: BTreeNode<K, V>,
    ) {
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

    pub(crate) fn merge_nodes(
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
}
