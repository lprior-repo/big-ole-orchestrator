#![allow(dead_code)]

use std::fmt;

pub enum RopeError {
    IndexOutOfRange { index: usize, len: usize },
    SplitAtEnd,
    DepthExceeded,
    EmptySplit,
}

impl fmt::Debug for RopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RopeError::IndexOutOfRange { index, len } => {
                write!(f, "IndexOutOfRange({}/{})", index, len)
            }
            RopeError::SplitAtEnd => write!(f, "SplitAtEnd"),
            RopeError::DepthExceeded => write!(f, "DepthExceeded"),
            RopeError::EmptySplit => write!(f, "EmptySplit"),
        }
    }
}

impl fmt::Display for RopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RopeError::IndexOutOfRange { index, len } => {
                write!(f, "index {} out of range (len={})", index, len)
            }
            RopeError::SplitAtEnd => write!(f, "split at end produces empty right"),
            RopeError::DepthExceeded => write!(f, "rope depth exceeded maximum"),
            RopeError::EmptySplit => write!(f, "cannot split empty rope"),
        }
    }
}

impl std::error::Error for RopeError {}

impl PartialEq for RopeError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                RopeError::IndexOutOfRange { index: a, len: b },
                RopeError::IndexOutOfRange { index: c, len: d },
            ) => a == c && b == d,
            (RopeError::SplitAtEnd, RopeError::SplitAtEnd) => true,
            (RopeError::DepthExceeded, RopeError::DepthExceeded) => true,
            (RopeError::EmptySplit, RopeError::EmptySplit) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
enum Node {
    Leaf(String),
    Internal {
        left: Box<Node>,
        right: Box<Node>,
        len: usize,
    },
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Leaf(s) => write!(f, "Leaf({:?})", s),
            Node::Internal { left, right, len } => f
                .debug_struct("Internal")
                .field("left", left)
                .field("right", right)
                .field("len", len)
                .finish(),
        }
    }
}

impl Node {
    fn len(&self) -> usize {
        match self {
            Node::Leaf(s) => s.len(),
            Node::Internal { len, .. } => *len,
        }
    }

    fn depth(&self) -> usize {
        match self {
            Node::Leaf(_) => 1,
            Node::Internal { left, right, .. } => 1 + usize::max(left.depth(), right.depth()),
        }
    }

    fn char_at(&self, index: usize) -> Option<char> {
        match self {
            Node::Leaf(s) => s.chars().nth(index),
            Node::Internal { left, right, .. } => {
                let left_len = left.len();
                if index < left_len {
                    left.char_at(index)
                } else {
                    right.char_at(index - left_len)
                }
            }
        }
    }

    fn to_string_inner(&self) -> String {
        match self {
            Node::Leaf(s) => s.clone(),
            Node::Internal { left, right, .. } => {
                let mut result = left.to_string_inner();
                result.push_str(&right.to_string_inner());
                result
            }
        }
    }

    fn split_at(self, index: usize) -> (Node, Node) {
        match self {
            Node::Leaf(s) => {
                let left_str: String = s.chars().take(index).collect();
                let right_str: String = s.chars().skip(index).collect();
                (Node::Leaf(left_str), Node::Leaf(right_str))
            }
            Node::Internal { left, right, .. } => {
                let left_len = left.len();
                if index == left_len {
                    (*left, *right)
                } else if index < left_len {
                    let (l, r) = left.split_at(index);
                    (l, concat_nodes(r, *right))
                } else {
                    let (l, r) = right.split_at(index - left_len);
                    (concat_nodes(*left, l), r)
                }
            }
        }
    }

    fn insert_char(&mut self, ch: char, index: usize) {
        match self {
            Node::Leaf(s) => {
                let byte_index = char_index_to_byte(s, index);
                s.insert(byte_index, ch);
            }
            Node::Internal { left, right, len } => {
                let left_len = left.len();
                if index <= left_len {
                    left.insert_char(ch, index);
                } else {
                    right.insert_char(ch, index - left_len);
                }
                *len = left.len() + right.len();
            }
        }
    }

    fn remove_char(&mut self, index: usize) -> Option<char> {
        match self {
            Node::Leaf(s) => {
                if index >= s.chars().count() {
                    return None;
                }
                let ch = s.chars().nth(index)?;
                let byte_index = char_index_to_byte(s, index);
                let end_byte = byte_index + ch.len_utf8();
                s.drain(byte_index..end_byte);
                Some(ch)
            }
            Node::Internal { left, right, len } => {
                let left_len = left.len();
                let result = if index < left_len {
                    left.remove_char(index)
                } else {
                    right.remove_char(index - left_len)
                };
                *len = left.len() + right.len();
                result
            }
        }
    }
}

