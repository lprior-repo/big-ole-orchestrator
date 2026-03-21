//! Core type stubs — concrete types are defined in subsequent beads.

/// Placeholder for the petgraph-backed workflow DAG (implemented in wtf-core dag bead).
pub struct WorkflowGraph;

/// Byte offset into a JetStream stream (used during replay).
pub struct JournalCursor(pub u64);
