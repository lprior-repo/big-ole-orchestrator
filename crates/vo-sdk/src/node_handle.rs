//! `NodeHandle<I, O>` — typed handle wrapping a workflow node (ADR-010).

use std::fmt;
use std::marker::PhantomData;

use vo_types::NodeName;

/// A typed handle to a node in a workflow DAG.
///
/// `I` and `O` are phantom type parameters representing the node's input and
/// output types. They enable compile-time enforcement of edge type compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NodeHandle<I, O> {
    name: NodeName,
    _phantom: PhantomData<(I, O)>,
}

impl<I, O> NodeHandle<I, O> {
    /// Create a new `NodeHandle` with the given name.
    #[must_use]
    pub fn new(name: NodeName) -> Self {
        Self {
            name,
            _phantom: PhantomData,
        }
    }

    /// Returns the node name as a string slice.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns a reference to the underlying `NodeName`.
    #[must_use]
    pub fn node_name(&self) -> &NodeName {
        &self.name
    }
}


impl<I, O> fmt::Display for NodeHandle<I, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeHandle({})", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vo_types::NodeName;

    fn nn(s: &str) -> NodeName {
        NodeName::parse(s).expect("test: valid node name")
    }

    #[test]
    fn construct_and_name_accessor() {
        let handle: NodeHandle<(), ()> = NodeHandle::new(nn("validate"));
        assert_eq!(handle.name(), "validate");
    }

    #[test]
    fn node_name_accessor_returns_reference() {
        let handle: NodeHandle<(), ()> = NodeHandle::new(nn("compile-artifact"));
        assert_eq!(handle.node_name(), &nn("compile-artifact"));
    }

    #[test]
    fn clone_preserves_name() {
        let handle: NodeHandle<(), ()> = NodeHandle::new(nn("validate"));
        let cloned = handle.clone();
        assert_eq!(cloned.name(), "validate");
    }

    #[test]
    fn serde_roundtrip_preserves_name() {
        let handle: NodeHandle<String, i32> = NodeHandle::new(nn("transform"));
        let json = serde_json::to_string(&handle).expect("test: serialize");
        let restored: NodeHandle<String, i32> =
            serde_json::from_str(&json).expect("test: deserialize");
        assert_eq!(restored.name(), "transform");
    }

    #[test]
    fn display_shows_name() {
        let handle: NodeHandle<(), ()> = NodeHandle::new(nn("validate"));
        assert_eq!(format!("{handle}"), "NodeHandle(validate)");
    }

    /// Compile-time test: NodeHandle<A, B> and NodeHandle<C, D> are different type parameterizations.
    #[derive(Debug)]
    struct Order;
    #[derive(Debug)]
    struct ValidatedOrder;
    #[derive(Debug)]
    struct Invoice;

    #[test]
    fn different_type_params_are_different_types() {
        let _validate: NodeHandle<Order, ValidatedOrder> = NodeHandle::new(nn("validate"));
        let _invoice: NodeHandle<ValidatedOrder, Invoice> = NodeHandle::new(nn("invoice"));
        // This test merely needs to compile to pass.
        assert_ne!(_validate, _invoice);
    }
}
