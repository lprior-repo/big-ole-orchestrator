//! Distributed Lock Manager for concurrent workspace access.
//!
//! Provides distributed locking with:
//! - Acquire/release with TTL (time-to-live)
//! - Deadlock detection via wait-for graph
//! - Lock promotion (shared -> exclusive) and demotion
//! - Crash-safe lock recovery
//! - Automatic retry with exponential backoff for lock acquisition

#![allow(unused)]
#![allow(missing_docs)]

mod connector;
pub use connector::{
    CommitOutcome, Connector, ConnectorError, ConnectorRegistry, HttpConnector, PreparedEffect,
    ReconcileOutcome,
};
pub mod executor;
pub use executor::{
    CancellationReason, EffectContext, EffectId, ExecutionOutcome, ManagedEffectError,
    ManagedEffectExecutor, ManagedEffectTask,
};
pub mod lock;
pub mod pool;
mod port;
pub mod retry;
pub mod storage;
pub mod supervisor;

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;
use tokio::time::Duration;

pub use lock::{
    LockEntry, LockError, LockId, LockMode, LockPromote, LockPromoteResponse, LockQuery,
    LockQueryResponse, LockRelease, LockRequest, LockResponse, LockStatus, OwnerId, WaitEdge,
    WaitForGraph,
};
pub use port::LockManager;
pub use retry::{LockManagerRetryWrapper, RetryConfig};
