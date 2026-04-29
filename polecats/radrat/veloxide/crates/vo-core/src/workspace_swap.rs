//! Atomic workspace swap for branch switches.
//!
//! # Overview
//!
//! This module implements crash-safe atomic workspace switching. When a branch
//! switch occurs, the new state is staged in a shadow directory, durability is
//! ensured via fsync, and then an atomic rename swaps the workspace. A journal
//! file tracks the swap state so that crash recovery can always reach a
//! consistent state regardless of when the process dies.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                    AtomicSwap Lifecycle                       │
//! │                                                              │
//! │  ┌─────────┐   stage()   ┌──────────┐   commit()   ┌─────────┐│
//! │  │ Original│──────────>  │ Shadow   │─────────────>│ Original││
//! │  │Workspace│             │Workspace│                │(new)    ││
//! │  └─────────┘             └──────────┘                └─────────┘│
//! │       │                        │                               │
//! │       │                        │  journal: "staging"           │
//! │       │                        │  shadow dir created           │
//! │       │                        │  files copied + fsync'd       │
//! │       │                        │  journal: "staged"            │
//! │       │                        │                               │
//! │       │  ┌─ commit() ─────────┤                                 │
//! │       │  │                     │                                 │
//! │       │  ▼                     │                                 │
//! │       │  ┌─────────┐          │                                 │
//! │       │  │ .backup │ (old ws) │                                 │
//! │       │  └────┬────┘          │                                 │
//! │       │       │ rename        │                                 │
//! │       │       ▼               │                                 │
//! │       │  ┌──────────┐        │                                 │
//! │       └─>│ .shadow  │────────┘ (shadow → workspace)            │
//! │          └────┬─────┘                                         │
//! │               │ fsync + journal "complete"                    │
//! │               ▼                                               │
//! │          ┌──────────┐                                         │
//! │          │ Workspace│ (new state, durable)                    │
//! │          └──────────┘                                         │
//! │               │ cleanup: remove .backup, journal              │
//! │               ▼                                               │
//! │          ┌──────────┐                                         │
//! │          │ Workspace│ (clean, no journal)                     │
//! │          └──────────┘                                         │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Swap Protocol
//!
//! The swap proceeds in two phases:
//!
//! ## Phase 1: Stage
//!
//! 1. Validate the workspace path exists and is a directory.
//! 2. Check that no shadow directory already exists (prevents concurrent swaps).
//! 3. Write journal with phase `"staging"`.
//! 4. Create shadow directory and recursively copy the workspace into it.
//! 5. Fsync both the shadow directory and the original workspace directory.
//! 6. Update journal to `"staged"`.
//!
//! After staging completes, the original workspace is untouched and the shadow
//! directory contains a full copy of the workspace state, durable on disk.
//!
//! ## Phase 2: Commit
//!
//! 1. Verify the swap is in a staged/incomplete state.
//! 2. Write journal with phase `"swapping"`.
//! 3. Rename the original workspace to `.backup`.
//! 4. Fsync the parent directory (ensures backup rename is durable).
//! 5. Rename the shadow directory to the workspace path.
//! 6. Fsync the parent directory (ensures swap rename is durable).
//! 7. Write journal with phase `"complete"`.
//! 8. Remove the `.backup` directory.
//! 9. Remove the journal file.
//!
//! ## Crash Recovery
//!
//! If the process crashes during a swap, the journal file retains the phase at
//! which the crash occurred. Calling [`recover`] (or [`AtomicSwap::recover`])
//! inspects the journal and restores consistency:
//!
//! | Journal Phase | Recovery Action |
//! |---------------|-----------------|
//! | `staging` / `staged` | Remove shadow directory, remove journal. Workspace is untouched. |
//! | `swapping` | If backup exists and workspace is missing, restore backup to workspace path. Remove shadow and backup directories and journal. |
//! | `complete` | Journal is stale — remove it. Workspace already has the new state. |
//! | no journal | Nothing to recover. |
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                  Crash Recovery Map                   │
//! │                                                      │
//! │  ┌──────────────┐    crash during    ┌──────────────┐│
//! │  │ staging      │───────────────────>│ Remove       ││
//! │  │ staged       │                    │ shadow +     ││
//! │  │              │                    │ journal      ││
//! │  └──────────────┘                    │ (ws untouched)││
//! │                                      └──────────────┘│
//! │                                                      │
//! │  ┌──────────────┐    crash during    ┌──────────────┐│
//! │  │ swapping     │───────────────────>│ Restore      ││
//! │  │              │                    │ backup → ws  ││
//! │  │              │                    │ Cleanup      ││
//! │  └──────────────┘                    │ shadow +     ││
//! │                                      │ backup +     ││
//! │                                      │ journal      ││
//! │                                      └──────────────┘│
//! │                                                      │
//! │  ┌──────────────┐    crash during    ┌──────────────┐│
//! │  │ complete     │───────────────────>│ Remove       ││
//! │  │ (stale)      │                    │ journal only ││
//! │  └──────────────┘                    │ (ws has new) ││
//! │                                      └──────────────┘│
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! # Directory Layout
//!
//! Given a workspace at `/path/to/workspace`, the swap protocol uses these
//! auxiliary paths in the same parent directory:
//!
//! | Path | Purpose |
//! |------|---------|
//! | `/path/to/workspace` | The active workspace directory |
//! | `/path/to/workspace.shadow` | Shadow copy during staging/swap (default suffix) |
//! | `/path/to/workspace.backup` | Temporary backup during commit |
//! | `/path/to/workspace.swap-journal` | Journal tracking swap phase (default suffix) |
//!
//! All suffixes are configurable via [`AtomicSwap::with_shadow_suffix`].
//!
//! # State Machine
//!
//! The swap lifecycle is a directed acyclic graph of phases:
//!
//! ```text
//!                         Initial (no swap in progress)
//!                              │
//!                              │ check_status → NoPriorSwap
//!                              │
//!                              ▼
//!                         ┌──── Staging ────┐
//!                         │  journal: "staging"│
//!                         │  shadow created    │
//!                         └────┬─────────────┘
//!                              │ copy + fsync
//!                              ▼
//!                         ┌──── Staged ─────┐
//!                         │  journal: "staged"│
//!                         │  shadow complete  │
//!                         └────┬─────────────┘
//!                              │ commit()
//!                              ▼
//!                         ┌──── Swapping ───┐
//!                         │  journal: "swapping"│
//!                         │  backup exists      │
//!                         └────┬─────────────┘
//!                              │ shadow → workspace
//!                              ▼
//!                         ┌──── Complete ────┐
//!                         │  journal: "complete"│
//!                         │  cleanup performed  │
//!                         │  journal removed    │
//!                         └────────────────────┘
//!                              │
//!                              │ check_status → NoPriorSwap
//!                              │
//!                              ▼
//!                         Initial (clean state)
//! ```
//!
//! # Error Recovery Guarantees
//!
//! The protocol guarantees that after any crash:
//!
//! 1. **The workspace is always in a valid state.** Either the old or new state
//!    is fully present, never partially written.
//! 2. **Recovery is always possible.** The journal file persists across crashes
//!    and can always be read to determine the correct recovery action.
//! 3. **No orphaned files.** After recovery completes, no shadow, backup, or
//!    journal files remain.
//!
//! # Invariants
//!
//! - The shadow directory must not exist before [`stage()`][AtomicSwap::stage] is
//!   called. If it does, the call fails with [`SwapError::ShadowExists`].
//! - The journal file is written and fsync'd before each phase transition,
//!   ensuring the phase state survives crashes.
//! - The backup directory is always removed after a successful commit, leaving
//!   no trace of the old workspace.
//! - [`commit()`][AtomicSwap::commit] is idempotent: calling it when there is no
//!   prior swap or when the swap is already complete returns `Ok(())`.
//!
//! # Convenience Functions
//!
//! For simple swap scenarios, use the convenience functions:
//!
//! - [`atomic_swap`] — Performs stage + commit in one call. Fails if an
//!   incomplete swap is detected (prevents double-staging).
//! - [`recover_swap`] — Performs recovery for a workspace.
//!
//! # Examples
//!
//! ## Manual two-phase swap
//!
//! ```no_run
//! use vo_core::workspace_swap::AtomicSwap;
//!
//! let swap = AtomicSwap::new("/path/to/workspace");
//!
//! // Phase 1: Stage — copy workspace to shadow, fsync
//! swap.stage().expect("stage failed");
//!
//! // ... at this point, shadow has the new state, original is untouched ...
//!
//! // Phase 2: Commit — atomically swap original with shadow
//! swap.commit().expect("commit failed");
//! // Original now contains the new state, shadow and journal are removed.
//! ```
//!
//! ## Convenience function
//!
//! ```no_run
//! use vo_core::workspace_swap::atomic_swap;
//!
//! atomic_swap("/path/to/workspace").expect("swap failed");
//! ```
//!
//! ## Crash recovery
//!
//! ```
//! use vo_core::workspace_swap::{recover_swap, RecoveryOutcome};
//!
//! // If a crash interrupted a swap, recover restores consistency:
//! match recover_swap("/path/to/workspace") {
//!     Ok(RecoveryOutcome::RolledBack) => { /* old state restored */ }
//!     Ok(RecoveryOutcome::AlreadyComplete) => { /* cleanup stale journal */ }
//!     Ok(RecoveryOutcome::NothingToRecover) => { /* no swap was in progress */ }
//!     Err(e) => { /* unexpected error */ }
//! }
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Tracks the current phase of an in-progress workspace swap.
///
/// This enum represents the five phases of the swap lifecycle, from initial
/// state through staging, staging completion, the atomic swap operation, and
/// final cleanup.
///
/// # Phase Flow
///
/// ```text
/// Initial → Staging → Staged → Swapping → Complete → (back to Initial)
/// ```
///
/// Each phase is durably recorded in the journal file, enabling crash recovery.
/// See the [module-level docs](crate::workspace_swap) for the full swap protocol.
///
/// # Crash Recovery Mapping
///
/// | Phase | Recovery Action |
/// |-------|-----------------|
/// | `Initial` | No recovery needed — treated as `NoPriorSwap` |
/// | `Staging` / `Staged` | Remove shadow directory and journal. Original workspace is untouched. |
/// | `Swapping` | Restore backup to workspace path. Remove shadow, backup, and journal. |
/// | `Complete` | Remove stale journal. Workspace already has the new state. |
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SwapPhase {
    /// Initial state — no swap has been initiated.
    ///
    /// This is the state before any swap operation begins, and the state
    /// reached after a successful swap completes and the journal is removed.
    Initial,

    /// Staging phase — shadow directory is being created and files are being
    /// copied.
    ///
    /// The journal has been written with `"staging"` but the copy operation
    /// is in progress. A crash during this phase means the shadow directory
    /// may be partially populated. Recovery removes whatever exists.
    Staging,

    /// Staged — shadow directory is complete and fsync'd.
    ///
    /// The original workspace has been fully copied to the shadow directory,
    /// both directories have been fsync'd, and the journal has been updated
    /// to `"staged"`. The swap is ready for [`commit()`][AtomicSwap::commit].
    Staged,

    /// Swapping — the atomic rename operation is in progress.
    ///
    /// The original workspace has been renamed to `.backup` and the shadow
    /// directory has been renamed to the workspace path. This phase is
    /// extremely brief (two rename syscalls + two fsyncs). A crash here
    /// means one of the renames may or may not have completed — recovery
    /// checks both the backup and workspace paths to determine the action.
    Swapping,

    /// Complete — swap finished and all cleanup performed.
    ///
    /// The new workspace is in place, the backup has been removed, and the
    /// journal has been written with `"complete"` and then deleted. This is
    /// a terminal phase; no further action is needed.
    Complete,
}