fn char_index_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn concat_nodes(left: Node, right: Node) -> Node {
    if left.len() == 0 {
        return right;
    }
    if right.len() == 0 {
        return left;
    }
    Node::Internal {
        len: left.len() + right.len(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

#[derive(Clone)]
pub struct Rope {
    root: Option<Node>,
}

impl fmt::Debug for Rope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.root {
            None => write!(f, "Rope(empty)"),
            Some(node) => write!(f, "Rope({:?})", node),
        }
    }
}

impl Rope {
    pub fn new() -> Self {
        Rope { root: None }
    }

    pub fn from_str(s: &str) -> Self {
        if s.is_empty() {
            return Rope { root: None };
        }
        Rope {
            root: Some(Node::Leaf(s.to_string())),
        }
    }

    pub fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.len())
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none() || self.len() == 0
    }

    pub fn char_at(&self, index: usize) -> Result<char, RopeError> {
        match &self.root {
            None => Err(RopeError::IndexOutOfRange { index, len: 0 }),
            Some(node) => node.char_at(index).ok_or(RopeError::IndexOutOfRange {
                index,
                len: node.len(),
            }),
        }
    }

    pub fn to_string(&self) -> String {
        match &self.root {
            None => String::new(),
            Some(node) => node.to_string_inner(),
        }
    }

    pub fn concat(&self, other: &Rope) -> Rope {
        match (&self.root, &other.root) {
            (None, None) => Rope { root: None },
            (None, Some(r)) => Rope {
                root: Some(r.clone()),
            },
            (Some(l), None) => Rope {
                root: Some(l.clone()),
            },
            (Some(l), Some(r)) => Rope {
                root: Some(concat_nodes(l.clone(), r.clone())),
            },
        }
    }

    pub fn split(self, index: usize) -> Result<(Rope, Rope), RopeError> {
        match self.root {
            None => {
                if index == 0 {
                    Ok((Rope::new(), Rope::new()))
                } else {
                    Err(RopeError::IndexOutOfRange { index, len: 0 })
                }
            }
            Some(node) => {
                let len = node.len();
                if index > len {
                    return Err(RopeError::IndexOutOfRange { index, len });
                }
                if index == 0 {
                    return Ok((Rope::new(), Rope { root: Some(node) }));
                }
                if index == len {
                    return Ok((Rope { root: Some(node) }, Rope::new()));
                }
                let (left, right) = node.split_at(index);
                Ok((Rope { root: Some(left) }, Rope { root: Some(right) }))
            }
        }
    }

    pub fn insert(&mut self, ch: char, index: usize) -> Result<(), RopeError> {
        let len = self.len();
        if index > len {
            return Err(RopeError::IndexOutOfRange { index, len });
        }
        match &mut self.root {
            None => {
                self.root = Some(Node::Leaf(ch.to_string()));
            }
            Some(node) => {
                node.insert_char(ch, index);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<char, RopeError> {
        let len = self.len();
        if index >= len {
            return Err(RopeError::IndexOutOfRange { index, len });
        }
        match &mut self.root {
            None => Err(RopeError::IndexOutOfRange { index, len: 0 }),
            Some(node) => node
                .remove_char(index)
                .ok_or(RopeError::IndexOutOfRange { index, len }),
        }
    }

    pub fn depth(&self) -> usize {
        self.root.as_ref().map_or(0, |n| n.depth())
    }

    pub fn insert_str(&mut self, s: &str, index: usize) -> Result<(), RopeError> {
        let len = self.len();
        if index > len {
            return Err(RopeError::IndexOutOfRange { index, len });
        }
        if s.is_empty() {
            return Ok(());
        }
        let (left, right) = self.clone().split(index)?;
        let middle = Rope::from_str(s);
        *self = left.concat(&middle).concat(&right);
        Ok(())
    }

    pub fn slice(&self, start: usize, end: usize) -> Result<Rope, RopeError> {
        let len = self.len();
        if start > end || end > len {
            return Err(RopeError::IndexOutOfRange {
                index: if start > end { start } else { end },
                len,
            });
        }
        if start == end {
            return Ok(Rope::new());
        }
        let cloned = self.clone();
        let (_, rest) = cloned.split(start)?;
        let (slice, _) = rest.split(end - start)?;
        Ok(slice)
    }

    pub fn chars(&self) -> Vec<char> {
        self.to_string().chars().collect()
    }

    pub fn balance(&self) -> Rope {
        let s = self.to_string();
        Rope::from_str(&s)
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct RopeSlice {
    start: usize,
    end: usize,
    source_len: usize,
}

impl RopeSlice {
    pub fn new(start: usize, end: usize, source_len: usize) -> Result<Self, RopeError> {
        if start > end || end > source_len {
            return Err(RopeError::IndexOutOfRange {
                index: if start > end { start } else { end },
                len: source_len,
            });
        }
        Ok(RopeSlice {
            start,
            end,
            source_len,
        })
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

pub struct RopeBuilder {
    chunks: Vec<String>,
}

impl RopeBuilder {
    pub fn new() -> Self {
        RopeBuilder { chunks: Vec::new() }
    }

    pub fn append(&mut self, s: &str) -> &mut Self {
        if !s.is_empty() {
            self.chunks.push(s.to_string());
        }
        self
    }

    pub fn build(&mut self) -> Rope {
        if self.chunks.is_empty() {
            return Rope::new();
        }
        if self.chunks.len() == 1 {
            return Rope::from_str(&self.chunks[0]);
        }
        let mut nodes: Vec<Node> = self.chunks.drain(..).map(Node::Leaf).collect();
        while nodes.len() > 1 {
            let mut next = Vec::new();
            let mut i = 0;
            while i + 1 < nodes.len() {
                let left = std::mem::replace(&mut nodes[i], Node::Leaf(String::new()));
                let right = std::mem::replace(&mut nodes[i + 1], Node::Leaf(String::new()));
                next.push(concat_nodes(left, right));
                i += 2;
            }
            if i < nodes.len() {
                next.push(std::mem::replace(&mut nodes[i], Node::Leaf(String::new())));
            }
            nodes = next;
        }
        Rope {
            root: nodes.into_iter().next(),
        }
    }
}

impl Default for RopeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Measurable {
    fn measure(&self) -> usize;
}

impl Measurable for Rope {
    fn measure(&self) -> usize {
        self.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn empty_rope_has_zero_length() {
        let rope = Rope::new();
        assert_eq!(rope.len(), 0);
        assert!(rope.is_empty());
    }

    #[test]
    fn from_str_roundtrip() {
        let s = "Hello, world!";
        let rope = Rope::from_str(s);
        assert_eq!(rope.to_string(), s);
        assert_eq!(rope.len(), s.len());
        assert!(!rope.is_empty());
    }

    #[test]
    fn from_str_empty() {
        let rope = Rope::from_str("");
        assert!(rope.is_empty());
        assert_eq!(rope.len(), 0);
        assert_eq!(rope.to_string(), "");
    }

    #[test]
    fn char_at_valid_indices() {
        let rope = Rope::from_str("abc");
        assert_eq!(rope.char_at(0).unwrap(), 'a');
        assert_eq!(rope.char_at(1).unwrap(), 'b');
        assert_eq!(rope.char_at(2).unwrap(), 'c');
    }

    #[test]
    fn char_at_out_of_range() {
        let rope = Rope::from_str("abc");
        let err = rope.char_at(3).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 3, len: 3 });
    }

    #[test]
    fn char_at_empty_rope() {
        let rope = Rope::new();
        let err = rope.char_at(0).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 0, len: 0 });
    }

    #[test]
    fn concat_two_ropes() {
        let a = Rope::from_str("hello");
        let b = Rope::from_str(" world");
        let c = a.concat(&b);
        assert_eq!(c.to_string(), "hello world");
        assert_eq!(c.len(), 11);
    }

    #[test]
    fn concat_empty_left() {
        let a = Rope::new();
        let b = Rope::from_str("abc");
        let c = a.concat(&b);
        assert_eq!(c.to_string(), "abc");
    }

    #[test]
    fn concat_empty_right() {
        let a = Rope::from_str("abc");
        let b = Rope::new();
        let c = a.concat(&b);
        assert_eq!(c.to_string(), "abc");
    }

    #[test]
    fn concat_both_empty() {
        let a = Rope::new();
        let b = Rope::new();
        let c = a.concat(&b);
        assert!(c.is_empty());
    }

    #[test]
    fn split_at_middle() {
        let rope = Rope::from_str("hello world");
        let (left, right) = rope.split(5).unwrap();
        assert_eq!(left.to_string(), "hello");
        assert_eq!(right.to_string(), " world");
    }

    #[test]
    fn split_at_start() {
        let rope = Rope::from_str("hello");
        let (left, right) = rope.split(0).unwrap();
        assert!(left.is_empty());
        assert_eq!(right.to_string(), "hello");
    }

    #[test]
    fn split_at_end() {
        let rope = Rope::from_str("hello");
        let (left, right) = rope.split(5).unwrap();
        assert_eq!(left.to_string(), "hello");
        assert!(right.is_empty());
    }

    #[test]
    fn split_empty_rope() {
        let rope = Rope::new();
        let (left, right) = rope.split(0).unwrap();
        assert!(left.is_empty());
        assert!(right.is_empty());
    }

    #[test]
    fn split_out_of_range() {
        let rope = Rope::from_str("abc");
        let err = rope.split(4).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 4, len: 3 });
    }

    #[test]
    fn insert_char_into_empty() {
        let mut rope = Rope::new();
        rope.insert('a', 0).unwrap();
        assert_eq!(rope.to_string(), "a");
    }

    #[test]
    fn insert_char_at_start() {
        let mut rope = Rope::from_str("bc");
        rope.insert('a', 0).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn insert_char_at_end() {
        let mut rope = Rope::from_str("ab");
        rope.insert('c', 2).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn insert_char_at_middle() {
        let mut rope = Rope::from_str("ac");
        rope.insert('b', 1).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn insert_char_out_of_range() {
        let mut rope = Rope::from_str("abc");
        let err = rope.insert('x', 4).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 4, len: 3 });
    }

    #[test]
    fn remove_char_from_start() {
        let mut rope = Rope::from_str("abc");
        let ch = rope.remove(0).unwrap();
        assert_eq!(ch, 'a');
        assert_eq!(rope.to_string(), "bc");
    }

    #[test]
    fn remove_char_from_end() {
        let mut rope = Rope::from_str("abc");
        let ch = rope.remove(2).unwrap();
        assert_eq!(ch, 'c');
        assert_eq!(rope.to_string(), "ab");
    }

    #[test]
    fn remove_char_from_middle() {
        let mut rope = Rope::from_str("abc");
        let ch = rope.remove(1).unwrap();
        assert_eq!(ch, 'b');
        assert_eq!(rope.to_string(), "ac");
    }

    #[test]
    fn remove_char_out_of_range() {
        let mut rope = Rope::from_str("abc");
        let err = rope.remove(3).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 3, len: 3 });
    }

    #[test]
    fn remove_from_empty_rope() {
        let mut rope = Rope::new();
        let err = rope.remove(0).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 0, len: 0 });
    }

    #[test]
    fn insert_str_at_middle() {
        let mut rope = Rope::from_str("ac");
        rope.insert_str("b", 1).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn insert_str_at_start() {
        let mut rope = Rope::from_str("world");
        rope.insert_str("hello ", 0).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn insert_str_at_end() {
        let mut rope = Rope::from_str("hello");
        rope.insert_str(" world", 5).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn insert_str_empty_is_noop() {
        let mut rope = Rope::from_str("abc");
        rope.insert_str("", 1).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn insert_str_out_of_range() {
        let mut rope = Rope::from_str("abc");
        let err = rope.insert_str("x", 4).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 4, len: 3 });
    }

    #[test]
    fn slice_middle() {
        let rope = Rope::from_str("hello world");
        let slice = rope.slice(0, 5).unwrap();
        assert_eq!(slice.to_string(), "hello");
    }

    #[test]
    fn slice_empty_range() {
        let rope = Rope::from_str("hello");
        let slice = rope.slice(2, 2).unwrap();
        assert!(slice.is_empty());
    }

    #[test]
    fn slice_full_range() {
        let rope = Rope::from_str("hello");
        let slice = rope.slice(0, 5).unwrap();
        assert_eq!(slice.to_string(), "hello");
    }

    #[test]
    fn slice_invalid_range() {
        let rope = Rope::from_str("hello");
        let err = rope.slice(3, 2).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 3, len: 5 });
    }

    #[test]
    fn slice_beyond_end() {
        let rope = Rope::from_str("hello");
        let err = rope.slice(0, 10).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 10, len: 5 });
    }

    #[test]
    fn chars_returns_all_chars() {
        let rope = Rope::from_str("abc");
        assert_eq!(rope.chars(), vec!['a', 'b', 'c']);
    }

    #[test]
    fn chars_empty_rope() {
        let rope = Rope::new();
        assert_eq!(rope.chars(), Vec::<char>::new());
    }

    #[test]
    fn depth_single_leaf() {
        let rope = Rope::from_str("hello");
        assert_eq!(rope.depth(), 1);
    }

    #[test]
    fn depth_empty() {
        let rope = Rope::new();
        assert_eq!(rope.depth(), 0);
    }

    #[test]
    fn depth_after_concat() {
        let a = Rope::from_str("hello");
        let b = Rope::from_str(" world");
        let c = a.concat(&b);
        assert!(c.depth() >= 2);
    }

    #[test]
    fn balance_reduces_depth() {
        let mut rope = Rope::from_str("a");
        for _ in 0..50 {
            let next = Rope::from_str("b");
            rope = rope.concat(&next);
        }
        let unbalanced_depth = rope.depth();
        let expected_content = rope.to_string();
        let balanced = rope.balance();
        assert!(balanced.depth() <= unbalanced_depth);
        assert_eq!(balanced.to_string(), expected_content);
    }

    #[test]
    fn repeated_inserts_maintain_content() {
        let mut rope = Rope::new();
        let expected = "abcdef";
        for (i, ch) in expected.chars().enumerate() {
            rope.insert(ch, i).unwrap();
        }
        assert_eq!(rope.to_string(), expected);
    }

    #[test]
    fn split_then_concat_identity() {
        let original = Rope::from_str("hello world");
        let (left, right) = original.clone().split(5).unwrap();
        let restored = left.concat(&right);
        assert_eq!(restored.to_string(), original.to_string());
    }

    #[test]
    fn multiple_splits() {
        let rope = Rope::from_str("abcdefghij");
        let (a, rest) = rope.split(3).unwrap();
        let (b, c) = rest.split(4).unwrap();
        assert_eq!(a.to_string(), "abc");
        assert_eq!(b.to_string(), "defg");
        assert_eq!(c.to_string(), "hij");
    }

    #[test]
    fn split_at_concat_boundary() {
        let a = Rope::from_str("hello");
        let b = Rope::from_str("world");
        let rope = a.concat(&b);
        let (left, right) = rope.split(5).unwrap();
        assert_eq!(left.to_string(), "hello");
        assert_eq!(right.to_string(), "world");
    }

    #[test]
    fn rope_builder_empty() {
        let rope = RopeBuilder::new().build();
        assert!(rope.is_empty());
    }

    #[test]
    fn rope_builder_single_chunk() {
        let mut builder = RopeBuilder::new();
        builder.append("hello");
        let rope = builder.build();
        assert_eq!(rope.to_string(), "hello");
    }

    #[test]
    fn rope_builder_multiple_chunks() {
        let mut builder = RopeBuilder::new();
        builder.append("hello").append(" ").append("world");
        let rope = builder.build();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn rope_builder_skips_empty() {
        let mut builder = RopeBuilder::new();
        builder.append("a").append("").append("b");
        let rope = builder.build();
        assert_eq!(rope.to_string(), "ab");
    }

    #[test]
    fn rope_builder_chain() {
        let mut builder = RopeBuilder::new();
        builder.append("x").append("y").append("z");
        let rope = builder.build();
        assert_eq!(rope.to_string(), "xyz");
    }

    #[test]
    fn rope_slice_basic() {
        let rs = RopeSlice::new(2, 5, 10).unwrap();
        assert_eq!(rs.len(), 3);
        assert_eq!(rs.start(), 2);
        assert_eq!(rs.end(), 5);
        assert!(!rs.is_empty());
    }

    #[test]
    fn rope_slice_empty() {
        let rs = RopeSlice::new(3, 3, 10).unwrap();
        assert_eq!(rs.len(), 0);
        assert!(rs.is_empty());
    }

    #[test]
    fn rope_slice_invalid_range() {
        let err = RopeSlice::new(5, 3, 10).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 5, len: 10 });
    }

    #[test]
    fn rope_slice_beyond_source() {
        let err = RopeSlice::new(0, 15, 10).unwrap_err();
        assert_eq!(err, RopeError::IndexOutOfRange { index: 15, len: 10 });
    }

    #[test]
    fn measurable_trait_for_rope() {
        let rope = Rope::from_str("hello");
        assert_eq!(rope.measure(), 5);
    }

    #[test]
    fn default_is_empty() {
        let rope = Rope::default();
        assert!(rope.is_empty());
    }

    #[test]
    fn builder_default_is_empty() {
        let mut builder = RopeBuilder::default();
        let rope = builder.build();
        assert!(rope.is_empty());
    }

    #[test]
    fn rope_error_display() {
        let err = RopeError::IndexOutOfRange { index: 5, len: 3 };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("3"));
    }

    #[test]
    fn large_rope_split_at_boundaries() {
        let mut builder = RopeBuilder::new();
        for i in 0..100 {
            builder.append(&format!("chunk{}:", i));
        }
        let rope = builder.build();
        let s = rope.to_string();
        assert_eq!(rope.len(), s.len());

        let mid = s.len() / 2;
        let (left, right) = rope.split(mid).unwrap();
        assert_eq!(left.to_string(), &s[..mid]);
        assert_eq!(right.to_string(), &s[mid..]);
    }

    #[test]
    fn split_concat_large_identity() {
        let mut builder = RopeBuilder::new();
        for i in 0..50 {
            builder.append(&format!("seg{}|", i));
        }
        let rope = builder.build();
        let s = rope.to_string();

        let (a, b) = rope.split(s.len() / 3).unwrap();
        let (c, d) = b.split(s.len() / 3).unwrap();
        let restored = a.concat(&c).concat(&d);
        assert_eq!(restored.to_string(), s);
    }

    #[test]
    fn remove_all_chars_one_by_one() {
        let mut rope = Rope::from_str("abc");
        rope.remove(0).unwrap();
        assert_eq!(rope.to_string(), "bc");
        rope.remove(0).unwrap();
        assert_eq!(rope.to_string(), "c");
        rope.remove(0).unwrap();
        assert_eq!(rope.to_string(), "");
    }

    #[test]
    fn insert_then_remove_roundtrip() {
        let mut rope = Rope::from_str("ac");
        rope.insert('b', 1).unwrap();
        assert_eq!(rope.to_string(), "abc");
        rope.remove(1).unwrap();
        assert_eq!(rope.to_string(), "ac");
    }

    #[test]
    fn concat_many_ropes() {
        let mut rope = Rope::new();
        let expected: String = (0..20).map(|i| char::from(b'a' + (i % 26))).collect();
        for ch in expected.chars() {
            rope = rope.concat(&Rope::from_str(&ch.to_string()));
        }
        assert_eq!(rope.to_string(), expected);
    }

    #[test]
    fn slice_then_concat_preserves_content() {
        let rope = Rope::from_str("abcdefghij");
        let a = rope.slice(0, 3).unwrap();
        let b = rope.slice(3, 7).unwrap();
        let c = rope.slice(7, 10).unwrap();
        let restored = a.concat(&b).concat(&c);
        assert_eq!(restored.to_string(), "abcdefghij");
    }

    #[test]
    fn insert_str_full_replacement() {
        let mut rope = Rope::new();
        rope.insert_str("hello world", 0).unwrap();
        assert_eq!(rope.to_string(), "hello world");
    }

    #[test]
    fn split_at_one() {
        let rope = Rope::from_str("abc");
        let (left, right) = rope.split(1).unwrap();
        assert_eq!(left.to_string(), "a");
        assert_eq!(right.to_string(), "bc");
    }

    #[test]
    fn unicode_char_at() {
        let rope = Rope::from_str("héllo");
        assert_eq!(rope.char_at(0).unwrap(), 'h');
        assert_eq!(rope.char_at(1).unwrap(), 'é');
        assert_eq!(rope.char_at(2).unwrap(), 'l');
    }

    #[test]
    fn unicode_split() {
        let rope = Rope::from_str("héllo");
        let (left, right) = rope.split(2).unwrap();
        assert_eq!(left.to_string(), "hé");
        assert_eq!(right.to_string(), "llo");
    }

    #[test]
    fn unicode_insert_remove() {
        let mut rope = Rope::from_str("héllo");
        rope.remove(1).unwrap();
        assert_eq!(rope.to_string(), "hllo");
        rope.insert('é', 1).unwrap();
        assert_eq!(rope.to_string(), "héllo");
    }

    #[test]
    fn clone_preserves_content() {
        let rope = Rope::from_str("hello");
        let cloned = rope.clone();
        assert_eq!(cloned.to_string(), "hello");
    }

    #[test]
    fn concat_associativity() {
        let a = Rope::from_str("aa");
        let b = Rope::from_str("bb");
        let c = Rope::from_str("cc");
        let left = a.concat(&b).concat(&c);
        let right = a.concat(&b.concat(&c));
        assert_eq!(left.to_string(), right.to_string());
        assert_eq!(left.to_string(), "aabbcc");
    }

    #[test]
    fn debug_format_empty() {
        let rope = Rope::new();
        let debug = format!("{:?}", rope);
        assert!(debug.contains("empty"));
    }

    #[test]
    fn debug_format_nonempty() {
        let rope = Rope::from_str("hi");
        let debug = format!("{:?}", rope);
        assert!(debug.contains("hi"));
    }

    #[test]
    fn rope_error_equality() {
        let a = RopeError::IndexOutOfRange { index: 1, len: 5 };
        let b = RopeError::IndexOutOfRange { index: 1, len: 5 };
        let c = RopeError::IndexOutOfRange { index: 2, len: 5 };
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(RopeError::SplitAtEnd, RopeError::SplitAtEnd);
        assert_eq!(RopeError::DepthExceeded, RopeError::DepthExceeded);
        assert_eq!(RopeError::EmptySplit, RopeError::EmptySplit);
    }

    #[test]
    fn rope_error_debug_format() {
        let err = RopeError::IndexOutOfRange { index: 5, len: 3 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("5/3"));
    }

    #[test]
    fn split_at_boundary_of_concat() {
        let mut builder = RopeBuilder::new();
        builder.append("aaa").append("bbb").append("ccc");
        let rope = builder.build();
        let s = rope.to_string();
        assert_eq!(s, "aaabbbccc");
        let (l, r) = rope.split(6).unwrap();
        assert_eq!(l.to_string(), "aaabbb");
        assert_eq!(r.to_string(), "ccc");
    }

    #[test]
    fn insert_str_into_empty() {
        let mut rope = Rope::new();
        rope.insert_str("abc", 0).unwrap();
        assert_eq!(rope.to_string(), "abc");
    }

    #[test]
    fn balance_deeply_nested() {
        let mut rope = Rope::new();
        for ch in "abcdefghijklmnopqrstuvwxyz".chars() {
            rope = rope.concat(&Rope::from_str(&ch.to_string()));
        }
        assert_eq!(rope.depth(), 26);
        let balanced = rope.balance();
        assert_eq!(balanced.depth(), 1);
        assert_eq!(balanced.to_string(), "abcdefghijklmnopqrstuvwxyz");
    }

    proptest! {
        #[test]
        fn proptest_from_str_roundtrip(ref s in ".*") {
            let rope = Rope::from_str(s);
            prop_assert_eq!(rope.to_string(), s.as_str());
            prop_assert_eq!(rope.len(), s.len());
        }

        #[test]
        fn proptest_char_at_matches_string(s in "[a-zA-Z]{1,20}") {
            let rope = Rope::from_str(&s);
            for (i, ch) in s.chars().enumerate() {
                prop_assert_eq!(rope.char_at(i).unwrap(), ch);
            }
        }

        #[test]
        fn proptest_concat_matches_string(a in "[a-zA-Z]{0,20}", b in "[a-zA-Z]{0,20}") {
            let rope_a = Rope::from_str(&a);
            let rope_b = Rope::from_str(&b);
            let combined = rope_a.concat(&rope_b);
            prop_assert_eq!(combined.to_string(), format!("{}{}", a, b));
        }

        #[test]
        fn proptest_split_then_concat_identity(s in "[a-zA-Z]{1,20}", split_at in 0usize..20) {
            let split_at = split_at.min(s.len());
            let rope = Rope::from_str(&s);
            let (left, right) = rope.split(split_at).unwrap();
            let restored = left.concat(&right);
            prop_assert_eq!(restored.to_string(), s);
        }

        #[test]
        fn proptest_split_matches_string(s in "[a-zA-Z]{2,20}", split_at in 1usize..19) {
            let split_at = split_at.min(s.len() - 1).max(1);
            let rope = Rope::from_str(&s);
            let (left, right) = rope.split(split_at).unwrap();
            prop_assert_eq!(left.to_string(), &s[..split_at]);
            prop_assert_eq!(right.to_string(), &s[split_at..]);
        }

        #[test]
        fn proptest_insert_char_preserves_content(
            mut s in "[a-zA-Z]{0,20}",
            ch in any::<char>(),
            index in 0usize..21,
        ) {
            let index = index.min(s.len());
            let mut rope = Rope::from_str(&s);
            rope.insert(ch, index).unwrap();
            s.insert(index, ch);
            prop_assert_eq!(rope.to_string(), s);
        }

        #[test]
        fn proptest_remove_char_preserves_content(
            s in "[a-zA-Z]{1,20}",
            index in 0usize..19,
        ) {
            let index = index.min(s.len() - 1);
            let mut rope = Rope::from_str(&s);
            let removed_rope = rope.remove(index).unwrap();
            let removed_str = s.chars().nth(index).unwrap();
            prop_assert_eq!(removed_rope, removed_str);
            let expected: String = s.chars().enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, c)| c)
                .collect();
            prop_assert_eq!(rope.to_string(), expected);
        }

        #[test]
        fn proptest_insert_str_preserves_content(
            mut s in "[a-zA-Z]{0,20}",
            insert in "[a-zA-Z]{0,10}",
            index in 0usize..21,
        ) {
            let index = index.min(s.len());
            let mut rope = Rope::from_str(&s);
            rope.insert_str(&insert, index).unwrap();
            s.insert_str(index, &insert);
            prop_assert_eq!(rope.to_string(), s);
        }

        #[test]
        fn proptest_slice_matches_string(
            s in "[a-zA-Z]{5,30}",
            start in 0usize..30,
            end in 0usize..30,
        ) {
            let start = start.min(s.len());
            let end = end.min(s.len());
            if start <= end {
                let rope = Rope::from_str(&s);
                let slice = rope.slice(start, end).unwrap();
                prop_assert_eq!(slice.to_string(), &s[start..end]);
            }
        }

        #[test]
        fn proptest_random_split_concat_sequence(
            initial in "[a-zA-Z]{1,10}",
            ops in proptest::collection::vec(
                (any::<bool>(), 0usize..10),
                0..10,
            ),
        ) {
            let mut rope = Rope::from_str(&initial);
            let mut reference = initial.clone();

            for (do_split, split_at) in ops {
                let split_at = split_at.min(reference.len());
                if reference.is_empty() {
                    continue;
                }
                if do_split && reference.len() > 1 {
                    let (left, right) = rope.clone().split(split_at).unwrap();
                    let other = Rope::from_str("XY");
                    if split_at <= left.len() {
                        rope = left.concat(&other).concat(&right);
                    } else {
                        rope = left.concat(&right);
                    }
                    reference.insert_str(split_at, "XY");
                }
            }
            prop_assert_eq!(rope.to_string(), reference);
        }

        #[test]
        fn proptest_balance_preserves_content(s in "[a-zA-Z]{0,50}") {
            let mut rope = Rope::new();
            for ch in s.chars() {
                rope = rope.concat(&Rope::from_str(&ch.to_string()));
            }
            let balanced = rope.balance();
            prop_assert_eq!(balanced.to_string(), s);
        }

        #[test]
        fn proptest_concat_associativity(
            a in "[a-z]{0,10}",
            b in "[a-z]{0,10}",
            c in "[a-z]{0,10}",
        ) {
            let ra = Rope::from_str(&a);
            let rb = Rope::from_str(&b);
            let rc = Rope::from_str(&c);
            let left = ra.clone().concat(&rb).concat(&rc);
            let right = ra.concat(&rb.concat(&rc));
            prop_assert_eq!(left.to_string(), right.to_string());
        }

        #[test]
        fn proptest_multiple_splits_reassemble(
            s in "[a-zA-Z]{3,20}",
            split_points in proptest::collection::vec(1usize..19, 0..5),
        ) {
            let mut points: Vec<usize> = split_points
                .into_iter()
                .filter(|&p| p < s.len())
                .collect();
            points.sort();
            points.dedup();

            let mut rope = Rope::from_str(&s);
            let mut pieces: Vec<Rope> = Vec::new();
            let mut offset = 0;

            for &p in &points {
                let split_at = p.saturating_sub(offset);
                if split_at == 0 || split_at >= rope.len() {
                    continue;
                }
                let (left, right) = rope.split(split_at).unwrap();
                pieces.push(left);
                offset = p;
                rope = right;
            }
            pieces.push(rope);

            let reassembled = pieces.into_iter()
                .reduce(|a, b| a.concat(&b))
                .unwrap();
            prop_assert_eq!(reassembled.to_string(), s);
        }

        #[test]
        fn proptest_builder_matches_concat(
            chunks in proptest::collection::vec("[a-zA-Z]{1,10}", 1..10),
        ) {
            let mut builder = RopeBuilder::new();
            for chunk in &chunks {
                builder.append(chunk);
            }
            let built = builder.build();
            let expected: String = chunks.join("");
            prop_assert_eq!(built.to_string(), expected);
        }
    }
}
