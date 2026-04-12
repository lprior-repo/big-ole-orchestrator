//! Core engine implementation for vo-engine.
//!
//! Contains the main workflow engine, execution engine, scheduler,
//! persistence layer, and state machine implementation.

pub mod admission;
pub mod circuit_breaker;
pub mod config_hot_reload;
mod db_writer_message;
pub mod debounce;
pub mod replay;
pub mod snapshot_compat;
pub mod upcaster;
pub mod workflow_version;
pub mod write_class;

#[cfg(kani)]
pub mod write_class_verification;
