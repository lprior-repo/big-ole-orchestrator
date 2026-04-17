use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
pub(super) struct Node<K, V> {
    pub key: K,
    pub value: V,
    pub color: Color,
    pub left: Option<Box<Node<K, V>>>,
    pub right: Option<Box<Node<K, V>>>,
}

impl<K, V> Node<K, V> {
    pub fn new(key: K, value: V) -> Self {
        Node {
            key,
            value,
            color: Color::Red,
            left: None,
            right: None,
        }
    }
    pub fn is_red(n: &Option<Box<Self>>) -> bool {
        n.as_ref().is_some_and(|n| n.color == Color::Red)
    }
    pub fn flip(node: &mut Self) {
        let f = |c: Color| {
            if c == Color::Red {
                Color::Black
            } else {
                Color::Red
            }
        };
        node.color = f(node.color);
        if let Some(ref mut l) = node.left {
            l.color = f(l.color);
        }
        if let Some(ref mut r) = node.right {
            r.color = f(r.color);
        }
    }
    pub fn rot_left(mut h: Box<Self>) -> Box<Self> {
        let mut x = h.right.take().expect("rot_left");
        h.right = x.left.take();
        x.color = h.color;
        h.color = Color::Red;
        x.left = Some(h);
        x
    }
    pub fn rot_right(mut h: Box<Self>) -> Box<Self> {
        let mut x = h.left.take().expect("rot_right");
        h.left = x.right.take();
        x.color = h.color;
        h.color = Color::Red;
        x.right = Some(h);
        x
    }
    pub fn fix_up(mut h: Box<Self>) -> Box<Self> {
        if Self::is_red(&h.right) && !Self::is_red(&h.left) {
            h = Self::rot_left(h);
        }
        if Self::is_red(&h.left) && Self::is_red(&h.left.as_ref().and_then(|l| l.left.clone())) {
            h = Self::rot_right(h);
        }
        if Self::is_red(&h.left) && Self::is_red(&h.right) {
            h.flip();
        }
        h
    }
    pub fn move_red_left(mut h: Box<Self>) -> Box<Self> {
        h.flip();
        if Self::is_red(&h.right.as_ref().and_then(|r| r.right.clone())) {
            h.right = h.right.map(Self::rot_left);
            h = Self::rot_right(h);
            h.flip();
        }
        h
    }
    pub fn move_red_right(mut h: Box<Self>) -> Box<Self> {
        h.flip();
        if Self::is_red(&h.left.as_ref().and_then(|l| l.left.clone())) {
            h = Self::rot_right(h);
            h.flip();
        }
        h
    }
    pub fn min_node(n: &Box<Self>) -> (&K, &V) {
        n.left
            .as_ref()
            .map_or((&n.key, &n.value), |l| Self::min_node(l))
    }
    pub fn max_node(n: &Box<Self>) -> (&K, &V) {
        n.right
            .as_ref()
            .map_or((&n.key, &n.value), |r| Self::max_node(r))
    }
    pub fn collect_inorder(n: &Self, v: &mut Vec<(&K, &V)>) {
        if let Some(ref l) = n.left {
            Self::collect_inorder(l, v);
        }
        v.push((&n.key, &n.value));
        if let Some(ref r) = n.right {
            Self::collect_inorder(r, v);
        }
    }
    pub fn collect_range(n: &Self, v: &mut Vec<(&K, &V)>, lo: Option<&K>, hi: Option<&K>) {
        if let Some(ref l) = n.left {
            if lo.is_none_or(|b| &n.key > b) {
                Self::collect_range(l, v, lo, hi);
            }
        }
        if lo.is_none_or(|b| &n.key >= b) && hi.is_none_or(|b| &n.key < b) {
            v.push((&n.key, &n.value));
        }
        if let Some(ref r) = n.right {
            if hi.is_none_or(|b| &n.key < b) {
                Self::collect_range(r, v, lo, hi);
            }
        }
    }
}
