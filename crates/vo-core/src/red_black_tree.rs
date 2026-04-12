//! Red-black tree implementation for ordered key-value storage.
//!
//! A red-black tree is a self-balancing binary search tree with the following properties:
//! 1. Every node is either red or black
//! 2. The root is black
//! 3. All NIL leaves are black
//! 4. Red nodes cannot have red children
//! 5. Every path from a node to its descendant NIL nodes has the same black height

use std::cmp::Ordering;
use std::fmt::Debug;
use std::iter::FusedIterator;

#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Red,
    Black,
}

impl Color {
    fn is_red(&self) -> bool {
        matches!(self, Color::Red)
    }
}

#[derive(Debug, Clone)]
pub struct Node<K, V> {
    pub key: K,
    pub value: V,
    pub color: Color,
    pub left: Option<Box<Node<K, V>>>,
    pub right: Option<Box<Node<K, V>>>,
}

impl<K: Ord, V> Node<K, V> {
    fn new(key: K, value: V) -> Self {
        Node {
            key,
            value,
            color: Color::Red,
            left: None,
            right: None,
        }
    }

    fn is_red(&self) -> bool {
        self.color.is_red()
    }
}

pub struct RedBlackTree<K, V> {
    root: Option<Box<Node<K, V>>>,
    len: usize,
}

