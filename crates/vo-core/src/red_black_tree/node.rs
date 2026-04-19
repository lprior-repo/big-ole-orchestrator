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
    pub fn flip(&mut self) {
        let toggle = |c: &mut Color| {
            *c = if *c == Color::Red {
                Color::Black
            } else {
                Color::Red
            };
        };
        toggle(&mut self.color);
        if let Some(ref mut l) = self.left {
            toggle(&mut l.color);
        }
        if let Some(ref mut r) = self.right {
            toggle(&mut r.color);
        }
    }
    pub fn fix_up(h: Box<Self>) -> Box<Self> {
        let mut node = h;
        if Self::is_red(&node.right) && !Self::is_red(&node.left) {
            node = Self::rotate_left(node);
        }
        if Self::is_red(&node.left) && Self::is_red_left_left(&node) {
            node = Self::rotate_right(node);
        }
        if Self::is_red(&node.left) && Self::is_red(&node.right) {
            node.flip();
        }
        node
    }
    pub(super) fn rotate_left(h: Box<Self>) -> Box<Self> {
        let mut node = h;
        let mut x = node.right.take();
        node.right = x.as_mut().and_then(|x| x.left.take());
        let mut x = x.unwrap_or_else(|| {
            panic!("rotate_left: invariant violated — called when right child is None")
        });
        x.color = node.color;
        node.color = Color::Red;
        x.left = Some(node);
        x
    }
    pub(super) fn rotate_right(h: Box<Self>) -> Box<Self> {
        let mut node = h;
        let mut x = node.left.take();
        node.left = x.as_mut().and_then(|x| x.right.take());
        let mut x = x.unwrap_or_else(|| {
            panic!("rotate_right: invariant violated — called when left child is None")
        });
        x.color = node.color;
        node.color = Color::Red;
        x.right = Some(node);
        x
    }
    pub fn is_red_left_left(n: &Box<Self>) -> bool {
        n.left.as_ref().is_some_and(|l| Self::is_red(&l.left))
    }
    pub fn is_red_right_right(n: &Box<Self>) -> bool {
        n.right.as_ref().is_some_and(|r| Self::is_red(&r.right))
    }
    pub fn is_red_right_left(n: &Box<Self>) -> bool {
        n.right.as_ref().is_some_and(|r| Self::is_red(&r.left))
    }
    pub fn move_red_left(h: Box<Self>) -> Box<Self> {
        let mut node = h;
        node.flip();
        if Self::is_red_right_right(&node) {
            let right = node.right.take();
            node.right = right.map(Self::rotate_left);
            node = Self::rotate_right(node);
            node.flip();
        }
        node
    }
    pub fn move_red_right(h: Box<Self>) -> Box<Self> {
        let mut node = h;
        node.flip();
        if Self::is_red_left_left(&node) {
            node = Self::rotate_right(node);
            node.flip();
        }
        node
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
    pub fn collect_inorder<'a>(n: &'a Self, v: &mut Vec<(&'a K, &'a V)>) {
        if let Some(ref l) = n.left {
            Self::collect_inorder(l, v);
        }
        v.push((&n.key, &n.value));
        if let Some(ref r) = n.right {
            Self::collect_inorder(r, v);
        }
    }
}

impl<K: Ord, V> Node<K, V> {
    pub fn collect_range<'a>(
        n: &'a Self,
        v: &mut Vec<(&'a K, &'a V)>,
        lo: Option<&'a K>,
        hi: Option<&'a K>,
    ) {
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
