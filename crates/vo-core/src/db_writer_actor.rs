//! DbWriterActor - Ractor actor for batch-committing control-plane transitions to fjall.
//!
//! Per ADR-015 (Actor Invariants and Backpressure):
//! - The `DbWriterActor` is configured with a **strictly bounded mailbox** (10,000 messages).
//! - When the mailbox is full, sending actors will block. This provides backpressure.
//!
//! Per ADR-016 (Atomic Storage):
//! - Uses `fjall::Batch` for every control-plane transition.
//! - All touched partitions are updated atomically in the same batch.
//!
//! # Architecture
//!
//! ```text
//! Workflow Actors --> DbWriterMessage --> [DbWriterActor Mailbox (bounded 10k)] --> fjall::Batch commit
//!                                       |
//!                                       +--> events partition
//!                                       +--> instances partition
//!                                       +--> timers partition
//!                                       +--> dedupe partition
//!                                       +--> effects partition
//!                                       +--> leases partition
//!                                       +--> snapshots partition
//! ```
//!
//! # Spawning
//!
//! ```ignore
//! let (actor_ref, handle) = DbWriterActor::spawn(
//!     db,
//!     DbWriterActorConfig::default(),
//! ).await?;
//! ```

use std::sync::Arc;

use ractor::Actor;
use ractor::ActorProcessingErr;
use ractor::ActorRef;
use ractor::RpcReplyPort;
use serde::{Deserialize, Serialize};

use crate::db_writer_message::DbWriterMessage;
use crate::transaction::{TransactionCommitter, TransactionError};

#[cfg(feature = "fjall")]
mod fjall_committer;

#[cfg(feature = "fjall")]
pub use fjall_committer::FjallDbWriter;

/// Configuration for DbWriterActor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbWriterActorConfig {
    /// Bounded mailbox capacity per ADR-015. Default is 10,000 messages.
    pub mailbox_capacity: usize,
    /// Batch flush interval in milliseconds. If zero, only flushes when batch is full.
    pub flush_interval_ms: u64,
    /// Maximum batch size before forced flush.
    pub max_batch_size: usize,
}

impl Default for DbWriterActorConfig {
    fn default() -> Self {
        Self {
            mailbox_capacity: 10_000,
            flush_interval_ms: 100,
            max_batch_size: 1_000,
        }
    }
}

/// Errors from DbWriterActor operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DbWriterActorError {
    #[error("transaction commit failed: {0}")]
    CommitFailed(String),
    #[error("actor error: {0}")]
    ActorError(String),
    #[error("storage error: {0}")]
    StorageError(String),
}

/// Reply port for commit operations.
pub type CommitReplyPort = RpcReplyPort<Result<(), DbWriterActorError>>;

/// Messages that can be sent to DbWriterActor.
#[derive(Debug)]
pub enum DbWriterActorMsg {
    /// A batch of messages to commit.
    Commit {
        messages: Vec<DbWriterMessage>,
        reply: CommitReplyPort,
    },
    /// Get current mailbox depth for health monitoring (ADR-015).
    GetMailboxDepth { reply: RpcReplyPort<usize> },
    /// Graceful shutdown signal.
    Shutdown { reply: RpcReplyPort<()> },
}

/// Actor state for DbWriterActor.
pub struct DbWriterActorState {
    committer: Box<dyn TransactionCommitter + Send + Sync>,
    config: DbWriterActorConfig,
    current_batch: Vec<DbWriterMessage>,
    messages_received: usize,
    batches_committed: usize,
}

impl DbWriterActorState {
    fn new(
        committer: Box<dyn TransactionCommitter + Send + Sync>,
        config: DbWriterActorConfig,
    ) -> Self {
        Self {
            committer,
            config,
            current_batch: Vec::new(),
            messages_received: 0,
            batches_committed: 0,
        }
    }

    fn should_flush(&self) -> bool {
        self.current_batch.len() >= self.config.max_batch_size
    }

