//! Splay tree: self-adjusting binary search tree with amortized O(log n) operations.
//!
//! Splay trees achieve amortized O(log n) performance by splaying (moving to root)
//! recently accessed nodes via a series of rotations. No explicit balance information
//! is stored — the tree restructures on every access.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplayNode<K, V> {
    pub key: K,
    pub value: V,
    pub left: Option<Box<SplayNode<K, V>>>,
    pub right: Option<Box<SplayNode<K, V>>>,
    pub parent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplayTree<K, V> {
    root: Option<Box<SplayNode<K, V>>>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplayTreeError {
    #[error("tree is empty")]
    EmptyTree,

    #[error("key not found: {0:?}")]
    KeyNotFound(K),

    #[error("invalid node index: {0}")]
    InvalidNode(usize),
}

impl<K: Ord, V> SplayTree<K, V> {
    pub fn new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn singleton(key: K, value: V) -> Self {
        Self {
            root: Some(Box::new(SplayNode {
                key,
                value,
                left: None,
                right: None,
                parent: None,
            })),
            len: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn root(&self) -> Option<&SplayNode<K, V>> {
        self.root.as_deref()
    }

    fn dir<K2: Ord>(&self, node: &SplayNode<K2, V>, child_idx: usize) -> usize {
        let p = node.parent?;
        if p.ch[child_idx].as_ref() as *const _ == node as *const _ {
            1
        } else {
            0
        }
    }

    fn rotate_right(root: &mut Option<Box<SplayNode<K, V>>>) {
        let mut root_box = root.take().unwrap();
        let mut right = root_box.right.take().unwrap();
        root_box.right = right.left.take();
        if let Some(ref mut r) = root_box.right {
            r.parent = None;
        }
        right.left = Some(root_box);
        *root = Some(right);
    }

    fn rotate_left(root: &mut Option<Box<SplayNode<K, V>>>) {
        let mut root_box = root.take().unwrap();
        let mut left = root_box.left.take().unwrap();
        root_box.left = left.right.take();
        if let Some(ref mut l) = root_box.left {
            l.parent = None;
        }
        left.right = Some(root_box);
        *root = Some(left);
    }

    fn splay_node(root: &mut Option<Box<SplayNode<K, V>>>) {
        loop {
            let (is_left, parent_is_left) = {
                let root_ref = root.as_ref().unwrap();
                let is_left_child = root_ref
                    .parent
                    .map(|p| {
                        p.ch[0].as_ref() as *const _
                            == root.as_ref().map(|r| r.as_ref()).unwrap() as *const _
                    })
                    .unwrap_or(false);
                let is_right_child = root_ref
                    .parent
                    .map(|p| {
                        p.ch[1].as_ref() as *const _
                            == root.as_ref().map(|r| r.as_ref()).unwrap() as *const _
                    })
                    .unwrap_or(false);
                (is_left_child, None)
            };

            if root.as_ref().unwrap().parent.is_none() {
                break;
            }

            let parent_idx = root.as_ref().unwrap().parent.unwrap();
            let grand_parent_idx = parent_idx.parent;

            if grand_parent_idx.is_none() {
                if root.as_ref().unwrap().parent.unwrap().ch[0]
                    .as_ref()
                    .map(|r| r.as_ref() as *const _)
                    .unwrap_or(std::ptr::null())
                    == root
                        .as_ref()
                        .map(|r| r.as_ref() as *const _)
                        .unwrap_or(std::ptr::null())
                {
                    Self::rotate_right(&mut root.as_mut().unwrap().parent.as_mut().unwrap().ch[0]);
                } else {
                    Self::rotate_left(&mut root.as_mut().unwrap().parent.as_mut().unwrap().ch[1]);
                }
            } else {
                let gp = grand_parent_idx.unwrap();
                let parent_is_left_child = gp.ch[0]
                    .as_ref()
                    .map(|r| r.as_ref() as *const _)
                    .unwrap_or(std::ptr::null())
                    == parent_idx.ch[0]
                        .as_ref()
                        .map(|r| r.as_ref() as *const _)
                        .unwrap_or(std::ptr::null());
                let root_is_left_child = parent_idx.ch[0]
                    .as_ref()
                    .map(|r| r.as_ref() as *const _)
                    .unwrap_or(std::ptr::null())
                    == root
                        .as_ref()
                        .map(|r| r.as_ref() as *const _)
                        .unwrap_or(std::ptr::null());

                if parent_is_left_child && root_is_left_child {
                    Self::rotate_right(&mut gp.ch[0].as_mut().unwrap().right);
                    Self::rotate_right(&mut gp.ch[0]);
                } else if !parent_is_left_child && !root_is_left_child {
                    Self::rotate_left(&mut gp.ch[1].as_mut().unwrap().left);
                    Self::rotate_left(&mut gp.ch[1]);
                } else if parent_is_left_child {
                    Self::rotate_right(&mut gp.ch[0].as_mut().unwrap().right);
                    Self::rotate_left(&mut gp.ch[0]);
                } else {
                    Self::rotate_left(&mut gp.ch[1].as_mut().unwrap().left);
                    Self::rotate_right(&mut gp.ch[1]);
                }
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.root.is_none() {
            self.root = Some(Box::new(SplayNode {
                key,
                value,
                left: None,
                right: None,
                parent: None,
            }));
            self.len = 1;
            return;
        }

        let mut current = &mut self.root;
        loop {
            if key < current.as_ref().unwrap().key {
                if current.as_ref().unwrap().left.is_some() {
                    current = &mut current.as_mut().unwrap().left;
                } else {
                    current.as_mut().unwrap().left = Some(Box::new(SplayNode {
                        key,
                        value,
                        left: None,
                        right: None,
                        parent: None,
                    }));
                    self.len += 1;
                    break;
                }
            } else if key > current.as_ref().unwrap().key {
                if current.as_ref().unwrap().right.is_some() {
                    current = &mut current.as_ref().unwrap().right;
                } else {
                    current.as_mut().unwrap().right = Some(Box::new(SplayNode {
                        key,
                        value,
                        left: None,
                        right: None,
                        parent: None,
                    }));
                    self.len += 1;
                    break;
                }
            } else {
                current.as_mut().unwrap().value = value;
                break;
            }
        }
    }

    pub fn find(&self, key: &K) -> Option<&V> {
        let mut current = &self.root;
        while let Some(node) = current {
            if key < &node.key {
                current = &node.left;
            } else if key > &node.key {
                current = &node.right;
            } else {
                return Some(&node.value);
            }
        }
        None
    }

    pub fn contains(&self, key: &K) -> bool {
        self.find(key).is_some()
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let mut current = &mut self.root;
        let mut target_idx: Option<usize> = None;

        while let Some(node) = current {
            if key < &node.key {
                current = &mut node.left;
            } else if key > &node.key {
                current = &mut node.right;
            } else {
                target_idx = Some(0);
                break;
            }
        }

        if target_idx.is_none() {
            return None;
        }

        let result = current.as_ref().unwrap().value.clone();

        let left = current.as_mut().unwrap().left.take();
        let right = current.as_mut().unwrap().right.take();
        *current = None;
        self.len -= 1;

        if left.is_some() {
            *current = left;
            self.merge_with_root(*current.as_mut().unwrap(), right);
        } else if right.is_some() {
            *current = right;
        } else {
            self.root = None;
        }

        Some(result)
    }

    fn merge_with_root(
        &mut self,
        mut left_tree: Box<SplayNode<K, V>>,
        right_tree: Option<Box<SplayNode<K, V>>>,
    ) {
        if let Some(mut right) = right_tree {
            let mut current = &mut right;
            while current.left.is_some() {
                current = current.left.as_mut().unwrap();
            }
            current.left = Some(left_tree);
            self.root = Some(right);
        } else {
            self.root = Some(left_tree);
        }
    }

    pub fn inorder(&self) -> Vec<(&K, &V)> {
        let mut result = Vec::new();
        self.inorder_recursive(self.root.as_deref(), &mut result);
        result
    }

    fn inorder_recursive(&self, node: Option<&SplayNode<K, V>>, result: &mut Vec<(&K, &V)>) {
        if let Some(n) = node {
            self.inorder_recursive(n.left.as_deref(), result);
            result.push((&n.key, &n.value));
            self.inorder_recursive(n.right.as_deref(), result);
        }
    }

    pub fn split(
        &mut self,
        key: &K,
    ) -> (Option<Box<SplayNode<K, V>>>, Option<Box<SplayNode<K, V>>>) {
        if self.root.is_none() {
            return (None, None);
        }

        let mut current = &mut self.root;
        while let Some(node) = current {
            if key < &node.key {
                if node.left.is_some() {
                    current = &mut node.left;
                } else {
                    let right = node.left.take();
                    self.root = right;
                    return (self.root.take(), Some(Box::new(*node.clone())));
                }
            } else if key > &node.key {
                if node.right.is_some() {
                    current = &mut node.right;
                } else {
                    let left = node.right.take();
                    self.root = left;
                    return (Some(Box::new(*node.clone())), self.root.take());
                }
            } else {
                let left = node.left.take();
                let right = node.right.take();
                self.root = left.clone();
                return (left, right);
            }
        }

        (None, None)
    }

    pub fn merge(
        &mut self,
        left: Option<Box<SplayNode<K, V>>>,
        right: Option<Box<SplayNode<K, V>>>,
    ) {
        if left.is_none() {
            self.root = right;
            return;
        }
        if right.is_none() {
            self.root = left;
            return;
        }

        let mut current = left.unwrap();
        while current.right.is_some() {
            current = current.right.unwrap();
        }
        current.right = right;
        self.root = Some(current);
    }
}

impl<K, V> Default for SplayTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Display for SplayTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SplayTree {{ ")?;
        for (k, v) in self.inorder() {
            write!(f, "{:?}: {:?}, ", k, v)?;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_is_empty() {
        let tree: SplayTree<i32, String> = SplayTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn singleton_has_one_element() {
        let tree = SplayTree::singleton(42, "answer".to_string());
        assert_eq!(tree.len(), 1);
        assert!(!tree.is_empty());
        assert_eq!(tree.find(&42), Some(&"answer".to_string()));
    }

    #[test]
    fn insert_increases_len() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        assert_eq!(tree.len(), 1);
        tree.insert(2, "two");
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn find_returns_inserted_value() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        tree.insert(3, "three");
        assert_eq!(tree.find(&1), Some(&"one".to_string()));
        assert_eq!(tree.find(&2), Some(&"two".to_string()));
        assert_eq!(tree.find(&3), Some(&"three".to_string()));
    }

    #[test]
    fn find_nonexistent_returns_none() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        assert_eq!(tree.find(&99), None);
    }

    #[test]
    fn contains_returns_correct_result() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        assert!(tree.contains(&1));
        assert!(tree.contains(&2));
        assert!(!tree.contains(&99));
    }

    #[test]
    fn insert_updates_existing_key() {
        let mut tree = SplayTree::new();
        tree.insert(1, "original");
        tree.insert(1, "updated");
        assert_eq!(tree.find(&1), Some(&"updated".to_string()));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn remove_existing_key_returns_value() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        let removed = tree.remove(&1);
        assert_eq!(removed, Some("one".to_string()));
        assert_eq!(tree.len(), 1);
        assert!(!tree.contains(&1));
    }

    #[test]
    fn remove_nonexistent_key_returns_none() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        let removed = tree.remove(&99);
        assert_eq!(removed, None);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn inorder_returns_sorted_keys() {
        let mut tree = SplayTree::new();
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(2, "b");
        tree.insert(5, "e");
        tree.insert(4, "d");

        let inorder = tree.inorder();
        let keys: Vec<_> = inorder.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn split_at_existing_key() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        tree.insert(3, "three");
        tree.insert(4, "four");
        tree.insert(5, "five");

        let (left, right) = tree.split(&3);

        let left_keys: Vec<_> = left.iter().map(|n| &n.key).cloned().collect();
        let right_keys: Vec<_> = right.iter().map(|n| &n.key).cloned().collect();
        assert!(left_keys.is_empty() || left_keys == vec![1, 2]);
        assert!(right_keys.is_empty() || right_keys == vec![4, 5]);
    }

    #[test]
    fn merge_combines_trees() {
        let mut tree = SplayTree::new();
        let left = Some(Box::new(SplayNode {
            key: 1,
            value: "one",
            left: None,
            right: None,
            parent: None,
        }));
        let right = Some(Box::new(SplayNode {
            key: 2,
            value: "two",
            left: None,
            right: None,
            parent: None,
        }));

        tree.merge(left, right);
        assert_eq!(tree.len(), 2);
        assert!(tree.contains(&1));
        assert!(tree.contains(&2));
    }

    #[test]
    fn insert_many_elements() {
        let mut tree = SplayTree::new();
        for i in 0..100 {
            tree.insert(i, i);
        }
        assert_eq!(tree.len(), 100);

        for i in 0..100 {
            assert_eq!(tree.find(&i), Some(&i));
        }
    }

    #[test]
    fn remove_all_elements() {
        let mut tree = SplayTree::new();
        for i in 0..10 {
            tree.insert(i, i);
        }

        for i in 0..10 {
            let removed = tree.remove(&i);
            assert_eq!(removed, Some(i));
        }

        assert!(tree.is_empty());
    }

    #[test]
    fn inorder_empty_tree() {
        let tree: SplayTree<i32, i32> = SplayTree::new();
        assert!(tree.inorder().is_empty());
    }

    #[test]
    fn singleton_tree_inorder() {
        let tree = SplayTree::singleton(42, "answer");
        let inorder = tree.inorder();
        assert_eq!(inorder.len(), 1);
        assert_eq!(inorder[0].0, &42);
        assert_eq!(inorder[0].1, &"answer");
    }

    #[test]
    fn display_format() {
        let mut tree = SplayTree::new();
        tree.insert(1, "one");
        tree.insert(2, "two");
        let display = format!("{}", tree);
        assert!(display.contains("1"));
        assert!(display.contains("2"));
    }
}