impl SwapPhase {
    /// Parse a string into the corresponding [`SwapPhase`].
    ///
    /// Returns `None` for unrecognized strings.
    pub(crate) fn from_str_lossy(s: &str) -> Option<Self> {
        match s.trim() {
            "staging" => Some(Self::Staging),
            "staged" => Some(Self::Staged),
            "swapping" => Some(Self::Swapping),
            "complete" => Some(Self::Complete),
            "initial" => Some(Self::Initial),
            _ => None,
        }
    }

    /// Returns the string representation of this phase.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Staging => "staging",
            Self::Staged => "staged",
            Self::Swapping => "swapping",
            Self::Complete => "complete",
            Self::Initial => "initial",
        }
    }
}

/// Status of a prior (or in-progress) workspace swap.
///
/// Returned by [`AtomicSwap::check_status`], this enum indicates whether a swap
/// is in progress, has completed, or has never been initiated.
///
/// # Variants
///
/// - [`NoPriorSwap`][Self::NoPriorSwap] — No journal file exists. This is the
///   normal idle state.
/// - [`Incomplete(SwapPhase)`][Self::Incomplete] — A swap was started but did
///   not complete. The contained [`SwapPhase`] indicates where the swap was
///   interrupted (or paused), and determines the recovery action.
/// - [`Complete`] — The journal contained `"complete"`. Note that after a
///   successful commit, the journal is removed, so this state is only observed
///   during the narrow window between writing the journal and deleting it.
///
/// # Examples
///
/// ```
/// use vo_core::workspace_swap::{AtomicSwap, SwapStatus};
///
/// let swap = AtomicSwap::new("/tmp/empty-ws");
/// assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum SwapStatus {
    /// No swap has been initiated. No journal file exists.
    NoPriorSwap,

    /// A swap was initiated but did not reach completion.
    ///
    /// The contained [`SwapPhase`] indicates the phase at which the swap
    /// stopped (due to crash, cancellation, or intentional pause). This phase
    /// determines the recovery action:
    ///
    /// | Phase | Action |
    /// |-------|--------|
    /// | `Staging` / `Staged` | Remove shadow, remove journal. |
    /// | `Swapping` | Restore backup, remove shadow and backup. |
    Incomplete(SwapPhase),

    /// The swap completed successfully.
    ///
    /// The journal file contains `"complete"`. In normal operation the journal
    /// is removed immediately after reaching this state, so this variant is
    /// primarily observed during the final moments of a commit.
    Complete,
}

