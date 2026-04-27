//! Snapshot schema version compatibility checking.
//!
//! # Overview
//!
//! This module implements the snapshot version compatibility protocol defined in
//! [ADR-016] and [ADR-035]. It determines whether a persisted snapshot's schema
//! version can be used to rehydrate an engine instance, or whether the snapshot
//! must be discarded and the engine rebuilt from the event log.
//!
//! [ADR-016]: <https://www.adr.org/adr-016>
//! [ADR-035]: <https://www.adr.org/adr-035>
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                  Engine Rehydration                   │
//! │                                                      │
//! │  ┌──────────┐    ┌──────────────────────┐           │
//! │  │ Load     │───>│ check_snapshot_compat│           │
//! │  │ snapshot │    │ (snapshot_ver, eng_ver)│           │
//! │  └──────────┘    └──────────┬───────────┘           │
//! │                             │                        │
//! │                     ┌───────┴───────┐               │
//! │                     │               │               │
//! │              ┌──────▼──────┐ ┌──────▼───────┐       │
//! │              │Compatible   │ │Non-compatible │       │
//! │              │Use snapshot │ │              │       │
//! │              └─────────────┘ │  NeedsUpcast │       │
//! │                              │  Discard     │       │
//! │                              │  Rebuild     │       │
//! │                              └──────────────┘       │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Design Principles
//!
//! 1. **Snapshots are a cache, not a schema authority** (ADR-035 §3). When
//!    incompatibility is detected, the engine discards the snapshot and rebuilds
//!    from the event log. The event log is the canonical source of truth.
//!
//! 2. **Version zero is invalid**. Both snapshot and engine versions must be
//!    positive. A version of `0` signals a corrupted or uninitialized state.
//!
//! 3. **Unidirectional compatibility**. Only older snapshots can be upcast to
//!    newer engine versions. A newer snapshot cannot be downgraded — this is
//!    the `Incompatible` case, which requires rebuilding from events.
//!
//! # Schema Versioning Model
//!
//! Schema versions are `u16` monotonically increasing integers. The engine's
//! current maximum supported version is supplied by the caller to
//! [`check_snapshot_compat`]. There is no version history stored — the engine
//! only knows its own current version and accepts snapshots from that version
//! or any older version.
//!
//! # State Transitions
//!
//! ```text
//!     snapshot_version < engine_version     snapshot_version > engine_version
//!              │                                      │
//!              ▼                                      ▼
//!     SnapshotCompat::NeedsUpcast        SnapshotCompat::Incompatible
//!         from: snapshot_version               snapshot: snapshot_version
//!         to: engine_version                  engine: engine_version
//!              │
//!              │ upcast transforms snapshot data
//!              │ to current engine schema
//!              ▼
//!     SnapshotCompat::Compatible
//! ```
//!
//! # Invariants
//!
//! - `Compatible` <=> `snapshot_version == engine_version` and neither is zero.
//! - `NeedsUpcast` <=> `0 < snapshot_version < engine_version`.
//! - `Incompatible` <=> `snapshot_version == 0 || engine_version == 0 || snapshot_version > engine_version`.
//! - The `is_compatible()` method returns `true` if and only if the state is
//!   `Compatible`. This is the only gate for snapshot acceptance.
//!
//! # Example
//!
//! ```
//! use vo_core::snapshot_compat::check_snapshot_compat;
//!
//! // Engine at version 5, snapshot at version 3 — upcast needed
//! match check_snapshot_compat(3, 5) {
//!     check_snapshot_compat::SnapshotCompat::NeedsUpcast { from, to } => {
//!         // Perform upcast from schema v3 → v5
//!         assert_eq!(from, 3);
//!         assert_eq!(to, 5);
//!     }
//!     _ => panic!("expected upcast"),
//! }
//!
//! // Snapshot at version 7, engine at version 5 — discard and rebuild
//! match check_snapshot_compat(7, 5) {
//!     check_snapshot_compat::SnapshotCompat::Incompatible { snapshot, engine } => {
//!         // Discard snapshot, rebuild from event log
//!         assert_eq!(snapshot, 7);
//!         assert_eq!(engine, 5);
//!     }
//!     _ => panic!("expected incompatible"),
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Result of comparing a snapshot's schema version against the engine's
/// current maximum supported schema version.
///
/// This enum represents one of three compatibility outcomes after calling
/// [`check_snapshot_compat`]. The caller must handle each variant appropriately:
///
/// - [`Compatible`][Self::Compatible] — Use the snapshot as-is.
/// - [`NeedsUpcast`][Self::NeedsUpcast] — Transform the snapshot data from its
///   source schema version to the engine's current version. See [module-level
///   docs](crate::snapshot_compat) for the versioning model.
/// - [`Incompatible`][Self::Incompatible] — The snapshot cannot be used. Discard
///   it and rehydrate from the event log.
///
/// # Version Zero Semantics
///
/// Both `0` snapshot and `0` engine versions produce `Incompatible`. A version of
/// zero represents an invalid state:
/// - Snapshot version 0: the snapshot was never written or is corrupted.
/// - Engine version 0: the engine has not been initialized.
///
/// # Examples
///
/// ```
/// use vo_core::snapshot_compat::check_snapshot_compat;
///
/// let result = check_snapshot_compat(3, 3);
/// assert!(result.is_compatible());
///
/// let result = check_snapshot_compat(0, 1);
/// assert!(!result.is_compatible());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapshotCompat {
    /// Snapshot version matches engine version exactly. Safe to use as-is.
    ///
    /// This is the only state where the snapshot can be directly applied to
    /// rehydrate the engine without any transformation.
    Compatible,

    /// Snapshot version is older than engine version. Upcasting is required.
    ///
    /// The snapshot was written by an older engine version and must be
    /// transformed to match the current engine's schema before it can be used
    /// for rehydration. The `from` field contains the snapshot's original
    /// version and `to` contains the engine's current version.
    ///
    /// # Upcast Process
    ///
    /// Upcasting is a forward migration: it transforms the snapshot data from
    /// an older schema to a newer one. The engine applies schema transformations
    /// sequentially from `from` to `to`, ensuring the rehydrated state conforms
    /// to the current schema.
    ///
    /// # Invariant
    ///
    /// `0 < from < to`. Neither field is zero.
    NeedsUpcast { from: u16, to: u16 },

    /// Snapshot version is newer than engine or is zero (invalid).
    ///
    /// This snapshot cannot be used for rehydration. The engine must discard
    /// it and rebuild state from the event log (the canonical source of truth).
    ///
    /// Two sub-cases:
    /// - **Forward incompatibility** (`snapshot > engine`): The snapshot was
    ///   produced by a newer engine version. Downgrading is not supported —
    ///   the schema may have added fields or changed semantics.
    /// - **Zero version** (`snapshot == 0 || engine == 0`): Invalid state.
    ///   Either the snapshot file is corrupted/uninitialized, or the engine
    ///   has not been properly initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::snapshot_compat::check_snapshot_compat;
    ///
    /// // Forward incompatibility: snapshot from a newer engine
    /// let result = check_snapshot_compat(7, 3);
    /// if let SnapshotCompat::Incompatible { snapshot, engine } = result {
    ///     assert_eq!(snapshot, 7);
    ///     assert_eq!(engine, 3);
    /// }
    /// ```
    Incompatible { snapshot: u16, engine: u16 },
}

