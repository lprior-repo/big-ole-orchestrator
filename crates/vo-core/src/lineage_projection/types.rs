//! Domain types for the lineage projection system (ADR-038).
//!
//! This module defines the core types for workflow lineage with execution epochs,
//! including epoch maps, carried state, signal routing, and projection rebuild scopes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable identifier for a workflow lineage (logical long-lived workflow).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LineageId(pub String);

/// Monotonically increasing epoch sequence number within a lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EpochId(pub u64);

impl EpochId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

/// A single event in the canonical event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalEvent {
    pub lineage_id: LineageId,
    pub epoch_id: EpochId,
    pub sequence: u64,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// The active epoch map tracks which epoch is active for each lineage.
/// This is the routing table for signal delivery and event targeting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochMap {
    /// Maps lineage_id -> active_epoch_id
    pub entries: BTreeMap<LineageId, EpochId>,
    /// True while a rollover transaction is in progress (atomicity guard)
    pub rollover_in_progress: bool,
}

impl EpochMap {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            rollover_in_progress: false,
        }
    }

    /// Returns the active epoch for a lineage, if one exists.
    pub fn active_epoch(&self, lineage_id: &LineageId) -> Option<EpochId> {
        self.entries.get(lineage_id).copied()
    }

    /// Checks if the given epoch matches the active one.
    pub fn is_active(&self, lineage_id: &LineageId, epoch_id: EpochId) -> bool {
        self.entries
            .get(lineage_id)
            .map_or(false, |active| *active == epoch_id)
    }

    /// Checks if an epoch is older than the active one.
    pub fn is_old_epoch(&self, lineage_id: &LineageId, epoch_id: EpochId) -> bool {
        if let Some(active) = self.entries.get(lineage_id) {
            epoch_id.as_u64() < active.as_u64()
        } else {
            false
        }
    }

    /// Registers a lineage at a given epoch (used after ContinuedAsNew rollover).
    pub fn register_epoch(&mut self, lineage_id: LineageId, epoch_id: EpochId) {
        self.entries.insert(lineage_id, epoch_id);
    }

    /// Unregisters a lineage from an old epoch (cleanup after rollover).
    pub fn unregister_epoch(&mut self, lineage_id: &LineageId) {
        self.entries.remove(lineage_id);
    }

    /// Sets the rollover-in-progress guard.
    pub fn set_rollover_in_progress(&mut self, in_progress: bool) {
        self.rollover_in_progress = in_progress;
    }

    /// Checks if rollover is in progress.
    pub fn is_rollover_in_progress(&self) -> bool {
        self.rollover_in_progress
    }
}

/// Carried state: minimal state transferred across a continue-as-new rollover.
/// Operational projections are carried; operator projections are discarded.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CarriedState {
    /// Operational state: execution context, work-item references, pending signals.
    /// These are carried forward because the workflow needs them to continue.
    pub operational: serde_json::Value,
    /// Operator state: UI display state, dashboard positions, annotations.
    /// These are NOT carried because they are rebuildable from canonical events.
    pub operator: serde_json::Value,
}

impl CarriedState {
    pub fn new(operational: serde_json::Value, operator: serde_json::Value) -> Self {
        Self {
            operational,
            operator,
        }
    }

    pub fn empty() -> Self {
        Self {
            operational: serde_json::Value::Null,
            operator: serde_json::Value::Null,
        }
    }
}

/// Which projection class a rebuild applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionClass {
    Operational,
    Operator,
}

/// Scope of a projection rebuild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebuildScope {
    /// Full rebuild from sequence 1 within a single epoch.
    FullEpoch {
        lineage_id: LineageId,
        epoch_id: EpochId,
    },
    /// Incremental rebuild: replay events from a starting sequence within an epoch.
    Incremental {
        lineage_id: LineageId,
        epoch_id: EpochId,
        from_sequence: u64,
    },
    /// Cross-epoch rebuild: replay across epoch boundaries.
    CrossEpoch {
        lineage_id: LineageId,
        from_epoch: EpochId,
        to_epoch: Option<EpochId>,
        from_sequence: u64,
    },
}

/// Signal buffering state during rollover.
/// Signals arriving for the old epoch are buffered until rollover completes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SignalBuffer {
    /// Buffered signals: lineage -> list of events destined for old epoch
    pub pending: BTreeMap<LineageId, Vec<CanonicalEvent>>,
}