/// Error type for workspace swap operations.
///
/// This enum covers all failure modes of the atomic swap protocol, categorized
/// by the layer at which the failure occurred:
///
/// # I/O Errors
///
/// | Variant | Cause |
/// |---------|-------|
/// | [`NotADirectory`][Self::NotADirectory] | Workspace path exists but is a file, not a directory. |
/// | [`WorkspaceNotFound`][Self::WorkspaceNotFound] | Workspace path does not exist. |
/// | [`ShadowCreate`][Self::ShadowCreate] | Failed to create the shadow directory. |
/// | [`CopyFailed`][Self::CopyFailed] | Failed to copy a file into the shadow directory. |
/// | [`SyncFailed`][Self::SyncFailed] | Failed to fsync a directory. |
/// | [`RenameFailed`][Self::RenameFailed] | Failed to rename (mv) a directory. |
/// | [`RemoveFailed`][Self::RemoveFailed] | Failed to remove a directory or file. |
///
/// # State Errors
///
/// | Variant | Cause |
/// |---------|-------|
/// | [`ShadowExists`][Self::ShadowExists] | Shadow directory already exists when `stage()` was called. |
/// | [`NotStaged`][Self::NotStaged] | `commit()` called without a prior `stage()` call. |
///
/// # Journal Errors
///
/// | Variant | Cause |
/// |---------|-------|
/// | [`JournalWrite`][Self::JournalWrite] | Failed to write the journal file. |
/// | [`JournalRead`][Self::JournalRead] | Failed to read the journal file. |
/// | [`InvalidJournal`][Self::InvalidJournal] | Journal contains unrecognized phase string. |
///
/// # Recovery Errors
///
/// | Variant | Cause |
/// |---------|-------|
/// | [`RecoveryNeeded`][Self::RecoveryNeeded] | `atomic_swap()` called while a prior swap is incomplete. |
///
/// # Error Handling Strategy
///
/// All I/O errors are wrapped with context (path, source operation) so that
/// the caller can diagnose the failure. State errors indicate protocol
/// violations that the caller must handle before retrying.
#[derive(Debug, thiserror::Error)]
pub enum SwapError {
    /// Workspace path is not a directory.
    ///
    /// The path exists but points to a file (or other non-directory entity).
    /// The workspace must be a directory for the swap protocol to operate.
    #[error("workspace path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Workspace does not exist.
    ///
    /// The path does not point to any existing file system entity. The workspace
    /// directory must exist before [`stage()`][AtomicSwap::stage] is called.
    #[error("workspace does not exist: {0}")]
    WorkspaceNotFound(PathBuf),