impl SnapshotCompat {
    /// Returns `true` if this result is `Compatible`.
    ///
    /// This is the primary gate function used during snapshot rehydration.
    /// Only `Compatible` results permit the snapshot to be loaded. All other
    /// cases (`NeedsUpcast` or `Incompatible`) must be handled by the caller.
    ///
    /// # Invariant
    ///
    /// `is_compatible()` returns `true` if and only if this variant is
    /// `SnapshotCompat::Compatible`. This is equivalent to checking
    /// `matches!(self, Self::Compatible)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use vo_core::snapshot_compat::{check_snapshot_compat, SnapshotCompat};
    ///
    /// assert!(check_snapshot_compat(3, 3).is_compatible());
    /// assert!(!check_snapshot_compat(1, 3).is_compatible());
    /// assert!(!check_snapshot_compat(5, 3).is_compatible());
    /// assert!(!check_snapshot_compat(0, 1).is_compatible());
    /// ```
    #[must_use]
    pub fn is_compatible(self) -> bool {
        matches!(self, Self::Compatible)
    }
}

/// Returns `true` when the snapshot version exactly matches the engine version.
///
/// This is a simple equality check — a fast-path predicate for callers that
/// only need to know whether versions match, without the detailed breakdown
/// provided by [`check_snapshot_compat`].
///
/// # Limitations
///
/// This function only returns `true` or `false`. It does not distinguish
/// between the cases of `NeedsUpcast` and `Incompatible`. For full diagnostics,
/// use [`check_snapshot_compat`] instead.
///
/// # See Also
///
/// - [`check_snapshot_compat`] — Full compatibility assessment with variant breakdown
/// - [`SnapshotCompat::is_compatible`] — Same logic as an instance method
///
/// # Examples
///
/// ```
/// use vo_core::snapshot_compat::is_snapshot_compatible;
///
/// assert!(is_snapshot_compatible(3, 3));
/// assert!(!is_snapshot_compatible(1, 3));
/// assert!(!is_snapshot_compatible(5, 3));
/// ```
#[must_use]
pub fn is_snapshot_compatible(snapshot_version: u16, engine_version: u16) -> bool {
    snapshot_version == engine_version
}