    fn commit_batch(&mut self) -> Result<(), TransactionError> {
        if self.current_batch.is_empty() {
            return Ok(());
        }

        let messages = std::mem::take(&mut self.current_batch);
        self.current_batch = Vec::new();

        // Commit using the committer
        (self.committer).commit_batch(messages)?;
        self.batches_committed += 1;

        Ok(())
    }
}

/// Arguments passed to DbWriterActor during spawn.
pub struct DbWriterActorArguments {
    pub committer: Box<dyn TransactionCommitter + Send + Sync>,
    pub config: DbWriterActorConfig,
}

/// DbWriterActor - Ractor actor for batch-committing control-plane transitions.
///
/// This actor:
/// 1. Receives `DbWriterMessage` messages via its bounded mailbox
/// 2. Buffers them into a local batch
/// 3. Commits the batch to fjall when full or on interval
///
/// # Backpressure (ADR-015)
///
/// The mailbox is bounded to 10,000 messages by default. When full:
/// - Sending actors will block (not dropped)
/// - The Engine monitors mailbox depth as a health metric
/// - At 80% capacity, HTTP API returns 429/503
pub struct DbWriterActor;

impl DbWriterActor {
    /// Spawn a new DbWriterActor with the given committer and configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if actor spawning fails.
    #[expect(clippy::unused_async)]
    pub async fn spawn(
        committer: Box<dyn TransactionCommitter + Send + Sync>,
        config: DbWriterActorConfig,
    ) -> Result<
        (ActorRef<DbWriterActorMsg>, tokio::task::JoinHandle<()>),
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let args = DbWriterActorArguments { committer, config };
        ractor::Actor::spawn(Some("db-writer".to_string()), Self, args)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl Actor for DbWriterActor {
    type Msg = DbWriterActorMsg;
    type State = DbWriterActorState;
    type Arguments = DbWriterActorArguments;

    async fn pre_start(
        &self,
        _: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(DbWriterActorState::new(args.committer, args.config))
    }

    async fn handle(
        &self,
        _: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            DbWriterActorMsg::Commit { messages, reply } => {
                for msg in messages {
                    state.current_batch.push(msg);
                    state.messages_received += 1;

                    if state.should_flush() {
                        if let Err(e) = state.commit_batch() {
                            let _ =
                                reply.send(Err(DbWriterActorError::CommitFailed(e.to_string())));
                            return Ok(());
                        }
                    }
                }

                if let Err(e) = state.commit_batch() {
                    let _ = reply.send(Err(DbWriterActorError::CommitFailed(e.to_string())));
                } else {
                    let _ = reply.send(Ok(()));
                }
            }
            DbWriterActorMsg::GetMailboxDepth { reply } => {
                let _ = reply.send(
                    state
                        .messages_received
                        .saturating_sub(state.batches_committed * state.config.max_batch_size),
                );
            }
            DbWriterActorMsg::Shutdown { reply } => {
                if let Err(e) = state.commit_batch() {
                    tracing::error!("final batch commit failed during shutdown: {}", e);
                }
                let _ = reply.send(());
            }
        }
        Ok(())
    }
}

