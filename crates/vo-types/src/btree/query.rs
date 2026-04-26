use super::node::BTreeNode;

impl<K: Ord + Clone, V: Clone> super::BTree<K, V> {
    pub fn search(&self, key: &K) -> Option<&V> {
        self.root
            .as_ref()
            .and_then(|node| Self::search_node(node, key))
    }

    fn search_node<'a>(node: &'a BTreeNode<K, V>, key: &K) -> Option<&'a V> {
        let idx = node.search_index(key);
        if idx < node.keys.len() && &node.keys[idx] == key {
            return Some(&node.values[idx]);
        }
        if node.is_leaf() {
            return None;
        }
        Self::search_node(&node.children[idx], key)
    }

    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.search(key).is_some()
    }

    #[must_use]
    pub fn min(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(|node| Self::find_min(node))
    }

    fn find_min(node: &BTreeNode<K, V>) -> (&K, &V) {
        if node.is_leaf() {
            (&node.keys[0], &node.values[0])
        } else {
            Self::find_min(&node.children[0])
        }
    }

    #[must_use]
    pub fn max(&self) -> Option<(&K, &V)> {
        self.root.as_ref().map(|node| Self::find_max(node))
    }

    fn find_max(node: &BTreeNode<K, V>) -> (&K, &V) {
        if node.is_leaf() {
            let last = node.keys.len() - 1;
            (&node.keys[last], &node.values[last])
        } else {
            let last = node.children.len() - 1;
            Self::find_max(&node.children[last])
        }
    }

    #[must_use]
    pub fn range<R>(&self, range: R) -> Vec<(&K, &V)>
    where
        R: std::ops::RangeBounds<K>,
    {
        let mut results = Vec::new();
        if let Some(ref root) = self.root {
            Self::collect_range(root, &range, &mut results);
        }
        results
    }

    fn collect_range<'a, R>(
        node: &'a BTreeNode<K, V>,
        range: &R,
        results: &mut Vec<(&'a K, &'a V)>,
    ) where
        R: std::ops::RangeBounds<K>,
    {
        for i in 0..node.keys.len() {
            if !node.is_leaf() {
                Self::collect_range(&node.children[i], range, results);
            }
            if range.contains(&node.keys[i]) {
                results.push((&node.keys[i], &node.values[i]));
            }
        }
        if !node.is_leaf() {
            Self::collect_range(&node.children[node.children.len() - 1], range, results);
        }
    }

    #[must_use]
    pub fn height(&self) -> usize {
        self.root.as_ref().map_or(0, |node| Self::node_height(node))
    }

    pub(crate) fn node_height(node: &BTreeNode<K, V>) -> usize {
        if node.is_leaf() {
            1
        } else {
            1 + Self::node_height(&node.children[0])
        }
    }

    #[must_use]
    pub fn verify(&self) -> bool {
        self.verify_reason().is_ok()
    }

    fn verify_reason(&self) -> Result<(), String> {
        match self.root.as_ref() {
            None => Ok(()),
            Some(root) => {
                let h = Self::node_height(root);
                Self::verify_node(root, self.min_keys(), self.max_keys(), h, true)
            }
        }
    }

    fn verify_node(
        node: &BTreeNode<K, V>,
        min_keys: usize,
        max_keys: usize,
        expected_height: usize,
        is_root: bool,
    ) -> Result<(), String> {
        if node.keys.len() > max_keys {
            return Err(format!(
                "keys.len {} > max_keys {}",
                node.keys.len(),
                max_keys
            ));
        }
        if !is_root && !node.is_leaf() && node.keys.len() < min_keys {
            return Err(format!(
                "non-root keys.len {} < min_keys {}",
                node.keys.len(),
                min_keys
            ));
        }
        if !node.children.is_empty() && node.children.len() != node.keys.len() + 1 {
            return Err(format!(
                "children {} != keys+1 {}",
                node.children.len(),
                node.keys.len() + 1
            ));
        }
        if node.is_leaf() && expected_height != 1 {
            return Err(format!("leaf height {} != 1", expected_height));
        }
        if !node.is_leaf() && expected_height <= 1 {
            return Err(format!("internal height {} <= 1", expected_height));
        }
        if !node.is_leaf() {
            for child in &node.children {
                Self::verify_node(child, min_keys, max_keys, expected_height - 1, false)?;
            }
        }
        Ok(())
    }
}