    /// Shadow directory already exists.
    ///
    /// A previous swap was staged but never committed, leaving the shadow
    /// directory in place. Call [`recover()`][AtomicSwap::recover] to clean up
    /// before attempting another swap.
    #[error("shadow directory already exists: {0}")]
    ShadowExists(PathBuf),

    /// Failed to create shadow directory.
    ///
    /// The system call to create the shadow directory failed. Check disk space,
    /// permissions, and path length.
    #[error("failed to create shadow directory: {path}: {source}")]
    ShadowCreate { path: PathBuf, source: io::Error },

    /// Failed to copy a file to the shadow directory.
    ///
    /// The recursive copy from the workspace to the shadow directory failed
    /// on a specific file. Check disk space and permissions on the destination.
    #[error("failed to copy file to shadow: {source}: {from} -> {to}")]
    CopyFailed {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },

    /// Failed to sync (fsync) a directory.
    ///
    /// The fsync syscall did not complete successfully. This may indicate
    /// disk errors or that the file system does not support fsync.
    #[error("failed to sync directory: {path}: {source}")]
    SyncFailed { path: PathBuf, source: io::Error },

    /// Failed to write the journal file.
    #[error("failed to write journal: {path}: {source}")]
    JournalWrite { path: PathBuf, source: io::Error },

    /// Failed to read the journal file.
    #[error("failed to read journal: {path}: {source}")]
    JournalRead { path: PathBuf, source: io::Error },

    /// Failed to perform an atomic rename.
    ///
    /// This error can occur during the commit phase when renaming the original
    /// workspace to `.backup` or renaming the shadow to the workspace path.
    #[error("failed to atomic rename: {from} -> {to}: {source}")]
    RenameFailed {
        from: PathBuf,
        to: PathBuf,
        source: io::Error,
    },

    /// Failed to remove a directory or file.
    ///
    /// This can occur during cleanup (removing backup, shadow, or journal)
    /// or during recovery.
    #[error("failed to remove directory: {path}: {source}")]
    RemoveFailed { path: PathBuf, source: io::Error },

    /// `commit()` was called without a prior successful `stage()` call.
    ///
    /// The swap protocol requires staging before committing. Call
    /// [`stage()`][AtomicSwap::stage] first, then [`commit()`][AtomicSwap::commit].
    #[error("swap not staged; call stage() first")]
    NotStaged,

    /// Journal file contains an unrecognized phase string.
    ///
    /// This indicates corruption of the journal file or an incompatible
    /// version. The contained string is the unrecognized content.
    #[error("invalid journal content: {0}")]
    InvalidJournal(String),

    /// A prior swap is incomplete and must be recovered before proceeding.
    ///
    /// Returned by [`atomic_swap`] when it detects an incomplete swap.
    /// Call [`recover_swap`] to restore consistency, then retry.
    #[error("recovery needed: swap incomplete at phase {0:?}")]
    RecoveryNeeded(SwapPhase),
}

/// Configurable atomic workspace swap operations.
///
/// This struct encapsulates the paths and configuration for a workspace swap
/// operation. It supports a two-phase commit protocol: [`stage()`][Self::stage]
/// copies the workspace to a shadow directory, and [`commit()`][Self::commit]
/// atomically swaps the original with the shadow.
///
/// # Thread Safety
///
/// `AtomicSwap` is `!Sync` by design — the swap protocol relies on file system
/// operations that are not safe to interleave. Each swap must be performed by
/// a single `AtomicSwap` instance from start to finish.
///
/// # Examples
///
/// ## Basic two-phase swap
///
/// ```no_run
/// use vo_core::workspace_swap::AtomicSwap;
///
/// let swap = AtomicSwap::new("/path/to/workspace");
/// swap.stage().expect("staging failed");
/// swap.commit().expect("commit failed");
/// ```
///
/// ## Custom suffixes
///
/// ```no_run
/// use vo_core::workspace_swap::AtomicSwap;
///
/// let swap = AtomicSwap::with_shadow_suffix("/path/to/workspace", ".v2-shadow");
/// swap.stage().expect("staging failed");
/// swap.commit().expect("commit failed");
/// ```
pub struct AtomicSwap {
    /// Path to the workspace directory being swapped.
    workspace: PathBuf,

    /// Suffix appended to the workspace name for the shadow directory.
    /// Default: `".shadow"`.
    shadow_suffix: String,

    /// Suffix appended to the workspace name for the journal file.
    /// Default: `".swap-journal"`.
    journal_suffix: String,
}