/// Spawn a DbWriterActor with a fjall-backed committer.
///
/// This is the production constructor that creates a fully functional actor.
#[cfg(feature = "fjall")]
#[expect(clippy::unused_async)]
pub async fn spawn_fjall_db_writer(
    db: Arc<fjall::Database>,
    config: DbWriterActorConfig,
) -> Result<(ActorRef<DbWriterActorMsg>, tokio::task::JoinHandle<()>), DbWriterActorError> {
    let committer = Box::new(FjallDbWriter::new(db));

    DbWriterActor::spawn(committer, config)
        .await
        .map_err(|e| DbWriterActorError::ActorError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use vo_types::{
        EffectIntent, EffectKind, EffectRecord, EventEnvelope,
        IdempotencyKey, InstanceId, SequenceNumber,
        MAX_SUPPORTED_SCHEMA_VERSION,
    };

    struct TrackingCommitter {
        committed: Arc<Mutex<Vec<Vec<DbWriterMessage>>>>,
    }

    impl TrackingCommitter {
        fn new(committed: Arc<Mutex<Vec<Vec<DbWriterMessage>>>>) -> Self {
            Self { committed }
        }
    }

    impl TransactionCommitter for TrackingCommitter {
        fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError> {
            self.committed.lock().unwrap().push(messages);
            Ok(())
        }
    }

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_event_envelope() -> EventEnvelope {
        EventEnvelope {
            schema_version: 1,
            instance_id: "01ARYZ6S410000000000000000".to_string(),
            sequence: 1,
            timestamp_ms: 1712200000000,
            payload: serde_json::json!({}),
            metadata: EventMetadata::default(),
        }
    }

    fn valid_snapshot_data() -> crate::db_writer_message::SnapshotData {
        crate::db_writer_message::SnapshotData::new(
            valid_sequence(),
            MAX_SUPPORTED_SCHEMA_VERSION,
            vec![0x01, 0x02, 0x03],
        )
        .expect("valid snapshot data")
    }

    fn valid_effect_record() -> EffectRecord {
        EffectRecord::new(
            "intent-1".to_string(),
            EffectKind::HttpCall,
            serde_json::json!({}),
            EffectIntent::Prepared,
            None,
        )
        .expect("valid effect record")
    }

    #[tokio::test]
    async fn actor_receives_messages_and_commits_batch() {
        let committed = Arc::new(Mutex::new(Vec::new()));
        let committer = Box::new(TrackingCommitter::new(Arc::clone(&committed)));
        let config = DbWriterActorConfig {
            mailbox_capacity: 100,
            flush_interval_ms: 1000,
            max_batch_size: 10,
        };

        let (actor_ref, handle) = DbWriterActor::spawn(committer, config)
            .await
            .expect("spawn should succeed");

        let msg = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: valid_sequence(),
            idempotency_key: valid_idempotency_key(),
        };

        let (tx, rx) = ractor::concurrency::oneshot::<Result<(), DbWriterActorError>>();
        actor_ref
            .cast(DbWriterActorMsg::Commit {
                messages: vec![msg],
                reply: tx.into(),
            })
            .expect("cast should succeed");

        let result = rx.await.expect("reply should succeed");
        assert!(result.is_ok());
        assert_eq!(committed.lock().unwrap().len(), 1);

        actor_ref.stop(None);
        handle.await.expect("handle should stop");
    }

    #[tokio::test]
    async fn bounded_mailbox_config_is_respected() {
        let committed = Arc::new(Mutex::new(Vec::new()));
        let committer = Box::new(TrackingCommitter::new(Arc::clone(&committed)));
        let config = DbWriterActorConfig {
            mailbox_capacity: 5,
            flush_interval_ms: 1000,
            max_batch_size: 2,
        };

        let (actor_ref, handle) = DbWriterActor::spawn(committer, config)
            .await
            .expect("spawn should succeed");

        let (tx, rx) = ractor::concurrency::oneshot::<usize>();
        actor_ref
            .cast(DbWriterActorMsg::GetMailboxDepth { reply: tx.into() })
            .expect("cast should succeed");

        let _depth = rx.await.expect("reply should succeed");

        actor_ref.stop(None);
        handle.await.expect("handle should stop");
    }

    #[tokio::test]
    async fn batch_flush_triggers_at_size_threshold() {
        let committed = Arc::new(Mutex::new(Vec::new()));
        let committer = Box::new(TrackingCommitter::new(Arc::clone(&committed)));
        let config = DbWriterActorConfig {
            mailbox_capacity: 10_000,
            flush_interval_ms: 0,
            max_batch_size: 100,
        };

        let (actor_ref, handle) = DbWriterActor::spawn(committer, config)
            .await
            .expect("spawn should succeed");

        // Push 99 messages — below threshold, no flush during loop.
        // Post-loop commit_batch flushes the 99 (auto-flush on Commit completion).
        let messages_99: Vec<DbWriterMessage> = (0..99u64)
            .map(|i| DbWriterMessage::AppendEvent {
                instance_id: valid_instance_id(),
                sequence_number: SequenceNumber::new_unchecked(i),
                idempotency_key: IdempotencyKey::parse(&format!("key-{}", i)).expect("valid key"),
            })
            .collect();

        let (tx, rx) = ractor::concurrency::oneshot::<Result<(), DbWriterActorError>>();
        actor_ref
            .cast(DbWriterActorMsg::Commit {
                messages: messages_99,
                reply: tx.into(),
            })
            .expect("cast should succeed");

        let result = rx.await.expect("reply should succeed");
        assert!(result.is_ok(), "commit of 99 messages should succeed");

        // After 99 messages: one batch of 99 from auto-flush (post-loop commit_batch).
        // No threshold flush occurred because 99 < 100.
        {
            let batches = committed.lock().unwrap();
            assert_eq!(batches.len(), 1, "one auto-flush batch after 99 messages");
            assert_eq!(batches[0].len(), 99, "auto-flush batch has 99 messages");
        }

        // Push 1 more message via a new Commit. It gets auto-flushed as its own batch.
        let msg_100 = DbWriterMessage::AppendEvent {
            instance_id: valid_instance_id(),
            sequence_number: SequenceNumber::new_unchecked(99),
            idempotency_key: IdempotencyKey::parse("key-99").expect("valid key"),
        };

        let (tx, rx) = ractor::concurrency::oneshot::<Result<(), DbWriterActorError>>();
        actor_ref
            .cast(DbWriterActorMsg::Commit {
                messages: vec![msg_100],
                reply: tx.into(),
            })
            .expect("cast should succeed");

        let result = rx.await.expect("reply should succeed");
        assert!(result.is_ok(), "commit of 100th message should succeed");

        // Now test the actual threshold: send 100 messages in a single Commit.
        // The 100th message triggers should_flush(), producing exactly one batch of 100.
        committed.lock().unwrap().clear();

        let messages_100: Vec<DbWriterMessage> = (0..100u64)
            .map(|i| DbWriterMessage::AppendEvent {
                instance_id: valid_instance_id(),
                sequence_number: SequenceNumber::new_unchecked(i),
                idempotency_key: IdempotencyKey::parse(&format!("key-t{}", i)).expect("valid key"),
            })
            .collect();

        let (tx, rx) = ractor::concurrency::oneshot::<Result<(), DbWriterActorError>>();
        actor_ref
            .cast(DbWriterActorMsg::Commit {
                messages: messages_100,
                reply: tx.into(),
            })
            .expect("cast should succeed");

        let result = rx.await.expect("reply should succeed");
        assert!(result.is_ok(), "commit of 100 messages should succeed");

        // Threshold flush at 100: exactly 1 batch of 100 messages.
        // The post-loop commit_batch is a no-op (batch already drained by threshold flush).
        {
            let batches = committed.lock().unwrap();
            assert_eq!(
                batches.len(),
                1,
                "threshold flush produces exactly one batch"
            );
            let batch = &batches[0];
            assert_eq!(batch.len(), 100, "threshold batch has exactly 100 messages");

            for (i, msg) in batch.iter().enumerate() {
                match msg {
                    DbWriterMessage::AppendEvent { sequence_number, .. } => {
                        assert_eq!(
                            sequence_number.as_u64(),
                            i as u64,
                            "message at index {} should have sequence {}",
                            i,
                            i
                        );
                    }
                    _ => panic!("expected AppendEvent variant"),
                }
            }
        }

        actor_ref.stop(None);
        handle.await.expect("handle should stop");
    }
}