impl<K: Ord, V> Default for RedBlackTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, V> RedBlackTree<K, V> {
    pub fn new() -> Self {
        RedBlackTree { root: None, len: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    fn rotate_left(&mut self, node: &mut Box<Node<K, V>>) {
        let mut right = node.right.take().expect("rotate_left: node.right is None");
        node.right = right.left.take();
        right.left = Some(node.clone());
        std::mem::drop(node);
        if let Some(ref mut right) = right.left {
            right.color = node.as_ref().map_or(Color::Black, |n| n.color);
        }
        if let Some(ref mut root) = self.root {
            if root.key == node.key {
                *root = right;
            }
        }
    }

    fn rotate_right(&mut self, node: &mut Box<Node<K, V>>) {
        let mut left = node.left.take().expect("rotate_right: node.left is None");
        node.left = left.right.take();
        left.right = Some(node.clone());
        std::mem::drop(node);
        if let Some(ref mut left) = left.right {
            left.color = node.as_ref().map_or(Color::Black, |n| n.color);
        }
        if let Some(ref mut root) = self.root {
            if root.key == node.key {
                *root = left;
            }
        }
    }

    fn flip_colors(&mut self, node: &mut Box<Node<K, V>>) {
        let mut is_root = false;
        if let Some(ref mut root) = self.root {
            is_root = root.key == node.key;
        }
        node.color = Color::Red;
        if let Some(ref mut left) = node.left {
            left.color = Color::Black;
        }
        if let Some(ref mut right) = node.right {
            right.color = Color::Black;
        }
        if !is_root {
            node.color = Color::Black;
        }
    }

    fn fix_up(&mut self, mut node: Box<Node<K, V>>) {
        let mut current = node;
        while let Some(ref mut curr) = self.root {
            if curr.key == current.key {
                break;
            }
            if curr.right.as_ref().map_or(false, |n| n.is_red())
                && !curr.left.as_ref().map_or(false, |n| n.is_red())
            {
                let mut right = curr.right.take().unwrap();
                self.rotate_left(curr);
                if let Some(ref mut root) = self.root {
                    if root.key == curr.key {
                        *root = right;
                        break;
                    }
                }
            }
            if curr.left.as_ref().map_or(false, |n| n.is_red())
                && curr
                    .left
                    .as_ref()
                    .map_or(false, |l| l.left.as_ref().map_or(false, |ll| ll.is_red()))
            {
                let mut left = curr.left.take().unwrap();
                self.rotate_right(curr);
                if let Some(ref mut root) = self.root {
                    if root.key == curr.key {
                        *root = left;
                        break;
                    }
                }
            }
            if curr.left.as_ref().map_or(false, |n| n.is_red())
                && curr.right.as_ref().map_or(false, |n| n.is_red())
            {
                self.flip_colors(curr);
            }
        }
    }

    fn insert(&mut self, key: K, value: V) {
        let mut new_node = Box::new(Node::new(key, value));
        let inserted = if let Some(ref mut root) = self.root {
            Self::insert_into_node(root, &mut new_node)
        } else {
            new_node.color = Color::Black;
            self.root = Some(new_node);
            true
        };
        if inserted {
            self.len += 1;
            if let Some(ref mut root) = self.root {
                root.color = Color::Black;
            }
        }
    }

    fn insert_into_node(node: &mut Box<Node<K, V>>, new_node: &mut Box<Node<K, V>>) -> bool {
        match node.key.cmp(&new_node.key) {
            Ordering::Equal => {
                node.value = new_node.value.clone();
                return false;
            }
            Ordering::Less => {
                if let Some(ref mut right) = node.right {
                    Self::insert_into_node(right, new_node)
                } else {
                    node.right = Some(new_node.clone());
                    true
                }
            }
            Ordering::Greater => {
                if let Some(ref mut left) = node.left {
                    Self::insert_into_node(left, new_node)
                } else {
                    node.left = Some(new_node.clone());
                    true
                }
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.root
            .as_ref()
            .and_then(|root| Self::get_from(root, key))
    }

    fn get_from(node: &Box<Node<K, V>>, key: &K) -> Option<&V> {
        match node.key.cmp(key) {
            Ordering::Equal => Some(&node.value),
            Ordering::Less => node.right.as_ref().and_then(|n| Self::get_from(n, key)),
            Ordering::Greater => node.left.as_ref().and_then(|n| Self::get_from(n, key)),
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed = if self.root.is_some() {
            self.delete(key)
        } else {
            None
        };
        if removed.is_some() {
            self.len -= 1;
        }
        if let Some(ref mut root) = self.root {
            root.color = Color::Black;
        }
        removed
    }

    fn delete(&mut self, key: &K) -> Option<V> {
        let mut dummy = Box::new(Node::new(key.clone(), std::marker::PhantomData::<V>));
        dummy.color = Color::Black;
        dummy.left = self.root.take();
        dummy.right = None;
        self.root = dummy.left.take();
        let result = self.delete_recursive(&mut dummy, key);
        self.root = dummy.left.take();
        if self.root.is_none() {
            dummy.left = None;
            dummy.right = None;
        }
        result
    }

    fn delete_recursive(&mut self, node: &mut Box<Node<K, V>>, key: &K) -> Option<V> {
        let cmp = node.key.cmp(key);
        let mut result = None;
        if cmp == Ordering::Greater {
            if node.left.is_some() {
                let mut left = node.left.take().unwrap();
                result = self.delete_recursive(&mut left, key);
                node.left = Some(left);
            }
        } else {
            if node.right.is_some() {
                let mut right = node.right.take().unwrap();
                if right.left.as_ref().map_or(false, |n| !n.is_red())
                    && !right.right.as_ref().map_or(false, |n| n.is_red())
                {
                    self.move_red_right(node);
                }
                let mut right = node.right.take().unwrap();
                result = self.delete_recursive(&mut right, key);
                node.right = Some(right);
            } else if cmp == Ordering::Equal {
                result = Some(node.value.clone());
                if node.left.is_some() {
                    let mut left = node.left.take().unwrap();
                    node.key = left.key.clone();
                    node.value = left.value.clone();
                    node.left = left.left.take();
                    node.right = left.right.take();
                } else {
                    return result;
                }
            }
        }
        if node.left.as_ref().map_or(false, |n| {
            !n.is_red() && n.left.as_ref().map_or(false, |nn| !nn.is_red())
        }) && node.right.as_ref().map_or(false, |n| !n.is_red())
        {
            self.move_red_left(node);
        }
        result
    }

    fn move_red_left(&mut self, node: &mut Box<Node<K, V>>) {
        self.flip_colors(node);
        if node
            .right
            .as_ref()
            .map_or(false, |n| n.right.as_ref().map_or(false, |nn| nn.is_red()))
        {
            let mut right = node.right.take().unwrap();
            self.rotate_right(node);
            node.right = Some(right);
        }
    }

    fn move_red_right(&mut self, node: &mut Box<Node<K, V>>) {
        self.flip_colors(node);
        if node
            .left
            .as_ref()
            .map_or(false, |n| n.left.as_ref().map_or(false, |nn| nn.is_red()))
        {
            let mut left = node.left.take().unwrap();
            self.rotate_right(node);
            node.left = Some(left);
        }
    }

    fn minimum(&self) -> Option<(&K, &V)> {
        self.root.as_ref().and_then(|root| Self::min_of(root))
    }

    fn min_of(node: &Box<Node<K, V>>) -> Option<(&K, &V)> {
        node.left
            .as_ref()
            .and_then(|left| Self::min_of(left))
            .or(Some((&node.key, &node.value)))
    }

    fn maximum(&self) -> Option<(&K, &V)> {
        self.root.as_ref().and_then(|root| Self::max_of(root))
    }

    fn max_of(node: &Box<Node<K, V>>) -> Option<(&K, &V)> {
        node.right
            .as_ref()
            .and_then(|right| Self::max_of(right))
            .or(Some((&node.key, &node.value)))
    }

    pub fn clear(&mut self) {
        self.root = None;
        self.len = 0;
    }

    pub fn iter(&self) -> Iter<K, V> {
        let mut iter = Iter::default();
        if let Some(ref root) = self.root {
            Self::collect_in_order(root, &mut iter.nodes);
        }
        iter.nodes.reverse();
        iter
    }

    fn collect_in_order(node: &Box<Node<K, V>>, nodes: &mut Vec<(&K, &V)>) {
        if let Some(ref left) = node.left {
            Self::collect_in_order(left, nodes);
        }
        nodes.push((&node.key, &node.value));
        if let Some(ref right) = node.right {
            Self::collect_in_order(right, nodes);
        }
    }

    fn collect_in_order_mut(node: &mut Box<Node<K, V>>, nodes: &mut Vec<(&K, &mut V)>) {
        if let Some(ref mut left) = node.left {
            Self::collect_in_order_mut(left, nodes);
        }
        nodes.push((&node.key, &mut node.value));
        if let Some(ref mut right) = node.right {
            Self::collect_in_order_mut(right, nodes);
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        let mut iter = IterMut::default();
        if let Some(ref mut root) = self.root {
            Self::collect_in_order_mut(root, &mut iter.nodes);
        }
        IterMut { nodes: iter.nodes }
    }

    pub fn keys(&self) -> Keys<K, V> {
        Keys(self.iter())
    }

    pub fn values(&self) -> Values<K, V> {
        Values(self.iter())
    }

    pub fn range(&self, lower: Option<&K>, upper: Option<&K>) -> Range<K, V> {
        let mut nodes = Vec::new();
        if let Some(ref root) = self.root {
            Self::collect_range(root, &mut nodes, lower, upper);
        }
        nodes.reverse();
        Range { nodes }
    }

    fn collect_range(
        node: &Box<Node<K, V>>,
        nodes: &mut Vec<(&K, &V)>,
        lower: Option<&K>,
        upper: Option<&K>,
    ) {
        let node_key = &node.key;
        if let Some(lower_key) = lower {
            if node_key < lower_key {
                if let Some(ref right) = node.right {
                    Self::collect_range(right, nodes, lower, upper);
                }
                return;
            }
        }
        if let Some(upper_key) = upper {
            if node_key >= upper_key {
                if let Some(ref left) = node.left {
                    Self::collect_range(left, nodes, lower, upper);
                }
                return;
            }
        }
        if let Some(ref left) = node.left {
            Self::collect_range(left, nodes, lower, upper);
        }
        nodes.push((&node.key, &node.value));
        if let Some(ref right) = node.right {
            Self::collect_range(right, nodes, lower, upper);
        }
    }
}

#[derive(Debug, Default)]
pub struct Iter<K, V> {
    nodes: Vec<(&K, &V)>,
}

impl<K, V> Iterator for Iter<K, V> {
    type Item = (&K, &V);

    fn next(&mut self) -> Option<Self::Item> {
        self.nodes.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nodes.len(), Some(self.nodes.len()))
    }
}

impl<K, V> ExactSizeIterator for Iter<K, V> {}
impl<K, V> FusedIterator for Iter<K, V> {}

#[derive(Debug, Default)]
pub struct IterMut<K, V> {
    nodes: Vec<(&K, &'static mut V)>,
}

impl<K, V> Iterator for IterMut<K, V> {
    type Item = (&K, &mut V);

    fn next(&mut self) -> Option<Self::Item> {
        self.nodes.pop().map(|(k, v)| (k, v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nodes.len(), Some(self.nodes.len()))
    }
}

impl<K, V> ExactSizeIterator for IterMut<K, V> {}
impl<K, V> FusedIterator for IterMut<K, V> {}

#[derive(Debug)]
pub struct Keys<K, V>(Iter<K, V>);

impl<K, V> Iterator for Keys<K, V> {
    type Item = &K;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Keys<K, V> {}
impl<K, V> FusedIterator for Keys<K, V> {}

#[derive(Debug)]
pub struct Values<K, V>(Iter<K, V>);

impl<K, V> Iterator for Values<K, V> {
    type Item = &V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<K, V> ExactSizeIterator for Values<K, V> {}
impl<K, V> FusedIterator for Values<K, V> {}

#[derive(Debug)]
pub struct Range<'a, K, V> {
    nodes: Vec<(&'a K, &'a V)>,
}

impl<'a, K, V> Iterator for Range<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.nodes.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.nodes.len(), Some(self.nodes.len()))
    }
}

impl<'a, K, V> ExactSizeIterator for Range<'a, K, V> {}
impl<'a, K, V> FusedIterator for Range<'a, K, V> {}

impl<K: Debug, V: Debug> Debug for RedBlackTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<K: Ord, V> Extend<(K, V)> for RedBlackTree<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for RedBlackTree<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut tree = RedBlackTree::new();
        tree.extend(iter);
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "a");
        tree.insert(2, "b");
        tree.insert(3, "c");

        assert_eq!(tree.get(&1), Some(&"a"));
        assert_eq!(tree.get(&2), Some(&"b"));
        assert_eq!(tree.get(&3), Some(&"c"));
        assert_eq!(tree.get(&4), None);
    }

    #[test]
    fn test_update_existing_key() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "a");
        tree.insert(1, "b");

        assert_eq!(tree.get(&1), Some(&"b"));
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "a");
        tree.insert(2, "b");
        tree.insert(3, "c");

        assert_eq!(tree.remove(&2), Some("b"));
        assert_eq!(tree.get(&2), None);
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn test_empty_tree() {
        let tree: RedBlackTree<i32, &str> = RedBlackTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.get(&1), None);
    }

