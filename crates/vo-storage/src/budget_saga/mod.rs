//! Durable dual-write saga for `WriteBudget`.
//!
//! This module implements ADR-034 saga compensation and reversibility for the
//! `BudgetQueues` write path. It provides:
//!
//! - **Atomic staging**: Writes are first placed in a durable staging area
//! - **Manifest update**: Commit by updating the manifest atomically
//! - **Compensating rollback**: If commit fails, rollback the staging
//! - **Crash recovery**: Recover consistent state after process crash mid-saga
//!
//! ## Saga States
//!
//! Each budget reservation goes through:
//! 1. `Staged` - Written to staging, awaiting commit
//! 2. `Committed` - Manifest updated, write is permanent
//! 3. `RolledBack` - Compensating action completed
//!
//! ## Crash Safety
//!
//! On process crash mid-saga:
//! - Staged writes that were not committed are recovered as rolled back
//! - The manifest is the source of truth for committed state

pub mod allocation;
pub mod consumption;

pub use allocation::SagaError;
pub use allocation::{BudgetManifest, SagaEntry, SagaStatus};
pub use consumption::{DurableBudgetSaga, RecoveryOutcome, SagaStore, StagedWrite};