/// Full compatibility assessment between a snapshot's schema version and the
/// engine's current maximum supported schema version.
///
/// This is the primary entry point for snapshot compatibility checking. It
/// returns a [`SnapshotCompat`] enum that precisely identifies the relationship
/// between the two versions and the required action.
///
/// # Algorithm
///
/// ```text
/// 1. If snapshot_version == 0 OR engine_version == 0
///    → Incompatible (invalid state)
///
/// 2. If snapshot_version == engine_version
///    → Compatible (use snapshot as-is)
///
/// 3. If snapshot_version < engine_version
///    → NeedsUpcast { from: snapshot_version, to: engine_version }
///
/// 4. If snapshot_version > engine_version
///    → Incompatible { snapshot: snapshot_version, engine: engine_version }
/// ```
///
/// # Zero Version Handling
///
/// A version of zero is treated as incompatible regardless of the other value.
/// This catches both uninitialized snapshots and uninitialized engines in a
/// single check.
///
/// # Pre-conditions
///
/// - `engine_version` must be the current maximum schema version supported by
///   the running engine instance.
/// - `snapshot_version` must be the schema version embedded in the snapshot
///   file being loaded.
///
/// # Examples
///
/// ```
/// use vo_core::snapshot_compat::{check_snapshot_compat, SnapshotCompat};
///
/// // Matching versions — compatible
/// assert!(check_snapshot_compat(3, 3).is_compatible());
///
/// // Snapshot from v1, engine at v3 — needs upcast
/// match check_snapshot_compat(1, 3) {
///     SnapshotCompat::NeedsUpcast { from, to } => {
///         assert_eq!(from, 1);
///         assert_eq!(to, 3);
///     }
///     _ => panic!("expected NeedsUpcast"),
/// }
///
/// // Snapshot from v5, engine at v3 — incompatible
/// match check_snapshot_compat(5, 3) {
///     SnapshotCompat::Incompatible { snapshot, engine } => {
///         assert_eq!(snapshot, 5);
///         assert_eq!(engine, 3);
///     }
///     _ => panic!("expected Incompatible"),
/// }
///
/// // Zero version — always incompatible
/// assert!(!check_snapshot_compat(0, 3).is_compatible());
/// assert!(!check_snapshot_compat(3, 0).is_compatible());
/// ```
#[must_use]
pub fn check_snapshot_compat(snapshot_version: u16, engine_version: u16) -> SnapshotCompat {
    if snapshot_version == 0 || engine_version == 0 {
        return SnapshotCompat::Incompatible {
            snapshot: snapshot_version,
            engine: engine_version,
        };
    }

    match snapshot_version.cmp(&engine_version) {
        std::cmp::Ordering::Equal => SnapshotCompat::Compatible,
        std::cmp::Ordering::Less => SnapshotCompat::NeedsUpcast {
            from: snapshot_version,
            to: engine_version,
        },
        std::cmp::Ordering::Greater => SnapshotCompat::Incompatible {
            snapshot: snapshot_version,
            engine: engine_version,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_versions_returns_compatible() {
        let result = check_snapshot_compat(3, 3);
        assert!(result.is_compatible());
    }

    #[test]
    fn snapshot_older_than_engine_returns_needs_upcast() {
        let result = check_snapshot_compat(1, 3);
        assert_eq!(result, SnapshotCompat::NeedsUpcast { from: 1, to: 3 });
    }

    #[test]
    fn snapshot_newer_than_engine_returns_incompatible() {
        let result = check_snapshot_compat(5, 3);
        assert_eq!(
            result,
            SnapshotCompat::Incompatible {
                snapshot: 5,
                engine: 3
            }
        );
    }

    #[test]
    fn version_zero_snapshot_is_incompatible() {
        let result = check_snapshot_compat(0, 1);
        assert_eq!(
            result,
            SnapshotCompat::Incompatible {
                snapshot: 0,
                engine: 1
            }
        );
    }

    #[test]
    fn version_zero_engine_is_incompatible() {
        let result = check_snapshot_compat(1, 0);
        assert_eq!(
            result,
            SnapshotCompat::Incompatible {
                snapshot: 1,
                engine: 0
            }
        );
    }

    #[test]
    fn serde_roundtrip_preserves_compatible() {
        let original = SnapshotCompat::Compatible;
        let json = serde_json::to_string(&original).unwrap();
        let roundtripped: SnapshotCompat = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped, original);
    }

    #[test]
    fn serde_roundtrip_preserves_needs_upcast() {
        let original = SnapshotCompat::NeedsUpcast { from: 2, to: 5 };
        let json = serde_json::to_string(&original).unwrap();
        let roundtripped: SnapshotCompat = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped, original);
    }

    #[test]
    fn serde_roundtrip_preserves_incompatible() {
        let original = SnapshotCompat::Incompatible {
            snapshot: 7,
            engine: 3,
        };
        let json = serde_json::to_string(&original).unwrap();
        let roundtripped: SnapshotCompat = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped, original);
    }

    #[test]
    fn is_snapshot_compatible_returns_true_for_matching_versions() {
        assert!(is_snapshot_compatible(3, 3));
    }

    #[test]
    fn is_snapshot_compatible_returns_false_for_mismatched_versions() {
        assert!(!is_snapshot_compatible(1, 3));
        assert!(!is_snapshot_compatible(3, 1));
    }
}