impl SignalBuffer {
    pub fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
        }
    }

    /// Buffer a signal destined for an old epoch.
    pub fn buffer(&mut self, event: CanonicalEvent) {
        self.pending
            .entry(event.lineage_id.clone())
            .or_default()
            .push(event);
    }

    /// Drain all buffered signals for a lineage (after rollover completes).
    pub fn drain(&mut self, lineage_id: &LineageId) -> Vec<CanonicalEvent> {
        self.pending.remove(lineage_id).unwrap_or_default()
    }

    /// Check if there are buffered signals for a lineage.
    pub fn has_pending(&self, lineage_id: &LineageId) -> bool {
        self.pending
            .get(lineage_id)
            .map_or(false, |events| !events.is_empty())
    }

    /// Total buffered signal count.
    pub fn pending_count(&self) -> usize {
        self.pending.values().map(|v| v.len()).sum()
    }
}

/// Corruption state for a projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionCorruption {
    /// Checksum mismatch on loaded projection state.
    ChecksumMismatch { expected: String, actual: String },
    /// Schema version mismatch.
    SchemaVersionMismatch { expected: u8, actual: u8 },
    /// Sequence gap detected during incremental update.
    SequenceGap { gap_at: u64 },
    /// Unknown corruption type.
    Unknown,
}

/// The state of a projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProjectionState {
    Building,
    Ready {
        schema_version: u8,
        last_sequence: u64,
    },
    Stale {
        reason: String,
        detected_at: u64,
    },
    Rebuilding {
        progress: f64,
        from_sequence: u64,
    },
    Failed {
        reason: String,
        attempted_at: u64,
    },
}

/// An effect that has been executed in an epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutedEffect {
    pub effect_id: String,
    pub effect_type: String,
    pub epoch_id: EpochId,
    pub lineage_id: LineageId,
    pub status: EffectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectStatus {
    Executed,
    Compensated,
    PendingCompensation,
}

/// A ContinuedAsNew event written during rollover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuedAsNew {
    pub lineage_id: LineageId,
    pub old_epoch_id: EpochId,
    pub new_epoch_id: EpochId,
    pub carried_state: CarriedState,
    pub trigger: ContinuedAsNewTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContinuedAsNewTrigger {
    /// Event count exceeded threshold.
    EventCountThreshold { event_count: u64, threshold: u64 },
    /// Signal count exceeded threshold.
    SignalCountThreshold { signal_count: u64, threshold: u64 },
    /// Payload blob references became too numerous.
    BlobReferencesThreshold { blob_count: u64, threshold: u64 },
    /// Workflow explicitly requested rollover.
    Explicit,
}

/// A WorkflowStarted event for the successor epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStarted {
    pub lineage_id: LineageId,
    pub epoch_id: EpochId,
    pub carried_state: CarriedState,
    pub parent_epoch_id: EpochId,
}

/// A signal event destined for a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalEvent {
    pub lineage_id: LineageId,
    pub epoch_id: EpochId,
    pub signal_type: String,
    pub payload: serde_json::Value,
    pub received_at: u64,
}

/// Result of epoch routing for an incoming event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteResult {
    /// Event routed to active epoch.
    Routed {
        lineage_id: LineageId,
        epoch_id: EpochId,
        routed_to_active: bool,
    },
    /// Event rejected: epoch is old.
    OldEpochRejected {
        lineage_id: LineageId,
        event_epoch: EpochId,
        active_epoch: EpochId,
    },
    /// Event buffered: rollover in progress.
    Buffered {
        lineage_id: LineageId,
        epoch_id: EpochId,
    },
    /// Event accepted for new lineage (no prior epoch).
    NewLineage {
        lineage_id: LineageId,
        epoch_id: EpochId,
    },
}

/// Result of a continue-as-new rollover transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloverResult {
    pub lineage_id: LineageId,
    pub old_epoch_id: EpochId,
    pub new_epoch_id: EpochId,
    pub carried_state: CarriedState,
    pub events_written: Vec<CanonicalEvent>,
    pub steps_completed: usize,
    pub step_count: usize,
}

/// Result of a carried-state computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedStateResult {
    pub operational: serde_json::Value,
    pub operator_discarded: bool,
    pub is_valid: bool,
}

/// Result of a projection rebuild.
#[derive(Debug, Clone, PartialEq)]
pub struct RebuildResult {
    pub scope: RebuildScope,
    pub events_applied: u64,
    pub final_state: ProjectionState,
    pub rebuilt_from_canonical: bool,
}

/// Result of an atomic projection swap.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionSwapResult {
    pub projection_id: String,
    pub old_state: ProjectionState,
    pub new_state: ProjectionState,
    pub swapped: bool,
}

/// Corruption isolation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorruptionIsolationResult {
    pub lineage_id: LineageId,
    pub epoch_id: EpochId,
    pub corruption: ProjectionCorruption,
    pub isolated: bool,
    pub other_epochs_affected: bool,
    pub rebuild_scope: RebuildScope,
}
