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

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let v = self.get(key).cloned()?;
        self.root = self.root.take().and_then(|h| Self::del(h, key));
        if let Some(ref r) = self.root {
            r.color = Color::Black;
        }
        self.len = self.len.saturating_sub(1);
        Some(v)
    }
    fn del(h: Box<Node<K, V>>, key: &K) -> Option<Box<Node<K, V>>> {
        let mut n = h;
        if key < &n.key {
            if !Node::is_red(&n.left)
                && !Node::is_red(&n.left.as_ref().and_then(|l| l.left.clone()))
            {
                n = Node::move_red_left(n);
            }
            n.left = n.left.take().and_then(|c| Self::del(c, key));
        } else {
            if Node::is_red(&n.left) {
                n = Node::rot_right(n);
            }
            if key == &n.key && n.right.is_none() {
                return None;
            }
            if !Node::is_red(&n.right)
                && !Node::is_red(&n.right.as_ref().and_then(|r| r.left.clone()))
            {
                n = Node::move_red_right(n);
            }
            if key == &n.key {
                let (sk, sv) = n.right.as_ref().map(Node::min_node)?;
                n.key = sk.clone();
                n.value = sv.clone();
                n.right = n.right.take().and_then(Self::del_min);
            } else {
                n.right = n.right.take().and_then(|c| Self::del(c, key));
            }
        }
        Some(Node::fix_up(n))
    }
    fn del_min(mut h: Box<Node<K, V>>) -> Option<Box<Node<K, V>>> {
        if h.left.is_none() {
            return None;
        }
        if !Node::is_red(&h.left) && !Node::is_red(&h.left.as_ref().and_then(|l| l.left.clone())) {
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
    pub fn iter(&self) -> Iter<K, V> {
        let mut v = Vec::new();
        if let Some(ref r) = self.root {
            Node::collect_inorder(r, &mut v);
        }
        v.reverse();
        Iter(v)
    }
    pub fn keys(&self) -> Keys<K, V> {
        Keys(self.iter())
    }
    pub fn values(&self) -> Values<K, V> {
        Values(self.iter())
    }
    pub fn range(&self, lo: Option<&K>, hi: Option<&K>) -> Range<K, V> {
        let mut v = Vec::new();
        if let Some(ref r) = self.root {
            Node::collect_range(r, &mut v, lo, hi);
        }
        v.reverse();
        Range(v)
    }
}

#[derive(Debug, Default)]
pub struct Iter<K, V>(Vec<(&K, &V)>);
impl<K, V> Iterator for Iter<K, V> {
    type Item = (&K, &V);
    fn next(&mut self) -> Option<Self::Item> {
        self.0.pop()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len(), Some(self.0.len()))
    }
}
impl<K, V> ExactSizeIterator for Iter<K, V> {}
impl<K, V> FusedIterator for Iter<K, V> {}

macro_rules! wrap {
    ($n:ident, $t:ty, $f:expr) => {
        #[derive(Debug)]
        pub struct $n<K, V>(pub(super) Iter<K, V>);
        impl<K, V> Iterator for $n<K, V> {
            type Item = $t;
            fn next(&mut self) -> Option<Self::Item> {
                self.0.next().map($f)
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }
        impl<K, V> ExactSizeIterator for $n<K, V> {}
        impl<K, V> FusedIterator for $n<K, V> {}
    };
}
wrap!(Keys, &K, |(k, _)| k);
wrap!(Values, &V, |(_, v)| v);

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
impl<'a, K, V> ExactSizeIterator for Range<'a, K, V> {}
impl<'a, K, V> FusedIterator for Range<'a, K, V> {}

impl<K: Debug, V: Debug> Debug for RedBlackTree<K, V> {
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
