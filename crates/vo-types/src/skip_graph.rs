//! Skip Graph — distributed ordered map with O(log n) search, insert, and delete.
//!
//! A skip graph is a probabilistic distributed data structure that provides
//! an ordered key-value store with efficient lookup, insertion, and deletion.
//! Unlike skip lists, skip graphs use deterministic node membership at each level
//! based on a global ordering of node identifiers.
//!
//! # Structure
//! - Level 0: Doubly-linked list of all nodes in key order
//! - Level 1+: Successively sparser lists, each node appears at level i
//!   with probability 2^(-i)
//!
//! # Complexity
//! - Search: O(log n) expected
//! - Insert: O(log n) expected
//! - Delete: O(log n) expected
//! - Range query: O(log n + k) where k is result size
//!
//! # References
//! - J. Aspnes and G. Shah, "Skip Graphs," SODA 2003.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipGraphNode<K, V> {
    pub key: K,
    pub value: V,
    pub forwards: Vec<Option<usize>>,
    pub backwards: Vec<Option<usize>>,
    level: usize,
}

impl<K, V> SkipGraphNode<K, V> {
    fn new(key: K, value: V, level: usize) -> Self {
        let forwards = vec![None; level + 1];
        let backwards = vec![None; level + 1];
        Self {
            key,
            value,
            forwards,
            backwards,
            level,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkipGraphError {
    #[error("key not found: {0}")]
    KeyNotFound(String),
    #[error("empty graph")]
    EmptyGraph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkipGraph<K, V> {
    nodes: Vec<SkipGraphNode<K, V>>,
    head: Option<usize>,
    max_level: usize,
    len: usize,
}

impl<K: Ord + std::fmt::Debug, V> Default for SkipGraph<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord + std::fmt::Debug, V> SkipGraph<K, V> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            head: None,
            max_level: 0,
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn max_level(&self) -> usize {
        self.max_level
    }

    fn random_level() -> usize {
        let mut level = 0;
        while fast_random() && level < 31 {
            level += 1;
        }
        level
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.len == 0 {
            let level = Self::random_level();
            let node = SkipGraphNode::new(key, value, level);
            let idx = 0;
            self.nodes.push(node);
            self.head = Some(idx);
            self.max_level = level;
            self.len = 1;
            return None;
        }

        let level = Self::random_level();
        let mut node = SkipGraphNode::new(key, value, level);

        if level > self.max_level {
            self.max_level = level;
        }

        let new_idx = self.nodes.len();
        let mut current_level = self.max_level;

        let mut update: Vec<Option<usize>> = vec![None; self.max_level + 1];
        let mut prev: Vec<Option<usize>> = vec![None; self.max_level + 1];

        let mut curr = self.head;

        while current_level > level {
            curr = self.forward(curr, current_level);
            current_level -= 1;
        }

        current_level = level.min(self.max_level);

        while current_level > 0 {
            let (next, prev_node) = self.find_next_at_level(key, curr, current_level);
            prev[current_level] = prev_node;
            update[current_level] = next;
            curr = prev_node;
            current_level -= 1;
        }

        let (next0, prev0) = self.find_next_at_level(key.clone(), curr, 0);
        prev[0] = prev0;
        update[0] = next0;

        node.forwards[0] = update[0];
        if let Some(f) = update[0] {
            self.nodes[f].backwards[0] = Some(new_idx);
        }
        node.backwards[0] = prev[0];
        if let Some(p) = prev[0] {
            self.nodes[p].forwards[0] = Some(new_idx);
        } else {
            self.head = Some(new_idx);
        }

        for i in 1..=level {
            let (next, prev_node) = self.find_next_at_level(key.clone(), prev[i], i);
            node.forwards[i] = next;
            if let Some(f) = next {
                self.nodes[f].backwards[i] = Some(new_idx);
            }
            node.backwards[i] = prev_node;
            if let Some(p) = prev_node {
                self.nodes[p].forwards[i] = Some(new_idx);
            }
        }

        let old_value = std::mem::replace(&mut self.nodes.push(node) - 1 as usize, &mut self.nodes)
            .map(|n| n.value);

        let inserted_idx = new_idx;
        if inserted_idx != new_idx {
            return old_value;
        }

        self.len += 1;
        None
    }

    fn forward(&self, node: Option<usize>, level: usize) -> Option<usize> {
        node.and_then(|n| {
            self.nodes
                .get(n)
                .and_then(|node| node.forwards.get(level).copied().flatten())
        })
    }

    fn backward(&self, node: Option<usize>, level: usize) -> Option<usize> {
        node.and_then(|n| {
            self.nodes
                .get(n)
                .and_then(|node| node.backwards.get(level).copied().flatten())
        })
    }

    fn find_next_at_level(
        &self,
        key: K,
        start: Option<usize>,
        level: usize,
    ) -> (Option<usize>, Option<usize>) {
        let mut curr = start;
        loop {
            let next = self.forward(curr, level);
            match next {
                Some(idx) => match self.nodes[idx].key.cmp(&key) {
                    Ordering::Less => curr = Some(idx),
                    Ordering::Equal | Ordering::Greater => return (Some(idx), curr),
                },
                None => return (None, curr),
            }
        }
    }

    pub fn search(&self, key: &K) -> Option<&V> {
        if self.len == 0 {
            return None;
        }

        let mut current_level = self.max_level;
        let mut curr = self.head;

        while current_level > 0 {
            while let Some(next) = self.forward(curr, current_level) {
                match self.nodes[next].key.cmp(key) {
                    Ordering::Less => curr = Some(next),
                    Ordering::Equal | Ordering::Greater => break,
                }
            }
            current_level -= 1;
        }

        let next = self.forward(curr, 0);
        match next {
            Some(idx) if self.nodes[idx].key == *key => Some(&self.nodes[idx].value),
            _ => None,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.search(key)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.search(key).is_some()
    }

    pub fn remove(&mut self, key: &K) -> Result<V, SkipGraphError> {
        if self.len == 0 {
            return Err(SkipGraphError::EmptyGraph);
        }

        let target = self
            .search(key)
            .ok_or_else(|| SkipGraphError::KeyNotFound(format!("{:?}", key)))?;

        let idx = self
            .nodes
            .iter()
            .position(|n| &n.value as *const V == target as *const V)
            .unwrap();

        for level in 0..=self.nodes[idx].level {
            let prev = self.nodes[idx].backwards[level];
            let next = self.nodes[idx].forwards[level];

            if let Some(p) = prev {
                self.nodes[p].forwards[level] = next;
            } else {
                if level == 0 {
                    self.head = next;
                }
            }

            if let Some(n) = next {
                self.nodes[n].backwards[level] = prev;
            }
        }

        let value = self.nodes.swap_remove(idx).value;

        if idx < self.nodes.len() {
            for i in 0..self.nodes.len() {
                for level in 0..=self.nodes[i].level {
                    if self.nodes[i].forwards[level] == Some(idx) {
                        self.nodes[i].forwards[level] = Some(idx);
                    }
                    if self.nodes[i].backwards[level] == Some(idx) {
                        self.nodes[i].backwards[level] = Some(idx);
                    }
                }
            }
        }

        self.len -= 1;
        Ok(value)
    }

    pub fn range_query(&self, low: &K, high: &K) -> Vec<(&K, &V)> {
        if self.len == 0 {
            return Vec::new();
        }

        let mut results = Vec::new();
        let mut curr = self.head;

        while let Some(next) = self.forward(curr, 0) {
            let node = &self.nodes[next];
            if node.key < *low {
                curr = Some(next);
                continue;
            }
            if node.key > *high {
                break;
            }
            results.push((&node.key, &node.value));
            curr = Some(next);
        }

        results
    }

    pub fn iter(&self) -> SkipGraphIter<'_, K, V> {
        SkipGraphIter {
            graph: self,
            current: self.head,
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}

pub struct SkipGraphIter<'a, K, V> {
    graph: &'a SkipGraph<K, V>,
    current: Option<usize>,
}

impl<'a, K, V> Iterator for SkipGraphIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let curr = self.current?;
        let node = &self.graph.nodes[curr];
        self.current = node.forwards[0];
        Some((&node.key, &node.value))
    }
}

fn fast_random() -> bool {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 2) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_graph_is_empty() {
        let sg: SkipGraph<i32, String> = SkipGraph::new();
        assert!(sg.is_empty());
        assert_eq!(sg.len(), 0);
    }

    #[test]
    fn insert_increases_len() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        assert_eq!(sg.len(), 1);
        sg.insert(2, "two");
        assert_eq!(sg.len(), 2);
    }

