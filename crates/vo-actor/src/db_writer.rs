//! DbWriterActor: single-writer ractor actor for atomic fjall commits.
//!
//! Per ADR-002: Actors NEVER write to fjall directly. All control-plane
//! transitions are sent to DbWriterActor for batch commit via `fjall::WriteBatch`.
//!
//! Per ADR-015: The mailbox is bounded at 10,000 messages. At 80% capacity
//! (8,000 pending), HTTP ingress must shed load with HTTP 429 responses.
//! Senders block (yield) when the mailbox is full.
//!
//! # Architecture
//!
//! ```text
//! InstanceActor ──┐
//! TimerActor   ───┤
//! Reanimator   ───┼──► DbWriterActor ──► fjall::WriteBatch ──► disk
//! ControlActor ───┤
//! ...             ─┘
//! ```
//!
//! The actor uses an internal `tokio::sync::mpsc` bounded channel to enforce
//! the mailbox capacity, since ractor 0.15 does not natively support bounded
//! mailboxes. The ractor `handle` method drains from this channel and commits
//! batches atomically.

use std::path::PathBuf;
use std::sync::Arc;

use ractor::{Actor, ActorProcessingErr, ActorRef};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing;

use vo_core::db_writer_message::DbWriterMessage;
use vo_storage::partitions::StorageEngine;

// ---------------------------------------------------------------------------
// Constants (ADR-015)
// ---------------------------------------------------------------------------

/// Maximum number of pending messages in the DbWriterActor mailbox.
/// Per ADR-015: strictly bounded at 10,000 messages.
pub const DB_WRITER_MAILBOX_CAPACITY: usize = 10_000;

/// Fraction of mailbox capacity at which load shedding begins.
/// Per ADR-015: HTTP ingress sheds load at 80% (8,000 of 10,000).
pub const SHED_FRACTION: f64 = 0.80;

/// Returns the shed threshold in number of messages.
#[must_use]
pub const fn shed_threshold() -> usize {
    (DB_WRITER_MAILBOX_CAPACITY as f64 * SHED_FRACTION) as usize
}

// ---------------------------------------------------------------------------
// Message type
// ---------------------------------------------------------------------------

/// Messages accepted by the DbWriterActor.
///
/// Each variant wraps a `DbWriterMessage` (the actual storage operation)
/// and optionally carries an `RpcReplyPort` for the caller to await
/// acknowledgement that the batch has been committed to disk.
#[derive(Debug)]
pub enum DbWriterMsg {
    /// Enqueue a single write operation for batch commit.
    Write {
        message: DbWriterMessage,
        reply: Option<ractor::port::RpcReplyPort<DbWriterAck>>,
    },
    /// Enqueue a batch of write operations for atomic commit.
    WriteBatch {
        messages: Vec<DbWriterMessage>,
        reply: Option<ractor::port::RpcReplyPort<DbWriterAck>>,
    },
}

/// Acknowledgement sent back to callers after a successful commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbWriterAck {
    /// Number of messages committed in this batch.
    pub committed_count: usize,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DbWriterError {
    #[error("mailbox is full: {pending}/{capacity}")]
    MailboxFull { pending: usize, capacity: usize },
    #[error("storage engine error: {reason}")]
    StorageEngine { reason: String },
    #[error("batch commit failed: {reason}")]
    BatchCommitFailed { reason: String },
    #[error("actor not started")]
    NotStarted,
}

// ---------------------------------------------------------------------------
// Actor arguments (startup config)
// ---------------------------------------------------------------------------

/// Configuration passed to DbWriterActor at spawn time.
pub struct DbWriterConfig {
    /// Path to the fjall database directory.
    pub storage_path: PathBuf,
    /// Optional mailbox capacity override (defaults to DB_WRITER_MAILBOX_CAPACITY).
    pub mailbox_capacity: Option<usize>,
}

// ---------------------------------------------------------------------------
// Actor state
// ---------------------------------------------------------------------------

