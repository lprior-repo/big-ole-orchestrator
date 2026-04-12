//! Transaction coordinator types for distributed two-phase commit (ADR-041).
//!
//! Architecture: Data (TransactionState, ParticipantStatus, TransactionRecord)
//!             → Calc (apply_coordinator_transition, is_terminal, all_variants).
//!
//! This module defines the type system for coordinating distributed transactions
//! across multiple resources (connectors). No I/O, no engine integration —
//! pure types and state machine logic.

// ============================================================================
// Data Layer: Type Definitions
// ============================================================================

/// Lifecycle state of a distributed transaction coordinator.
///
/// Follows the two-phase commit (2PC) protocol:
/// - Init → Preparing → Prepared → Committing → Committed
/// - Init → Preparing → Prepared → RollingBack → RolledBack
/// - Any state except Committed/RolledBack can transition to Aborted on timeout/error
/// - Prepared state can transition to Committing or RollingBack
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TransactionState {
    /// Coordinator is initialized, no participants enrolled.
    Init,

    /// Coordinator is in the process of enrolling participants and sending prepare.
    Enrolling,

    /// Coordinator has sent prepare to all participants, awaiting responses.
    Preparing,

    /// All participants voted "prepared" — ready to commit.
    Prepared,

    /// Coordinator is sending commit to all participants.
    Committing,

    /// Transaction committed successfully (terminal).
    Committed,

    /// Coordinator is sending rollback to all participants.
    RollingBack,

    /// Transaction rolled back successfully (terminal).
    RolledBack,

    /// Transaction aborted due to timeout, participant failure, or coordinator crash.
    Aborted,

    /// Transaction outcome is ambiguous — recovery required (ADR-041 §3).
    Ambiguous,
}

/// Status of a participant in a distributed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ParticipantStatus {
    /// Participant is enrolled but not yet responded to prepare.
    Enrolled,

    /// Participant voted "prepared" — can commit or rollback.
    Prepared,

    /// Participant voted "rollback" or voted prepared but coordinator timed out.
    VotedRollback,

    /// Participant has committed the transaction.
    Committed,

    /// Participant has rolled back the transaction.
    RolledBack,

    /// Participant status is unknown — reconcile required.
    Unknown,
}

/// Decision made by the transaction coordinator after the prepare phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CoordinatorDecision {
    /// All participants voted prepared — proceed with commit.
    Commit,

    /// One or more participants voted rollback — abort transaction.
    Rollback,
}

/// Events that drive the TransactionState transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoordinatorTransition {
    /// Begin enrolling participants.
    BeginEnroll,

    /// All participants enrolled, begin prepare phase.
    BeginPrepare,

    /// A participant responded to prepare.
    ParticipantPrepared,

    /// A participant voted rollback.
    ParticipantRollback,

    /// All participants have responded (all prepared or any rollback).
    AllResponded,

    /// Coordinator decided to commit.
    DecideCommit,

    /// Coordinator decided to rollback.
    DecideRollback,

    /// Coordinator timed out waiting for participant responses.
    Timeout,

    /// Coordinator crashed and is recovering.
    Recover,

    /// Recovery determined transaction was committed.
    ReconcileCommitted,

    /// Recovery determined transaction was rolled back.
    ReconcileRolledBack,

    /// Recovery could not determine outcome — needs retry.
    ReconcileRetry,
}

/// Error returned when a coordinator state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorTransitionError {
    /// Attempted transition from a terminal state.
    TerminalStateTransition,

    /// Event not valid for the current state.
    InvalidTransition,

    /// Required votes not yet received.
    InsufficientVotes,
}

impl std::fmt::Display for CoordinatorTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinatorTransitionError::TerminalStateTransition => {
                write!(f, "Cannot transition from terminal transaction state")
            }
            CoordinatorTransitionError::InvalidTransition => {
                write!(f, "Invalid transaction coordinator state transition")
            }
            CoordinatorTransitionError::InsufficientVotes => {
                write!(f, "Insufficient participant votes to transition")
            }
        }
    }
}

impl std::error::Error for CoordinatorTransitionError {}

/// Record of a distributed transaction coordinator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TransactionRecord {
    transaction_id: String,
    state: TransactionState,
    decision: Option<CoordinatorDecision>,
    participants: Vec<ParticipantRecord>,
    created_at: Option<crate::types::TimestampMs>,
    prepared_at: Option<crate::types::TimestampMs>,
    committed_at: Option<crate::types::TimestampMs>,
}

