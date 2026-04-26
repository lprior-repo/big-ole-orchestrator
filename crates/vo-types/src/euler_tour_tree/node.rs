use super::traits::Monoid;

#[derive(Clone)]
pub struct EttNode<V, A: Monoid> {
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
    pub(crate) value: V,
    pub(crate) agg: A,
    #[allow(dead_code)]
    pub(crate) entry_pos: usize,
    #[allow(dead_code)]
    pub(crate) exit_pos: usize,
}
