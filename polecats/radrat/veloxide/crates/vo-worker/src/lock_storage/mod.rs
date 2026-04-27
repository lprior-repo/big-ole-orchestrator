//! Distributed Lock Storage Module
//!
//! This module provides storage backend abstraction for distributed lock state
//! with support for multiple backend implementations.

mod memory;
mod port;

pub use memory::InMemoryLockStorage;
pub use port::{LockStorage, LockStorageError};