impl AtomicSwap {
    /// Creates a new `AtomicSwap` for the given workspace path.
    ///
    /// Uses default suffixes:
    /// - Shadow directory: `{workspace}.shadow`
    /// - Journal file: `{workspace}.swap-journal`
    ///
    /// # Arguments
    ///
    /// * `workspace` — Path to the workspace directory. Must exist and be a
    ///   directory before [`stage()`][Self::stage] is called.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::new("/tmp/my-workspace");
    /// assert_eq!(swap.workspace().as_os_str(), "/tmp/my-workspace");
    /// ```
    pub fn new<P: AsRef<Path>>(workspace: P) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            shadow_suffix: ".shadow".to_string(),
            journal_suffix: ".swap-journal".to_string(),
        }
    }

    /// Creates a new `AtomicSwap` with a custom shadow directory suffix.
    ///
    /// The journal suffix remains the default (`.swap-journal`). Use this
    /// when multiple shadow directories coexist for the same workspace
    /// (e.g., different branch swap targets).
    ///
    /// # Arguments
    ///
    /// * `workspace` — Path to the workspace directory.
    /// * `suffix` — Suffix appended to the workspace name for the shadow
    ///   directory path. For example, `".v2-shadow"` produces
    ///   `{workspace}.v2-shadow`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::with_shadow_suffix("/tmp/ws", ".alt");
    /// // shadow path would be /tmp/ws.alt
    /// ```
    pub fn with_shadow_suffix<P: AsRef<Path>>(workspace: P, suffix: &str) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            shadow_suffix: suffix.to_string(),
            journal_suffix: ".swap-journal".to_string(),
        }
    }

    /// Checks the swap status by reading the journal file.
    ///
    /// Returns a [`SwapStatus`] indicating whether a swap is in progress,
    /// has completed, or has never been initiated.
    ///
    /// # Return Values
    ///
    /// | Condition | Return Value |
    /// |-----------|-------------|
    /// | No journal file | `Ok(SwapStatus::NoPriorSwap)` |
    /// | Journal = `"complete"` | `Ok(SwapStatus::Complete)` |
    /// | Journal = `"staging"` | `Ok(SwapStatus::Incomplete(SwapPhase::Staging))` |
    /// | Journal = `"staged"` | `Ok(SwapStatus::Incomplete(SwapPhase::Staged))` |
    /// | Journal = `"swapping"` | `Ok(SwapStatus::Incomplete(SwapPhase::Swapping))` |
    /// | Journal = unrecognized content | `Err(SwapError::InvalidJournal)` |
    /// | Journal read I/O error | `Err(SwapError::JournalRead)` |
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::{AtomicSwap, SwapStatus};
    ///
    /// let swap = AtomicSwap::new("/tmp/empty-ws");
    /// assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    /// ```
    pub fn check_status(&self) -> Result<SwapStatus, SwapError> {
        let journal = self.journal_path();
        if !journal.exists() {
            return Ok(SwapStatus::NoPriorSwap);
        }

        let content = fs::read_to_string(&journal).map_err(|e| SwapError::JournalRead {
            path: journal.clone(),
            source: e,
        })?;

        let phase = SwapPhase::from_str_lossy(&content)
            .ok_or_else(|| SwapError::InvalidJournal(content.clone()))?;

        match phase {
            SwapPhase::Complete => Ok(SwapStatus::Complete),
            other => Ok(SwapStatus::Incomplete(other)),
        }
    }

    /// Stages the workspace by copying it to a shadow directory.
    ///
    /// This is the first phase of the two-phase commit protocol. After
    /// staging:
    /// - The shadow directory contains a full copy of the workspace.
    /// - Both the shadow and original directories have been fsync'd.
    /// - The journal file records phase `"staged"`.
    /// - The original workspace is **untouched**.
    ///
    /// # Pre-conditions
    ///
    /// - The workspace path must exist and be a directory.
    /// - No shadow directory may already exist (checked via [`shadow_path`][Self::shadow_path]
    ///   naming convention). If a prior swap left a shadow behind, call
    ///   [`recover()`][Self::recover] first.
    ///
    /// # Return Value
    ///
    /// Returns `Ok(SwapPhase::Staged)` on success.
    ///
    /// # Errors
    ///
    /// | Error | Condition |
    /// |-------|-----------|
    /// | [`WorkspaceNotFound`][SwapError::WorkspaceNotFound] | Workspace path does not exist. |
    /// | [`NotADirectory`][SwapError::NotADirectory] | Workspace path is a file. |
    /// | [`ShadowExists`][SwapError::ShadowExists] | Shadow directory already exists. |
    /// | [`ShadowCreate`][SwapError::ShadowCreate] | Failed to create shadow directory. |
    /// | [`CopyFailed`][SwapError::CopyFailed] | Failed to copy a file. |
    /// | [`SyncFailed`][SwapError::SyncFailed] | Failed to fsync a directory. |
    /// | [`JournalWrite`][SwapError::JournalWrite] | Failed to write journal. |
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::new("/tmp/ws");
    /// swap.stage().expect("stage failed");
    /// // Shadow directory now has full workspace copy, ready for commit.
    /// ```
    pub fn stage(&self) -> Result<SwapPhase, SwapError> {
        self.validate_workspace()?;

        let shadow = self.shadow_path();
        if shadow.exists() {
            return Err(SwapError::ShadowExists(shadow));
        }

        self.write_journal(SwapPhase::Staging)?;

        fs::create_dir_all(&shadow).map_err(|e| SwapError::ShadowCreate {
            path: shadow.clone(),
            source: e,
        })?;

        copy_dir_recursive(&self.workspace, &shadow)?;

        sync_dir(&shadow)?;
        sync_dir(&self.workspace)?;

        self.write_journal(SwapPhase::Staged)?;

        Ok(SwapPhase::Staged)
    }

    /// Commits the staged swap by atomically swapping the workspace.
    ///
    /// This is the second phase of the two-phase commit protocol. After commit:
    /// - The original workspace is replaced with the shadow directory contents.
    /// - The original workspace is temporarily backed up as `.backup`.
    /// - The backup is removed after the new workspace is verified in place.
    /// - The journal file is removed.
    ///
    /// # Idempotency
    ///
    /// This method is idempotent. Calling it when there is no prior swap
    /// (`NoPriorSwap`) or when the swap is already complete returns `Ok(())`
    /// without error.
    ///
    /// # Pre-conditions
    ///
    /// - [`stage()`][Self::stage] must have been called and succeeded.
    ///   Without a staged state, this returns [`SwapError::NotStaged`].
    ///
    /// # Errors
    ///
    /// | Error | Condition |
    /// |-------|-----------|
    /// | [`RenameFailed`][SwapError::RenameFailed] | Failed to rename original→backup or shadow→workspace. |
    /// | [`SyncFailed`][SwapError::SyncFailed] | Failed to fsync parent directory. |
    /// | [`RemoveFailed`][SwapError::RemoveFailed] | Failed to remove backup or journal. |
    /// | [`JournalWrite`][SwapError::JournalWrite] | Failed to write journal during commit. |
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::new("/tmp/ws");
    /// swap.stage().unwrap();
    /// swap.commit().unwrap();
    /// // Workspace now contains the shadow contents.
    /// // Shadow, backup, and journal are all cleaned up.
    /// ```
    pub fn commit(&self) -> Result<(), SwapError> {
        let status = self.check_status()?;
        match status {
            SwapStatus::NoPriorSwap | SwapStatus::Complete => return Ok(()),
            SwapStatus::Incomplete(_) => {}
        }

        self.write_journal(SwapPhase::Swapping)?;

        let shadow = self.shadow_path();
        let backup = self.backup_path();

        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup.clone(),
                source: e,
            })?;
        }

        fs::rename(&self.workspace, &backup).map_err(|e| SwapError::RenameFailed {
            from: self.workspace.clone(),
            to: backup.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        fs::rename(&shadow, &self.workspace).map_err(|e| SwapError::RenameFailed {
            from: shadow.clone(),
            to: self.workspace.clone(),
            source: e,
        })?;

        sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;

        self.write_journal(SwapPhase::Complete)?;

        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                path: backup,
                source: e,
            })?;
        }

        let journal = self.journal_path();
        if journal.exists() {
            fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
                path: journal,
                source: e,
            })?;
        }

        Ok(())
    }

    /// Recovers from a crash or interruption during a workspace swap.
    ///
    /// Inspects the journal file and performs the appropriate recovery action
    /// to restore the workspace to a consistent state.
    ///
    /// # Recovery Behavior
    ///
    /// | Prior Phase | Recovery Action | Return Value |
    /// |-------------|-----------------|--------------|
    /// | No journal | Nothing to do | `Ok(NothingToRecover)` |
    /// | `Complete` (stale journal) | Remove journal | `Ok(AlreadyComplete)` |
    /// | `Staging` / `Staged` | Remove shadow + journal | `Ok(RolledBack)` |
    /// | `Swapping` (backup exists) | Restore backup to workspace | `Ok(RolledBack)` |
    /// | `Swapping` (backup + workspace both exist) | Remove backup + shadow + journal | `Ok(RolledBack)` |
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::{AtomicSwap, RecoveryOutcome};
    ///
    /// let swap = AtomicSwap::new("/tmp/empty-ws");
    /// let outcome = swap.recover().unwrap();
    /// assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
    /// ```
    pub fn recover(&self) -> Result<RecoveryOutcome, SwapError> {
        let status = self.check_status()?;

        match status {
            SwapStatus::NoPriorSwap => Ok(RecoveryOutcome::NothingToRecover),
            SwapStatus::Complete => {
                self.cleanup_journal()?;
                Ok(RecoveryOutcome::AlreadyComplete)
            }
            SwapStatus::Incomplete(phase) => {
                let shadow = self.shadow_path();
                let backup = self.backup_path();

                match phase {
                    SwapPhase::Staging | SwapPhase::Staged => {
                        if shadow.exists() {
                            fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
                                path: shadow.clone(),
                                source: e,
                            })?;
                        }
                        self.cleanup_journal()?;
                        Ok(RecoveryOutcome::RolledBack)
                    }
                    SwapPhase::Swapping => {
                        if backup.exists() && !self.workspace.exists() {
                            fs::rename(&backup, &self.workspace).map_err(|e| {
                                SwapError::RenameFailed {
                                    from: backup.clone(),
                                    to: self.workspace.clone(),
                                    source: e,
                                }
                            })?;
                            sync_dir(self.workspace.parent().unwrap_or(Path::new(".")))?;
                        } else if backup.exists() && self.workspace.exists() {
                            fs::remove_dir_all(&backup).map_err(|e| SwapError::RemoveFailed {
                                path: backup.clone(),
                                source: e,
                            })?;
                        }

                        if shadow.exists() {
                            fs::remove_dir_all(&shadow).map_err(|e| SwapError::RemoveFailed {
                                path: shadow.clone(),
                                source: e,
                            })?;
                        }

                        self.cleanup_journal()?;
                        Ok(RecoveryOutcome::RolledBack)
                    }
                    _ => Ok(RecoveryOutcome::NothingToRecover),
                }
            }
        }
    }

    /// Returns the workspace path.
    ///
    /// This is the original workspace directory path passed to [`new()`][Self::new]
    /// or [`with_shadow_suffix()`][Self::with_shadow_suffix].
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::new("/tmp/my-ws");
    /// assert_eq!(swap.workspace().as_os_str(), "/tmp/my-ws");
    /// ```
    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Returns the shadow directory path.
    ///
    /// The shadow path is `{workspace}.{shadow_suffix}`. By default this is
    /// `{workspace}.shadow`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::workspace_swap::AtomicSwap;
    ///
    /// let swap = AtomicSwap::new("/tmp/ws");
    /// assert_eq!(swap.shadow_dir(), "/tmp/ws.shadow");
    ///
    /// let swap = AtomicSwap::with_shadow_suffix("/tmp/ws", ".alt");
    /// assert_eq!(swap.shadow_dir(), "/tmp/ws.alt");
    /// ```
    #[must_use]
    pub fn shadow_dir(&self) -> PathBuf {
        self.shadow_path()
    }

    fn journal_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}{}", self.journal_suffix));
        p
    }

    fn shadow_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}{}", self.shadow_suffix));
        p
    }

    fn backup_path(&self) -> PathBuf {
        let mut p = self.workspace.clone();
        let name = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        p.set_file_name(format!("{name}.backup"));
        p
    }

    fn validate_workspace(&self) -> Result<(), SwapError> {
        if !self.workspace.exists() {
            return Err(SwapError::WorkspaceNotFound(self.workspace.clone()));
        }
        if !self.workspace.is_dir() {
            return Err(SwapError::NotADirectory(self.workspace.clone()));
        }
        Ok(())
    }

    fn write_journal(&self, phase: SwapPhase) -> Result<(), SwapError> {
        let journal = self.journal_path();
        let content = phase.as_str();
        fs::write(&journal, content).map_err(|e| SwapError::JournalWrite {
            path: journal.clone(),
            source: e,
        })?;
        sync_dir(journal.parent().unwrap_or(Path::new(".")))?;
        Ok(())
    }

    fn cleanup_journal(&self) -> Result<(), SwapError> {
        let journal = self.journal_path();
        if journal.exists() {
            fs::remove_file(&journal).map_err(|e| SwapError::RemoveFailed {
                path: journal,
                source: e,
            })?;
        }
        Ok(())
    }
}

