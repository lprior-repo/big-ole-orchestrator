use super::traits::{A, Monoid, V};

#[derive(Clone)]
pub struct EttNode<V, A: Monoid> {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub value: V,
    pub agg: A,
    #[allow(dead_code)]
    pub entry_pos: usize,
    #[allow(dead_code)]
    pub exit_pos: usize,
}
