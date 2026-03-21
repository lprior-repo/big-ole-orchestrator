//! wtf-frontend — Dioxus compiler control plane (ADR-018).
//! Design / Simulate / Monitor modes.
//! Adapted from Oya frontend — implemented in wtf-7n80 and subsequent beads.
//!
//! Current status: Core types exported. UI/linter/graph adaptation in subsequent beads.
//! TODO in subsequent beads:
//! - Add missing dependencies (petgraph, serde_yaml, wasm-*, itertools)
//! - Replace oya_frontend references with crate::graph references
//! - Remove/adapt Restate-specific types (restate_types.rs)
//! - Wire up UI modules with proper hooks

pub mod wtf_client;

mod graph_core_types {
    use serde::{Deserialize, Serialize};
    use std::fmt;
    use uuid::Uuid;

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub struct NodeId(pub Uuid);

    impl NodeId {
        #[must_use]
        pub fn new() -> Self {
            Self(Uuid::new_v4())
        }
    }

    impl Default for NodeId {
        fn default() -> Self {
            Self::new()
        }
    }

    impl fmt::Display for NodeId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
    pub struct PortName(pub String);

    impl<S: Into<String>> From<S> for PortName {
        fn from(s: S) -> Self {
            Self(s.into())
        }
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum NodeCategory {
        Entry,
        Durable,
        State,
        Flow,
        Timing,
        Signal,
    }

    impl fmt::Display for NodeCategory {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let s = match self {
                Self::Entry => "entry",
                Self::Durable => "durable",
                Self::State => "state",
                Self::Flow => "flow",
                Self::Timing => "timing",
                Self::Signal => "signal",
            };
            write!(f, "{s}")
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Viewport {
        pub x: f32,
        pub y: f32,
        pub zoom: f32,
    }
}

pub use graph_core_types::{NodeCategory, NodeId, PortName, Viewport};