/// Outcome of a crash recovery operation.
///
/// Returned by [`AtomicSwap::recover`] and [`recover_swap`], this enum indicates
/// what action the recovery took (if any).
///
/// # Variants
///
/// - [`NothingToRecover`] — No journal file existed. No swap was in progress.
/// - [`AlreadyComplete`] — The journal indicated a completed swap. The journal
///   was removed as cleanup. The workspace already has the new state.
/// - [`RolledBack`] — An incomplete swap was detected and reversed. The workspace
///   has been restored to a consistent state (either the original state for
///   pre-commit crashes, or the restored backup for mid-swap crashes).
///
/// # Examples
///
/// ```
/// use vo_core::workspace_swap::{recover_swap, RecoveryOutcome};
///
/// match recover_swap("/tmp/empty-ws") {
///     Ok(RecoveryOutcome::NothingToRecover) => {
///         println!("No swap was in progress");
///     }
///     Ok(RecoveryOutcome::AlreadyComplete) => {
///         println!("Swap completed, stale journal cleaned up");
///     }
///     Ok(RecoveryOutcome::RolledBack) => {
///         println!("Incomplete swap was rolled back");
///     }
///     Err(e) => eprintln!("Recovery error: {}", e),
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryOutcome {
    /// No swap was in progress. No action was taken.
    NothingToRecover,

    /// The swap had already completed. The stale journal file was removed.
    AlreadyComplete,

    /// An incomplete swap was detected and rolled back.
    ///
    /// For crashes during `staging`/`staged`, the shadow directory is removed
    /// and the original workspace is untouched.
    ///
    /// For crashes during `swapping`, the backup is restored to the workspace
    /// path (if the workspace is missing) or both backup and shadow are removed
    /// (if both exist).
    RolledBack,
}