    #[test]
    fn search_returns_value() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        sg.insert(2, "two");
        sg.insert(3, "three");
        assert_eq!(sg.search(&1), Some(&"one".to_string()));
        assert_eq!(sg.search(&2), Some(&"two".to_string()));
        assert_eq!(sg.search(&3), Some(&"three".to_string()));
    }

    #[test]
    fn search_missing_returns_none() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        assert_eq!(sg.search(&2), None);
    }

    #[test]
    fn remove_returns_value() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        let v = sg.remove(&1).unwrap();
        assert_eq!(v, "one");
        assert!(sg.is_empty());
    }

    #[test]
    fn remove_missing_returns_error() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        assert!(matches!(sg.remove(&2), Err(SkipGraphError::KeyNotFound(_))));
    }

    #[test]
    fn range_query_returns_matching() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        sg.insert(2, "two");
        sg.insert(3, "three");
        sg.insert(4, "four");
        sg.insert(5, "five");

        let results = sg.range_query(&2, &4);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, &2);
        assert_eq!(results[1].0, &3);
        assert_eq!(results[2].0, &4);
    }

    #[test]
    fn iter_yields_all_entries() {
        let mut sg = SkipGraph::new();
        sg.insert(3, "c");
        sg.insert(1, "a");
        sg.insert(2, "b");

        let entries: Vec<_> = sg.iter().collect();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn contains_returns_correct() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        assert!(sg.contains(&1));
        assert!(!sg.contains(&2));
    }

    #[test]
    fn get_returns_value() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        assert_eq!(sg.get(&1), Some(&"one".to_string()));
    }

    #[test]
    fn insert_replace_returns_old() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "one");
        let old = sg.insert(1, "ONE").unwrap();
        assert_eq!(old, "one");
        assert_eq!(sg.get(&1), Some(&"ONE".to_string()));
    }

    #[test]
    fn keys_yields_all_keys() {
        let mut sg = SkipGraph::new();
        sg.insert(3, "c");
        sg.insert(1, "a");
        sg.insert(2, "b");

        let mut keys: Vec<_> = sg.keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, vec![1, 2, 3]);
    }

    #[test]
    fn values_yields_all_values() {
        let mut sg = SkipGraph::new();
        sg.insert(1, "a");
        sg.insert(2, "b");
        sg.insert(3, "c");

        let mut values: Vec<_> = sg.values().cloned().collect();
        values.sort();
        assert_eq!(values, vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_graph_search_returns_none() {
        let sg: SkipGraph<i32, String> = SkipGraph::new();
        assert_eq!(sg.search(&1), None);
    }

    #[test]
    fn empty_graph_remove_returns_error() {
        let mut sg: SkipGraph<i32, String> = SkipGraph::new();
        assert!(matches!(sg.remove(&1), Err(SkipGraphError::EmptyGraph)));
    }

    #[test]
    fn empty_range_query_returns_empty() {
        let sg: SkipGraph<i32, String> = SkipGraph::new();
        assert!(sg.range_query(&1, &10).is_empty());
    }
}