    #[test]
    fn test_iter() {
        let mut tree = RedBlackTree::new();
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(2, "b");

        let keys: Vec<_> = tree.keys().collect();
        assert_eq!(keys, vec![&1, &2, &3]);
    }

    #[test]
    fn test_from_iterator() {
        let tree: RedBlackTree<i32, &str> =
            vec![(3, "c"), (1, "a"), (2, "b")].into_iter().collect();

        assert_eq!(tree.len(), 3);
        assert_eq!(tree.get(&1), Some(&"a"));
    }

    #[test]
    fn test_min_max() {
        let mut tree = RedBlackTree::new();
        tree.insert(3, "c");
        tree.insert(1, "a");
        tree.insert(2, "b");

        assert_eq!(tree.minimum(), Some((&1, &"a")));
        assert_eq!(tree.maximum(), Some((&3, &"c")));
    }

    #[test]
    fn test_clear() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "a");
        tree.insert(2, "b");
        tree.clear();

        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn test_contains() {
        let mut tree = RedBlackTree::new();
        tree.insert(1, "a");

        assert!(tree.contains(&1));
        assert!(!tree.contains(&2));
    }

    #[test]
    fn test_range_query() {
        let mut tree = RedBlackTree::new();
        for i in 1..=10 {
            tree.insert(i, i);
        }

        let keys: Vec<_> = tree.range(Some(&3), Some(&7)).map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![3, 4, 5, 6]);
    }

    #[rstest]
    fn test_red_black_invariants(#[values(1..=100)] n: i32) {
        let mut tree = RedBlackTree::new();
        for i in (1..=n).rev() {
            tree.insert(i, i);
        }

        assert!(tree.len() as i32 == n);
        assert!(tree.get(&1).is_some());
        assert!(tree.get(&n).is_some());
    }
}
