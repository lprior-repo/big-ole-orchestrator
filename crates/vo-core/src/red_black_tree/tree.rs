use std::cmp::Ordering;
use std::fmt::Debug;
use std::iter::FusedIterator;

use super::node::{Color, Node};

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

    pub fn insert(&mut self, key: K, value: V) {
        let dup = self.contains(&key);
        self.root = Some(match self.root.take() {
            Some(n) => {
                let mut h = Self::ins(n, key, value);
                h.color = Color::Black;
                h
            }
            None => {
                let mut n = Box::new(Node::new(key, value));
                n.color = Color::Black;
                n
            }
        });
        if !dup {
            self.len += 1;
        }
    }
    fn ins(mut h: Box<Node<K, V>>, key: K, val: V) -> Box<Node<K, V>> {
        match h.key.cmp(&key) {
            Ordering::Equal => h.value = val,
            Ordering::Less => {
                h.right = Some(match h.right.take() {
                    Some(c) => Self::ins(c, key, val),
                    None => Box::new(Node::new(key, val)),
                })
            }
            Ordering::Greater => {
                h.left = Some(match h.left.take() {
                    Some(c) => Self::ins(c, key, val),
                    None => Box::new(Node::new(key, val)),
                })
            }
        }
        Node::fix_up(h)
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        let mut c = self.root.as_ref()?;
        loop {
            match c.key.cmp(key) {
                Ordering::Equal => return Some(&c.value),
                Ordering::Less => c = c.right.as_ref()?,
                Ordering::Greater => c = c.left.as_ref()?,
            }
        }
    }
    pub fn contains(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn remove(&mut self, key: &K) -> bool
    where
        K: Clone,
        V: Clone,
    {
        if !self.contains(key) {
            return false;
        }
        self.root = self.root.take().and_then(|h| Self::del(h, key));
        if let Some(ref mut r) = self.root {
            r.color = Color::Black;
        }
        self.len = self.len.saturating_sub(1);
        true
    }
    fn del(mut h: Box<Node<K, V>>, key: &K) -> Option<Box<Node<K, V>>>
    where
        K: Clone,
        V: Clone,
    {
        if key < &h.key {
            if !Node::is_red(&h.left) && !Node::is_red_left_left(&h) {
                h = Node::move_red_left(h);
            }
            h.left = h.left.take().and_then(|c| Self::del(c, key));
        } else {
            if Node::is_red(&h.left) {
                h = Node::rotate_right(h);
            }
            if key == &h.key && h.right.is_none() {
                return None;
            }
            if !Node::is_red(&h.right) && !Node::is_red_right_left(&h) {
                h = Node::move_red_right(h);
            }
            if key == &h.key {
                let r = h.right.take()?;
                let (sk, sv) = Node::min_node(&r);
                h.key = sk.clone();
                h.value = sv.clone();
                h.right = Some(Self::del_min(r)?);
            } else {
                h.right = h.right.take().and_then(|c| Self::del(c, key));
            }
        }
        Some(Node::fix_up(h))
    }
    fn del_min(mut h: Box<Node<K, V>>) -> Option<Box<Node<K, V>>> {
        if h.left.is_none() {
            return None;
        }
        if !Node::is_red(&h.left) && !Node::is_red_left_left(&h) {
            h = Node::move_red_left(h);
        }
        h.left = h.left.take().and_then(Self::del_min);
        Some(Node::fix_up(h))
    }
    pub fn minimum(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(Node::min_node)
    }
    pub fn maximum(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(Node::max_node)
    }
    pub fn clear(&mut self) {
        self.root = None;
        self.len = 0;
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        let mut v = Vec::new();
        if let Some(ref r) = self.root {
            Node::collect_inorder(r, &mut v);
        }
        v.reverse();
        Iter(v)
    }
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.iter())
    }
    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.iter())
    }
    pub fn range<'a>(&'a self, lo: Option<&'a K>, hi: Option<&'a K>) -> Range<'a, K, V> {
        let mut v = Vec::new();
        if let Some(ref r) = self.root {
            Node::collect_range(r, &mut v, lo, hi);
        }
        v.reverse();
        Range(v)
    }
}

#[derive(Debug, Default)]
pub struct Iter<'a, K, V>(Vec<(&'a K, &'a V)>);
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> FusedIterator for Iter<'_, K, V> {}

macro_rules! wrap {
    ($n:ident, $t:ty, $f:expr) => {
        #[derive(Debug)]
        pub struct $n<'a, K, V>(pub(super) Iter<'a, K, V>);
        impl<'a, K, V> Iterator for $n<'a, K, V> {
            type Item = $t;
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map($f)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }
        impl<K, V> ExactSizeIterator for $n<'_, K, V> {}
        impl<K, V> FusedIterator for $n<'_, K, V> {}
    };
}
wrap!(Keys, &'a K, |(k, _)| k);
wrap!(Values, &'a V, |(_, v)| v);

#[derive(Debug)]
pub struct Range<'a, K, V>(Vec<(&'a K, &'a V)>);
impl<'a, K, V> Iterator for Range<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }
}
impl<K, V> ExactSizeIterator for Range<'_, K, V> {}
impl<K, V> FusedIterator for Range<'_, K, V> {}

impl<K: Ord + Debug, V: Debug> Debug for RedBlackTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<K: Ord, V> Extend<(K, V)> for RedBlackTree<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, i: T) {
        for (k, v) in i {
            self.insert(k, v);
        }
    }
}
impl<K: Ord, V> FromIterator<(K, V)> for RedBlackTree<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(i: T) -> Self {
        let mut t = Self::new();
        t.extend(i);
        t
    }
}
