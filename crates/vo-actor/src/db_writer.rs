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
use vo_types::{InstanceId, InstanceStatus, SequenceNumber, TimestampMs};

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
        apply_message_to_batch(db, message, &mut batch);
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
            apply_message_to_batch(db, msg, &mut batch);
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
fn apply_message_to_batch(
    db: &fjall::Database,
    message: &DbWriterMessage,
    batch: &mut fjall::OwnedWriteBatch,
) {
    match message {
        DbWriterMessage::AppendEvent {
            instance_id,
            sequence_number,
            idempotency_key: _,
        } => {
            let events_ks = db
                .keyspace(
                    vo_storage::partitions::EVENTS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("events partition");
            let key = vo_storage::key_encoding::encode_event_key(
                instance_id,
                *sequence_number,
            );
            batch.insert(&events_ks, key, &[] as &[u8]);
        }
        DbWriterMessage::RecordInstanceStatus {
            instance_id,
            status_byte,
        } => {
            let instances_ks = db
                .keyspace(
                    vo_storage::partitions::INSTANCES_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("instances partition");
            let status =
                vo_types::InstanceStatus::from_byte(*status_byte)
                    .unwrap_or(vo_types::InstanceStatus::Running);
            let now = vo_types::TimestampMs::now();
            let key = vo_storage::instance_index::encode_instance_index_key(
                status,
                now,
                instance_id,
            )
            .expect("instance index key");
            batch.insert(&instances_ks, key, &[] as &[u8]);
        }
        DbWriterMessage::AcquireLease {
            instance_id,
            step_id,
            fence: _,
        } => {
            let leases_ks = db
                .keyspace(
                    vo_storage::partitions::LEASE_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("leases partition");
            let key = format!("{instance_id}::{step_id}").into_bytes();
            batch.insert(&leases_ks, key, &[] as &[u8]);
        }
        DbWriterMessage::ReleaseLease {
            instance_id,
            step_id,
        } => {
            let leases_ks = db
                .keyspace(
                    vo_storage::partitions::LEASE_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("leases partition");
            let key = format!("{instance_id}::{step_id}").into_bytes();
            batch.remove(&leases_ks, key);
        }
        DbWriterMessage::UpsertTimer {
            instance_id,
            timer_id,
            fire_at,
        } => {
            let timers_ks = db
                .keyspace(
                    vo_storage::partitions::TIMERS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("timers partition");
            let key = encode_timer_key(instance_id, timer_id, fire_at);
            batch.insert(&timers_ks, key, &[] as &[u8]);
        }
        DbWriterMessage::DeleteTimer {
            instance_id,
            timer_id,
        } => {
            let timers_ks = db
                .keyspace(
                    vo_storage::partitions::TIMERS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("timers partition");
            let fire_at_ms = 0u64;
            let key = encode_timer_key_raw(instance_id, timer_id, fire_at_ms);
            batch.remove(&timers_ks, key);
        }
        DbWriterMessage::RecordEffect { effect } => {
            let effects_ks = db
                .keyspace(
                    vo_storage::partitions::EFFECTS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("effects partition");
            let placeholder_instance_id = InstanceId::from_bytes([0u8; 16]);
            let effect_id = vo_storage::effect_journal::EffectId::new(
                &placeholder_instance_id,
                effect.intent_id(),
            )
            .expect("valid effect id");
            let key = vo_storage::effect_journal::encode_effect_key(&effect_id);
            let value = vo_storage::effect_journal::encode_effect_record(effect)
                .expect("encode effect record");
            batch.insert(&effects_ks, key, &value);
        }
        DbWriterMessage::TakeSnapshot {
            instance_id,
            sequence_number,
            snapshot_data,
        } => {
            let snapshots_ks = db
                .keyspace(
                    vo_storage::partitions::SNAPSHOTS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("snapshots partition");
            let key = vo_storage::snapshots::encode_snapshot_key(
                instance_id,
                sequence_number.as_u64(),
            )
            .expect("snapshot key");
            let value =
                serde_json::to_vec(snapshot_data).expect("serialize snapshot");
            batch.insert(&snapshots_ks, key, &value);
        }
        DbWriterMessage::AtomicTransition {
            step_id: _,
            instance_status,
            timer_ops,
            snapshot,
            event,
        } => {
            let events_ks = db
                .keyspace(
                    vo_storage::partitions::EVENTS_PARTITION,
                    fjall::KeyspaceCreateOptions::default,
                )
                .expect("events partition");
            let event_key = vo_storage::key_encoding::encode_event_key(
                &vo_types::InstanceId::parse(&event.instance_id)
                    .expect("valid instance id"),
                vo_types::SequenceNumber::try_from(event.sequence)
                    .expect("valid sequence"),
            );
            let event_value =
                serde_json::to_vec(event).expect("serialize event envelope");
            batch.insert(&events_ks, event_key, &event_value);

            if let Some(status) = instance_status {
                let instances_ks = db
                    .keyspace(
                        vo_storage::partitions::INSTANCES_PARTITION,
                        fjall::KeyspaceCreateOptions::default,
                    )
                    .expect("instances partition");
                let now = vo_types::TimestampMs::now();
                let instance_id = vo_types::InstanceId::parse(&event.instance_id)
                    .expect("valid instance id");
                let key = vo_storage::instance_index::encode_instance_index_key(
                    *status,
                    now,
                    &instance_id,
                )
                .expect("instance index key");
            batch.insert(&instances_ks, key, &[] as &[u8]);
            }

            for op in timer_ops {
                match op {
                    vo_core::db_writer_message::TimerOp::Upsert {
                        timer_id,
                        fire_at,
                    } => {
                        let timers_ks = db
                            .keyspace(
                                vo_storage::partitions::TIMERS_PARTITION,
                                fjall::KeyspaceCreateOptions::default,
                            )
                            .expect("timers partition");
                        let instance_id =
                            vo_types::InstanceId::parse(&event.instance_id)
                                .expect("valid instance id");
                        let key =
                            encode_timer_key(&instance_id, timer_id, fire_at);
                        batch.insert(&timers_ks, key, &[] as &[u8]);
                    }
                    vo_core::db_writer_message::TimerOp::Delete {
                        timer_id,
                    } => {
                        let timers_ks = db
                            .keyspace(
                                vo_storage::partitions::TIMERS_PARTITION,
                                fjall::KeyspaceCreateOptions::default,
                            )
                            .expect("timers partition");
                        let instance_id =
                            vo_types::InstanceId::parse(&event.instance_id)
                                .expect("valid instance id");
                        let key = encode_timer_key_raw(
                            &instance_id,
                            timer_id,
                            0u64,
                        );
                        batch.remove(&timers_ks, key);
                    }
                }
            }

            if let Some(snap) = snapshot {
                let snapshots_ks = db
                    .keyspace(
                        vo_storage::partitions::SNAPSHOTS_PARTITION,
                        fjall::KeyspaceCreateOptions::default,
                    )
                    .expect("snapshots partition");
                let instance_id =
                    vo_types::InstanceId::parse(&event.instance_id)
                        .expect("valid instance id");
                let key = vo_storage::snapshots::encode_snapshot_key(
                    &instance_id,
                    snap.sequence_number().as_u64(),
                )
                .expect("snapshot key");
                let value =
                    serde_json::to_vec(snap).expect("serialize snapshot");
batch.insert(&snapshots_ks, key, &value);
            }
        }
    }
}

fn encode_timer_key(
    instance_id: &vo_types::InstanceId,
    timer_id: &vo_types::TimerId,
    fire_at: &vo_types::FireAtMs,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + 16 + 16);
    key.extend_from_slice(&fire_at.as_u64().to_be_bytes());
    key.extend_from_slice(
        &instance_id.to_bytes().expect("instance id to bytes"),
    );
    key.extend_from_slice(
        &timer_id.to_bytes().expect("timer id to bytes"),
    );
    key
}

fn encode_timer_key_raw(
    instance_id: &vo_types::InstanceId,
    timer_id: &vo_types::TimerId,
    fire_at_ms: u64,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + 16 + 16);
    key.extend_from_slice(&fire_at_ms.to_be_bytes());
    key.extend_from_slice(
        &instance_id.to_bytes().expect("instance id to bytes"),
    );
    key.extend_from_slice(
        &timer_id.to_bytes().expect("timer id to bytes"),
    );
    key
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