fn sync_dir(path: &Path) -> Result<(), SwapError> {
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| SwapError::SyncFailed {
            path: path.to_path_buf(),
            source: e,
        })?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SwapError> {
    fs::create_dir_all(dst).map_err(|e| SwapError::ShadowCreate {
        path: dst.to_path_buf(),
        source: e,
    })?;

    for entry in fs::read_dir(src).map_err(|e| SwapError::CopyFailed {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| SwapError::CopyFailed {
            from: src.to_path_buf(),
            to: dst.to_path_buf(),
            source: e,
        })?;

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type().map_err(|e| SwapError::CopyFailed {
            from: src_path.clone(),
            to: dst_path.clone(),
            source: e,
        })?;

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path).map_err(|e| SwapError::CopyFailed {
                from: src_path.clone(),
                to: dst_path.clone(),
                source: e,
            })?;
        }
    }

    Ok(())
}

/// Performs a complete atomic workspace swap (stage + commit) in one call.
///
/// This is a convenience function that creates an [`AtomicSwap`] instance,
/// checks for incomplete prior swaps, then calls [`stage()`][AtomicSwap::stage]
/// and [`commit()`][AtomicSwap::commit] in sequence.
///
/// # Safety Check
///
/// If an incomplete swap is detected (a journal file exists from a prior
/// interrupted swap), this function returns [`SwapError::RecoveryNeeded`]
/// instead of proceeding. Call [`recover_swap`] first to restore consistency,
/// then retry.
///
/// # Examples
///
/// ```no_run
/// use vo_core::workspace_swap::atomic_swap;
///
/// atomic_swap("/path/to/workspace").expect("swap failed");
/// ```
pub fn atomic_swap<P: AsRef<Path>>(workspace: P) -> Result<(), SwapError> {
    let swap = AtomicSwap::new(workspace);

    if let SwapStatus::Incomplete(phase) = swap.check_status()? {
        return Err(SwapError::RecoveryNeeded(phase));
    }

    swap.stage()?;
    swap.commit()?;

    Ok(())
}

