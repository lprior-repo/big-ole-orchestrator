use super::node::{BTreeNode, InsertResult};

impl<K: Ord + Clone, V: Clone> super::BTree<K, V> {
    pub(crate) fn insert_recursive(
        &self,
        mut node: BTreeNode<K, V>,
        key: K,
        value: V,
    ) -> InsertResult<K, V> {
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
}