/// Internal state of the DbWriterActor.
pub struct DbWriterState {
    /// The storage engine providing access to fjall partitions.
    storage: Arc<StorageEngine>,
    /// Bounded channel sender — cloned for callers to send messages.
    tx: mpsc::Sender<DbWriterMsg>,
    /// Bounded channel receiver — drained by the actor's handle loop.
    rx: mpsc::Receiver<DbWriterMsg>,
    /// Configured mailbox capacity.
    mailbox_capacity: usize,
    /// Running count of total messages committed since actor start.
    total_committed: u64,
}

impl DbWriterState {
    /// Returns the number of pending messages in the mailbox channel.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.tx.max_capacity() - self.tx.capacity()
    }

    /// Returns whether the actor should signal load shedding.
    #[must_use]
    pub fn should_shed(&self) -> bool {
        let threshold = (self.mailbox_capacity as f64 * SHED_FRACTION) as usize;
        self.pending_count() >= threshold
    }

    /// Returns the configured mailbox capacity.
    #[must_use]
    pub fn mailbox_capacity(&self) -> usize {
        self.mailbox_capacity
    }
}

// ---------------------------------------------------------------------------
// Actor struct
// ---------------------------------------------------------------------------

/// The DbWriterActor: single-writer ractor actor for atomic fjall commits.
///
/// # Spawn example
///
/// ```rust,ignore
/// use vo_actor::db_writer::{DbWriterActor, DbWriterConfig};
/// use ractor::Actor;
///
/// let config = DbWriterConfig {
///     storage_path: "/tmp/veloxide-storage".into(),
///     mailbox_capacity: None,
/// };
/// let (actor_ref, join_handle) = DbWriterActor::spawn(
///     Some("db-writer".to_string()),
///     DbWriterActor,
///     config,
/// ).await.unwrap();
/// ```
pub struct DbWriterActor;

// ---------------------------------------------------------------------------
// Actor trait implementation
// ---------------------------------------------------------------------------

impl Actor for DbWriterActor {
    type Msg = DbWriterMsg;
    type State = DbWriterState;
    type Arguments = DbWriterConfig;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let capacity = args.mailbox_capacity.unwrap_or(DB_WRITER_MAILBOX_CAPACITY);

        // Open the storage engine at the configured path.
        let storage = StorageEngine::open(&args.storage_path).map_err(|e| {
            ActorProcessingErr::from(format!("failed to open storage engine: {e}"))
        })?;
        let storage = Arc::new(storage);

        // Create the internal bounded mailbox channel.
        let (tx, rx) = mpsc::channel(capacity);

        tracing::info!(
            capacity,
            path = %args.storage_path.display(),
            "DbWriterActor pre_start: storage opened, bounded mailbox created"
        );