/// Record of a single participant in a distributed transaction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ParticipantRecord {
    participant_id: String,
    status: ParticipantStatus,
    vote: Option<ParticipantVote>,
}

/// Vote cast by a participant during the prepare phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ParticipantVote {
    /// Participant is prepared to commit.
    Prepared,

    /// Participant wishes to rollback.
    Rollback,
}

// ============================================================================
// Calc Layer: Pure Functions
// ============================================================================

impl TransactionState {
    /// Check if this state is terminal (Committed, RolledBack, or Aborted).
    ///
    /// Note: Ambiguous is NOT terminal — it can be reconciled.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            TransactionState::Committed | TransactionState::RolledBack | TransactionState::Aborted
        )
    }

    /// Returns all TransactionState variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [TransactionState] {
        &[
            TransactionState::Init,
            TransactionState::Enrolling,
            TransactionState::Preparing,
            TransactionState::Prepared,
            TransactionState::Committing,
            TransactionState::Committed,
            TransactionState::RollingBack,
            TransactionState::RolledBack,
            TransactionState::Aborted,
            TransactionState::Ambiguous,
        ]
    }
}

impl ParticipantStatus {
    /// Returns all ParticipantStatus variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [ParticipantStatus] {
        &[
            ParticipantStatus::Enrolled,
            ParticipantStatus::Prepared,
            ParticipantStatus::VotedRollback,
            ParticipantStatus::Committed,
            ParticipantStatus::RolledBack,
            ParticipantStatus::Unknown,
        ]
    }
}

impl CoordinatorDecision {
    /// Returns all CoordinatorDecision variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [CoordinatorDecision] {
        &[CoordinatorDecision::Commit, CoordinatorDecision::Rollback]
    }
}

impl CoordinatorTransition {
    /// Returns all CoordinatorTransition variants in declaration order.
    #[must_use]
    pub const fn all_variants() -> &'static [CoordinatorTransition] {
        &[
            CoordinatorTransition::BeginEnroll,
            CoordinatorTransition::BeginPrepare,
            CoordinatorTransition::ParticipantPrepared,
            CoordinatorTransition::ParticipantRollback,
            CoordinatorTransition::AllResponded,
            CoordinatorTransition::DecideCommit,
            CoordinatorTransition::DecideRollback,
            CoordinatorTransition::Timeout,
            CoordinatorTransition::Recover,
            CoordinatorTransition::ReconcileCommitted,
            CoordinatorTransition::ReconcileRolledBack,
            CoordinatorTransition::ReconcileRetry,
        ]
    }
}

impl TransactionRecord {
    /// Construct a new TransactionRecord.
    ///
    /// Returns `None` if `transaction_id` is empty (INV-TC-001).
    #[must_use]
    pub fn new(
        transaction_id: String,
        state: TransactionState,
        decision: Option<CoordinatorDecision>,
        participants: Vec<ParticipantRecord>,
        created_at: Option<crate::types::TimestampMs>,
        prepared_at: Option<crate::types::TimestampMs>,
        committed_at: Option<crate::types::TimestampMs>,
    ) -> Option<Self> {
        if transaction_id.is_empty() {
            return None;
        }
        Some(Self {
            transaction_id,
            state,
            decision,
            participants,
            created_at,
            prepared_at,
            committed_at,
        })
    }

    #[must_use]
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    #[must_use]
    pub fn state(&self) -> TransactionState {
        self.state
    }

    #[must_use]
    pub fn decision(&self) -> Option<CoordinatorDecision> {
        self.decision
    }

    #[must_use]
    pub fn participants(&self) -> &[ParticipantRecord] {
        &self.participants
    }

    #[must_use]
    pub fn created_at(&self) -> Option<&crate::types::TimestampMs> {
        self.created_at.as_ref()
    }

    #[must_use]
    pub fn prepared_at(&self) -> Option<&crate::types::TimestampMs> {
        self.prepared_at.as_ref()
    }

    #[must_use]
    pub fn committed_at(&self) -> Option<&crate::types::TimestampMs> {
        self.committed_at.as_ref()
    }
}

