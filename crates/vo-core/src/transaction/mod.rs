//! Atomic control plane transaction struct.
//!
//! Per ADR-016: All control-plane transitions must commit atomically via
//! `fjall::Batch`. This module provides a `Transaction` that locally buffers
//! `DbWriterMessage` operations and commits them as a single batch through
//! a storage backend.
//!
//! # Invariants
//!
//! - No events are passed to storage before `commit()` is explicitly called.
//! - If `commit()` fails, zero state changes are visible (all-or-nothing).
//! - A `Transaction` cannot be committed more than once.

mod committer;
mod tests;

pub use committer::TransactionCommitter;

use std::marker::PhantomData;

use thiserror::Error;

use crate::db_writer_message::DbWriterMessage;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionError {
    #[error("transaction already committed")]
    AlreadyCommitted,
    #[error("transaction is empty: nothing to commit")]
    EmptyTransaction,
    #[error("storage commit failed: {0}")]
    StorageCommitFailed(String),
    #[error("optimistic concurrency conflict: {0}")]
    OccConflict(String),
}

pub enum TransactionState {
    Open,
    Committed,
}

pub struct Transaction<C> {
    messages: Vec<DbWriterMessage>,
    state: TransactionState,
    committer: PhantomData<C>,
}

impl<C> Transaction<C> {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            state: TransactionState::Open,
            committer: PhantomData,
        }
    }

    pub fn push(&mut self, message: DbWriterMessage) -> Result<(), TransactionError> {
        if matches!(self.state, TransactionState::Committed) {
            return Err(TransactionError::AlreadyCommitted);
        }
        self.messages.push(message);
        Ok(())
    }

    pub fn pending_count(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[DbWriterMessage] {
        &self.messages
    }
}

impl<C> Default for Transaction<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: TransactionCommitter> Transaction<C> {
    pub fn commit(mut self, committer: &C) -> Result<(), TransactionError> {
        if matches!(self.state, TransactionState::Committed) {
            return Err(TransactionError::AlreadyCommitted);
        }
        if self.messages.is_empty() {
            return Err(TransactionError::EmptyTransaction);
        }
        self.state = TransactionState::Committed;
        let messages = std::mem::take(&mut self.messages);
        committer.commit_batch(messages)
    }
}
