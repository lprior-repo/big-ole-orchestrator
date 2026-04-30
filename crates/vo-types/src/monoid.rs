//! Shared Monoid trait and common implementations.
//!
//! Extracted from `link_cut_tree.rs` and `euler_tour_tree/traits.rs` to eliminate
//! duplication. Used by both Link-Cut Tree and Euler Tour Tree test modules.

pub trait Monoid: Clone {
    fn identity() -> Self;
    fn combine(&self, other: &Self) -> Self;
}

impl Monoid for () {
    fn identity() -> Self {}
    fn combine(&self, _other: &Self) -> Self {}
}

impl Monoid for u64 {
    fn identity() -> Self {
        0
    }
    fn combine(&self, other: &Self) -> Self {
        self + other
    }
}