impl ParticipantRecord {
    /// Construct a new ParticipantRecord.
    ///
    /// Returns `None` if `participant_id` is empty (INV-TC-002).
    #[must_use]
    pub fn new(
        participant_id: String,
        status: ParticipantStatus,
        vote: Option<ParticipantVote>,
    ) -> Option<Self> {
        if participant_id.is_empty() {
            return None;
        }
        Some(Self {
            participant_id,
            status,
            vote,
        })
    }

    #[must_use]
    pub fn participant_id(&self) -> &str {
        &self.participant_id
    }

    #[must_use]
    pub fn status(&self) -> ParticipantStatus {
        self.status
    }

    #[must_use]
    pub fn vote(&self) -> Option<ParticipantVote> {
        self.vote
    }
}

/// Apply a state transition to a TransactionState.
///
/// # Errors
///
/// Returns `CoordinatorTransitionError::TerminalStateTransition` if the current state
/// is terminal (Committed, RolledBack, or Aborted).
/// Returns `CoordinatorTransitionError::InvalidTransition` if the event is not valid
/// for the current state.
pub fn apply_coordinator_transition(
    current: TransactionState,
    event: CoordinatorTransition,
) -> Result<TransactionState, CoordinatorTransitionError> {
    match (current, event) {
        // Init transitions
        (TransactionState::Init, CoordinatorTransition::BeginEnroll) => {
            Ok(TransactionState::Enrolling)
        }

        // Enrolling transitions
        (TransactionState::Enrolling, CoordinatorTransition::BeginPrepare) => {
            Ok(TransactionState::Preparing)
        }

        // Preparing transitions
        (TransactionState::Preparing, CoordinatorTransition::ParticipantPrepared) => {
            Ok(TransactionState::Preparing)
        }
        (TransactionState::Preparing, CoordinatorTransition::ParticipantRollback) => {
            Ok(TransactionState::Preparing)
        }
        (TransactionState::Preparing, CoordinatorTransition::AllResponded) => {
            Ok(TransactionState::Prepared)
        }
        (TransactionState::Preparing, CoordinatorTransition::Timeout) => {
            Ok(TransactionState::Aborted)
        }

        // Prepared transitions
        (TransactionState::Prepared, CoordinatorTransition::DecideCommit) => {
            Ok(TransactionState::Committing)
        }
        (TransactionState::Prepared, CoordinatorTransition::DecideRollback) => {
            Ok(TransactionState::RollingBack)
        }
        (TransactionState::Prepared, CoordinatorTransition::Timeout) => {
            Ok(TransactionState::Aborted)
        }

        // Committing transitions
        (TransactionState::Committing, CoordinatorTransition::AllResponded) => {
            Ok(TransactionState::Committed)
        }
        (TransactionState::Committing, CoordinatorTransition::Timeout) => {
            Ok(TransactionState::Ambiguous)
        }

        // RollingBack transitions
        (TransactionState::RollingBack, CoordinatorTransition::AllResponded) => {
            Ok(TransactionState::RolledBack)
        }
        (TransactionState::RollingBack, CoordinatorTransition::Timeout) => {
            Ok(TransactionState::Ambiguous)
        }

        // Terminal states reject all transitions
        (TransactionState::Committed, _) => {
            Err(CoordinatorTransitionError::TerminalStateTransition)
        }
        (TransactionState::RolledBack, _) => {
            Err(CoordinatorTransitionError::TerminalStateTransition)
        }
        (TransactionState::Aborted, _) => Err(CoordinatorTransitionError::TerminalStateTransition),

        // Ambiguous recovery transitions
        (TransactionState::Ambiguous, CoordinatorTransition::ReconcileCommitted) => {
            Ok(TransactionState::Committed)
        }
        (TransactionState::Ambiguous, CoordinatorTransition::ReconcileRolledBack) => {
            Ok(TransactionState::RolledBack)
        }
        (TransactionState::Ambiguous, CoordinatorTransition::ReconcileRetry) => {
            Ok(TransactionState::Ambiguous)
        }

        // Recovery from any non-terminal state
        (state, CoordinatorTransition::Recover) if !state.is_terminal() => {
            Ok(TransactionState::Ambiguous)
        }

        // All other combinations are invalid
        _ => Err(CoordinatorTransitionError::InvalidTransition),
    }
}