        Ok(DbWriterState {
            storage,
            tx,
            rx,
            mailbox_capacity: capacity,
            total_committed: 0,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            DbWriterMsg::Write { message, reply } => {
                self.commit_single(&message, state)?;
                state.total_committed += 1;

                // Emit metrics.
                metrics::counter!("vo.db_writer.committed").increment(1);
                metrics::gauge!("vo.db_writer.mailbox_depth")
                    .set(state.pending_count() as f64);

                send_ack(reply, 1);
            }
            DbWriterMsg::WriteBatch { messages, reply } => {
                let count = messages.len();
                if count > 0 {
                    self.commit_batch(&messages, state)?;
                    state.total_committed += count as u64;

                    metrics::counter!("vo.db_writer.committed").increment(count as u64);
                    metrics::gauge!("vo.db_writer.mailbox_depth")
                        .set(state.pending_count() as f64);
                }
                send_ack(reply, count);
            }
        }

        // Drain any additional pending messages from the bounded channel
        // and batch-commit them together for group commit efficiency.
        let mut drain_batch: Vec<DbWriterMessage> = Vec::new();
        let mut drain_replies: Vec<Option<ractor::port::RpcReplyPort<DbWriterAck>>> = Vec::new();
        let mut drain_counts: Vec<usize> = Vec::new();

        while let Ok(msg) = state.rx.try_recv() {
            match msg {
                DbWriterMsg::Write { message, reply } => {
                    drain_batch.push(message);
                    drain_replies.push(reply);
                    drain_counts.push(1);
                }
                DbWriterMsg::WriteBatch {
                    mut messages,
                    reply,
                } => {
                    let n = messages.len();
                    drain_batch.append(&mut messages);
                    drain_replies.push(reply);
                    drain_counts.push(n);
                }
            }
        }

        if !drain_batch.is_empty() {
            self.commit_batch(&drain_batch, state)?;
            state.total_committed += drain_batch.len() as u64;

            metrics::counter!("vo.db_writer.committed").increment(drain_batch.len() as u64);
            metrics::gauge!("vo.db_writer.mailbox_depth")
                .set(state.pending_count() as f64);

            // Send individual acks for each drained message.
            for (reply, count) in drain_replies.into_iter().zip(drain_counts.iter()) {
                send_ack(reply, *count);
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        tracing::info!(
            total_committed = state.total_committed,
            "DbWriterActor post_stop: shutting down"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Commit helpers
// ---------------------------------------------------------------------------

impl DbWriterActor {
    /// Commit a single message to the storage engine.
    fn commit_single(
        &self,
        message: &DbWriterMessage,
        state: &DbWriterState,
    ) -> Result<(), ActorProcessingErr> {
        let db = state.storage.db();
        let mut batch = db.batch();
        apply_message_to_batch(message, &mut batch);
        batch.commit().map_err(|e| {
            ActorProcessingErr::from(format!("batch commit failed: {e}"))
        })?;
        Ok(())
    }

    /// Commit a batch of messages atomically.
    fn commit_batch(
        &self,
        messages: &[DbWriterMessage],
        state: &DbWriterState,
    ) -> Result<(), ActorProcessingErr> {
        let db = state.storage.db();
        let mut batch = db.batch();
        for msg in messages {
            apply_message_to_batch(msg, &mut batch);
        }
        batch.commit().map_err(|e| {
            ActorProcessingErr::from(format!("batch commit failed: {e}"))
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Batch application logic
// ---------------------------------------------------------------------------

/// Applies a single `DbWriterMessage` to a `fjall::WriteBatch`.
///
/// This function translates the high-level `DbWriterMessage` variants
/// into the appropriate fjall partition key-value operations.
/// Currently a placeholder that will be filled in as partitions are wired.
fn apply_message_to_batch(
    _message: &DbWriterMessage,
    _batch: &mut fjall::OwnedWriteBatch,
) {
    // TODO: Implement per-variant partition routing.
    // Each DbWriterMessage variant maps to one or more partition writes:
    //
    // AppendEvent        -> events partition insert
    // RecordInstanceStatus -> instances partition insert
    // AcquireLease       -> leases partition insert
    // ReleaseLease       -> leases partition remove
    // UpsertTimer        -> timers partition insert
    // DeleteTimer        -> timers partition remove
    // RecordEffect       -> effects partition insert
    // TakeSnapshot       -> snapshots partition insert
    // AtomicTransition   -> multi-partition atomic write
    //
    // The partition keyspaces need to be opened and stored in DbWriterState
    // for this to work. This will be wired in a follow-up commit.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn send_ack(reply: Option<ractor::port::RpcReplyPort<DbWriterAck>>, count: usize) {
    if let Some(reply_port) = reply {
        let _ = reply_port.send(DbWriterAck {
            committed_count: count,
        });
    }
}

// ---------------------------------------------------------------------------
// DbWriterHandle: ergonomic sender wrapper
// ---------------------------------------------------------------------------

/// A handle for sending messages to the DbWriterActor.
///
/// This wraps the bounded `mpsc::Sender` and provides ergonomic methods
/// for enqueueing write operations. It also exposes mailbox depth
/// information for load-shedding decisions.
#[derive(Clone)]
pub struct DbWriterHandle {
    tx: mpsc::Sender<DbWriterMsg>,
    capacity: usize,
}

impl DbWriterHandle {
    /// Create a new handle from the actor state.
    /// This is called after the actor starts, extracting the sender.
    #[must_use]
    pub fn from_state(state: &DbWriterState) -> Self {
        Self {
            tx: state.tx.clone(),
            capacity: state.mailbox_capacity,
        }
    }

    /// Enqueue a single write operation.
    ///
    /// Returns `Err(DbWriterError::MailboxFull)` if the bounded channel
    /// is full and would block.
    pub fn try_write(&self, message: DbWriterMessage) -> Result<(), DbWriterError> {
        self.tx
            .try_send(DbWriterMsg::Write {
                message,
                reply: None,
            })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => DbWriterError::MailboxFull {
                    pending: self.capacity,
                    capacity: self.capacity,
                },
                mpsc::error::TrySendError::Closed(_) => DbWriterError::NotStarted,
            })
    }

    /// Enqueue a batch of write operations atomically.
    pub fn try_write_batch(
        &self,
        messages: Vec<DbWriterMessage>,
    ) -> Result<(), DbWriterError> {
        self.tx
            .try_send(DbWriterMsg::WriteBatch {
                messages,
                reply: None,
            })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => DbWriterError::MailboxFull {
                    pending: self.capacity,
                    capacity: self.capacity,
                },
                mpsc::error::TrySendError::Closed(_) => DbWriterError::NotStarted,
            })
    }

    /// Returns the current number of pending messages in the mailbox.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.capacity - self.tx.capacity()
    }

    /// Returns whether callers should shed load (mailbox >= 80% full).
    #[must_use]
    pub fn should_shed(&self) -> bool {
        self.pending_count() >= shed_threshold()
    }

    /// Returns the configured mailbox capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shed_threshold_is_80_percent() {
        assert_eq!(shed_threshold(), 8_000);
    }

    #[test]
    fn mailbox_capacity_is_10k() {
        assert_eq!(DB_WRITER_MAILBOX_CAPACITY, 10_000);
    }

    #[test]
    fn shed_fraction_is_0_80() {
        assert!((SHED_FRACTION - 0.80).abs() < f64::EPSILON);
    }

    #[test]
    fn db_writer_ack_equality() {
        let ack1 = DbWriterAck { committed_count: 5 };
        let ack2 = DbWriterAck { committed_count: 5 };
        let ack3 = DbWriterAck { committed_count: 3 };
        assert_eq!(ack1, ack2);
        assert_ne!(ack1, ack3);
    }

    #[test]
    fn db_writer_error_display() {
        let err = DbWriterError::MailboxFull {
            pending: 10_000,
            capacity: 10_000,
        };
        assert!(err.to_string().contains("mailbox is full"));
        assert!(err.to_string().contains("10000"));

        let err = DbWriterError::StorageEngine {
            reason: "disk full".to_string(),
        };
        assert!(err.to_string().contains("disk full"));

        let err = DbWriterError::BatchCommitFailed {
            reason: "io error".to_string(),
        };
        assert!(err.to_string().contains("io error"));
    }

    #[test]
    fn db_writer_handle_try_write_respects_capacity() {
        let (tx, rx) = mpsc::channel(2);
        let handle = DbWriterHandle {
            tx,
            capacity: 2,
        };

        // Fill the channel.
        assert!(handle.try_write(make_test_message()).is_ok());
        assert!(handle.try_write(make_test_message()).is_ok());

        // Third write should fail with MailboxFull.
        let result = handle.try_write(make_test_message());
        assert!(
            matches!(result, Err(DbWriterError::MailboxFull { .. })),
            "expected MailboxFull, got {result:?}"
        );

        // Drain one.
        drop(rx);
    }

    #[test]
    fn db_writer_handle_should_shed_at_threshold() {
        let (tx, _rx) = mpsc::channel(10);
        let handle = DbWriterHandle {
            tx,
            capacity: 10,
        };

        // 80% of 10 is 8.
        assert!(!handle.should_shed());
        for _ in 0..7 {
            handle.try_write(make_test_message()).unwrap();
        }
        assert!(!handle.should_shed()); // 7 < 8

        handle.try_write(make_test_message()).unwrap(); // 8 == threshold
        assert!(handle.should_shed());
    }

    #[test]
    fn db_writer_msg_debug_format() {
        let msg = DbWriterMsg::Write {
            message: make_test_message(),
            reply: None,
        };
        let debug = format!("{msg:?}");
        assert!(debug.contains("Write"));
    }

    fn make_test_message() -> DbWriterMessage {
        use vo_types::InstanceId;
        DbWriterMessage::RecordInstanceStatus {
            instance_id: InstanceId::parse("01H5JYV4XHGSR2F8KZ9BWNRFMA").unwrap(),
            status_byte: 1,
        }
    }
}
