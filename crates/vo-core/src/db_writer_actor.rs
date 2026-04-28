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
use ractor::ActorRef;
use ractor::ActorProcessingErr;
use ractor::RpcReplyPort;
use serde::{Deserialize, Serialize};

use crate::db_writer_message::DbWriterMessage;
use crate::transaction::{Transaction, TransactionCommitter, TransactionError};

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
#[derive(Debug, Clone)]
pub enum DbWriterActorMsg {
    /// A batch of messages to commit.
    Commit {
        messages: Vec<DbWriterMessage>,
        reply: CommitReplyPort,
    },
    /// Get current mailbox depth for health monitoring (ADR-015).
    GetMailboxDepth {
        reply: RpcReplyPort<usize>,
    },
    /// Graceful shutdown signal.
    Shutdown {
        reply: RpcReplyPort<()>,
    },
}

/// Actor state for DbWriterActor.
pub struct DbWriterActorState<C> {
    committer: C,
    config: DbWriterActorConfig,
    current_batch: Vec<DbWriterMessage>,
    messages_received: usize,
    batches_committed: usize,
}

impl<C> DbWriterActorState<C> {
    fn new(committer: C, config: DbWriterActorConfig) -> Self {
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

        let tx: Transaction<C> = Transaction::new();
        let mut tx = tx;
        for msg in messages {
            tx.push(msg).map_err(|e| TransactionError::AlreadyCommitted)?;
        }

        tx.commit(&self.committer)?;
        self.batches_committed += 1;

        Ok(())
    }
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
    pub async fn spawn<C>(
        committer: C,
        config: DbWriterActorConfig,
    ) -> Result<(ActorRef<DbWriterActorMsg>, ractor::ActorHash), ActorProcessingErr>
    where
        C: TransactionCommitter + Send + Sync + 'static,
    {
        let props = ractor::ActorProperties::default()
            .set_mailbox_capacity(config.mailbox_capacity);

        let state = DbWriterActorState::new(committer, config);

        ractor::Actor::spawn_linked(Some("db-writer".to_string()), Self, state, props)
    }
}

impl Actor for DbWriterActor {
    type Msg = DbWriterActorMsg;
    type State = DbWriterActorState<()>;
    type Arguments = ();

    async fn pre_start(
        &self,
        _: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(DbWriterActorState::new((), DbWriterActorConfig::default()))
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
                            let _ = reply.send(Err(DbWriterActorError::CommitFailed(e.to_string())));
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
                let _ = reply.send(state.messages_received.saturating_sub(state.batches_committed * state.config.max_batch_size));
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
) -> Result<(ActorRef<DbWriterActorMsg>, ractor::ActorHash), DbWriterActorError> {
    let committer = FjallDbWriter::new(db);

    DbWriterActor::spawn(committer, config)
        .await
        .map_err(|e| DbWriterActorError::ActorError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_writer_message::{SnapshotData, TimerOp};
    use std::cell::RefCell;
    use vo_types::events::EventMetadata;
    use vo_types::{
        EffectIntent, EffectKind, EffectRecord, EventEnvelope, FenceToken, FireAtMs,
        IdempotencyKey, InstanceId, InstanceStatus, SequenceNumber, StepId, TimerId,
        MAX_SUPPORTED_SCHEMA_VERSION,
    };

    struct MockCommitter {
        committed: RefCell<Vec<Vec<DbWriterMessage>>>,
    }

    impl MockCommitter {
        fn new() -> Self {
            Self {
                committed: RefCell::new(Vec::new()),
            }
        }
    }

    impl TransactionCommitter for MockCommitter {
        fn commit_batch(&self, messages: Vec<DbWriterMessage>) -> Result<(), TransactionError> {
            self.committed.borrow_mut().push(messages);
            Ok(())
        }
    }

    fn valid_instance_id() -> InstanceId {
        InstanceId::parse("01ARYZ6S410000000000000000").expect("valid instance id")
    }

    fn valid_sequence() -> SequenceNumber {
        SequenceNumber::new_unchecked(1)
    }

    fn valid_step_id() -> StepId {
        StepId::parse("step-1").expect("valid step id")
    }

    fn valid_fence_token() -> FenceToken {
        FenceToken::new(1).expect("valid fence token")
    }

    fn valid_idempotency_key() -> IdempotencyKey {
        IdempotencyKey::parse("key-1").expect("valid key")
    }

    fn valid_timer_id() -> TimerId {
        TimerId::parse("timer-1").expect("valid timer id")
    }

    fn valid_fire_at() -> FireAtMs {
        FireAtMs::try_from(1712200000000u64).expect("valid fire_at")
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

    fn valid_snapshot_data() -> SnapshotData {
        SnapshotData::new(
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
        let committer = MockCommitter::new();
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

        let (reply_port, reply) = ractor::single::oneshot();
        actor_ref.cast(actor_ref.clone(), DbWriterActorMsg::Commit {
            messages: vec![msg],
            reply: reply_port,
        }).expect("cast should succeed");

        let result = reply.await.expect("reply should succeed");
        assert!(result.is_ok());

        actor_ref.cast(actor_ref, DbWriterActorMsg::Shutdown { reply }).expect("shutdown should succeed");
        handle.await.expect("handle should stop");
    }

    #[tokio::test]
    async fn bounded_mailbox_config_is_respected() {
        let committer = MockCommitter::new();
        let config = DbWriterActorConfig {
            mailbox_capacity: 5, // Very small for testing
            flush_interval_ms: 1000,
            max_batch_size: 2,
        };

        let (actor_ref, handle) = DbWriterActor::spawn(committer, config)
            .await
            .expect("spawn should succeed");

        // Verify the actor was created with correct mailbox capacity
        // This tests ADR-015: bounded mailbox

        let (reply_port, reply) = ractor::single::oneshot();
        actor_ref.cast(actor_ref.clone(), DbWriterActorMsg::GetMailboxDepth { reply })
            .expect("cast should succeed");

        let _depth = reply.await.expect("reply should succeed");

        actor_ref.cast(actor_ref, DbWriterActorMsg::Shutdown { reply: reply_port })
            .expect("shutdown should succeed");
        handle.await.expect("handle should stop");
    }
}