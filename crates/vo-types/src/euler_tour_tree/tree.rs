use super::node::EttNode;
use super::traits::{EttAggregate, EttError, Monoid};

pub struct EulerTourTree<V, A: Monoid> {
    pub(crate) nodes: Vec<EttNode<V, A>>,
    next_pos: usize,
}

impl<V: EttAggregate<A> + Clone, A: Monoid> Default for EulerTourTree<V, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: EttAggregate<A> + Clone, A: Monoid> EulerTourTree<V, A> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            next_pos: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn make_tree(&mut self, value: V) -> usize {
        let idx = self.nodes.len();
        let entry = self.next_pos;
        self.next_pos += 2;
        let exit = entry + 1;
        let agg = value.ett_aggregate();
        self.nodes.push(EttNode {
            parent: None,
            children: Vec::new(),
            value,
            agg,
            entry_pos: entry,
            exit_pos: exit,
        });
        idx
    }

    pub(crate) fn find_root_internal(&self, mut node: usize) -> usize {
        while let Some(parent) = self.nodes[node].parent {
            node = parent;
        }
        node
    }

    pub fn find_root(&mut self, node: usize) -> Result<usize, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(self.find_root_internal(node))
    }

    pub fn link(&mut self, child: usize, parent: usize) -> Result<(), EttError> {
        if child >= self.nodes.len() {
            return Err(EttError::InvalidNode(child));
        }
        if parent >= self.nodes.len() {
            return Err(EttError::InvalidNode(parent));
        }
        if self.find_root_internal(child) == self.find_root_internal(parent) {
            return Err(EttError::AlreadyConnected {
                a: child,
                b: parent,
            });
        }
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
        self.recalc_aggregate(parent);
        Ok(())
    }

    pub fn cut(&mut self, node: usize) -> Result<(), EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        if self.nodes[node].parent.is_none() {
            return Err(EttError::NotConnected { a: node, b: node });
        }
        let parent = self.nodes[node]
            .parent
            .take()
            .expect("parent is Some after is_none check");
        self.nodes[parent].children.retain(|&c| c != node);
        self.recalc_aggregate(parent);
        Ok(())
    }

    pub fn connected(&mut self, a: usize, b: usize) -> Result<bool, EttError> {
        if a >= self.nodes.len() {
            return Err(EttError::InvalidNode(a));
        }
        if b >= self.nodes.len() {
            return Err(EttError::InvalidNode(b));
        }
        Ok(self.find_root_internal(a) == self.find_root_internal(b))
    }

    fn recalc_aggregate(&mut self, node: usize) {
        let mut agg = self.nodes[node].value.ett_aggregate();
        for &child in &self.nodes[node].children {
            agg = agg.combine(&self.nodes[child].agg.clone());
        }
        self.nodes[node].agg = agg;
    }

    fn subtree_sum_internal(&self, node: usize) -> A {
        let mut agg = self.nodes[node].value.ett_aggregate();
        for &child in &self.nodes[node].children {
            agg = agg.combine(&self.subtree_sum_internal(child));
        }
        agg
    }

    pub fn subtree_aggregate(&mut self, node: usize) -> Result<A, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(self.subtree_sum_internal(node))
    }

    pub fn get(&self, node: usize) -> Result<&V, EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        Ok(&self.nodes[node].value)
    }

    pub fn set(&mut self, node: usize, value: V) -> Result<(), EttError> {
        if node >= self.nodes.len() {
            return Err(EttError::InvalidNode(node));
        }
        self.nodes[node].value = value.clone();
        self.nodes[node].agg = value.ett_aggregate();
        self.recalc_aggregate(node);
        Ok(())
    }
}