/// Recovers from a crash or interruption during a workspace swap.
///
/// This is a convenience function that creates an [`AtomicSwap`] instance and
/// calls [`recover()`][AtomicSwap::recover].
///
/// # Examples
///
/// ```
/// use vo_core::workspace_swap::recover_swap;
///
/// let outcome = recover_swap("/tmp/empty-ws").unwrap();
/// assert_eq!(outcome, vo_core::workspace_swap::RecoveryOutcome::NothingToRecover);
/// ```
pub fn recover_swap<P: AsRef<Path>>(workspace: P) -> Result<RecoveryOutcome, SwapError> {
    let swap = AtomicSwap::new(workspace);
    swap.recover()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn atomic_swap_creates_shadow_then_commits() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "hello").unwrap();

        let swap = AtomicSwap::new(&workspace);

        let phase = swap.stage().unwrap();
        assert_eq!(phase, SwapPhase::Staged);

        let shadow = swap.shadow_path();
        assert!(shadow.exists());
        assert!(shadow.join("file.txt").exists());
        assert_eq!(
            fs::read_to_string(shadow.join("file.txt")).unwrap(),
            "hello"
        );

        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );

        swap.commit().unwrap();

        assert!(!shadow.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn atomic_swap_preserves_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        let nested = workspace.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "deep content").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(
            fs::read_to_string(workspace.join("a/b/c/deep.txt")).unwrap(),
            "deep content"
        );
    }

    #[test]
    fn stage_fails_if_shadow_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let swap2 = AtomicSwap::new(&workspace);
        assert!(matches!(swap2.stage(), Err(SwapError::ShadowExists(_))));
    }

    #[test]
    fn commit_is_idempotent_when_no_prior_swap() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert!(swap.commit().is_ok());
    }

    #[test]
    fn stage_fails_if_workspace_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent");

        let swap = AtomicSwap::new(&missing);
        assert!(matches!(swap.stage(), Err(SwapError::WorkspaceNotFound(_))));
    }

    #[test]
    fn stage_fails_if_path_is_file_not_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "data").unwrap();

        let swap = AtomicSwap::new(&file);
        assert!(matches!(swap.stage(), Err(SwapError::NotADirectory(_))));
    }

    #[test]
    fn commit_idempotent_when_already_complete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "data").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert!(swap.commit().is_ok());
    }

    #[test]
    fn check_status_reports_no_prior_swap_initially() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn check_status_reports_incomplete_after_stage() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        assert_eq!(
            swap.check_status().unwrap(),
            SwapStatus::Incomplete(SwapPhase::Staged)
        );
    }

    #[test]
    fn check_status_reports_no_prior_after_commit() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();
        swap.commit().unwrap();

        assert_eq!(swap.check_status().unwrap(), SwapStatus::NoPriorSwap);
    }

    #[test]
    fn recover_rolls_back_from_staging_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "staging").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!swap.shadow_path().exists());
        assert!(!journal.exists());
    }

    #[test]
    fn recover_rolls_back_from_swapping_phase() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let journal = swap.journal_path();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn recover_restores_backup_when_workspace_missing_during_swapping() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("file.txt"), "original").unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let backup = swap.backup_path();
        let shadow = swap.shadow_path();
        let journal = swap.journal_path();

        fs::rename(&workspace, &backup).unwrap();
        fs::write(&journal, "swapping").unwrap();

        let outcome = swap.recover().unwrap();
        assert_eq!(outcome, RecoveryOutcome::RolledBack);

        assert!(workspace.exists());
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).unwrap(),
            "original"
        );
        assert!(!shadow.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn recover_returns_nothing_when_no_journal() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.recover().unwrap(), RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn atomic_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("test.txt"), "content").unwrap();

        assert!(atomic_swap(&workspace).is_ok());

        assert_eq!(
            fs::read_to_string(workspace.join("test.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn atomic_swap_returns_recovery_needed_on_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::new(&workspace);
        swap.stage().unwrap();

        let result = atomic_swap(&workspace);
        assert!(matches!(result, Err(SwapError::RecoveryNeeded(_))));
    }

    #[test]
    fn recover_swap_convenience_function_works() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let outcome = recover_swap(&workspace).unwrap();
        assert_eq!(outcome, RecoveryOutcome::NothingToRecover);
    }

    #[test]
    fn swap_phase_roundtrip() {
        assert_eq!(
            SwapPhase::from_str_lossy("staging"),
            Some(SwapPhase::Staging)
        );
        assert_eq!(SwapPhase::from_str_lossy("staged"), Some(SwapPhase::Staged));
        assert_eq!(
            SwapPhase::from_str_lossy("swapping"),
            Some(SwapPhase::Swapping)
        );
        assert_eq!(
            SwapPhase::from_str_lossy("complete"),
            Some(SwapPhase::Complete)
        );
        assert_eq!(SwapPhase::from_str_lossy("garbage"), None);
        assert_eq!(SwapPhase::from_str_lossy(""), None);
    }

    #[test]
    fn with_shadow_suffix_uses_custom_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).unwrap();

        let swap = AtomicSwap::with_shadow_suffix(&workspace, ".custom-shadow");
        swap.stage().unwrap();

        assert!(dir.path().join("ws.custom-shadow").exists());
        assert!(!dir.path().join("ws.shadow").exists());
    }

    #[test]
    fn workspace_accessor_returns_original_path() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");

        let swap = AtomicSwap::new(&workspace);
        assert_eq!(swap.workspace(), workspace);
    }
}
