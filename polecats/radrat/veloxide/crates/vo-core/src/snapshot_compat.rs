//! Snapshot schema version compatibility check (ADR-016, ADR-035).
//!
//! On rehydration, if a snapshot's schema_version doesn't match the current
//! engine version, the engine must either upcast or discard the snapshot.
//! Snapshots are a cache, not a schema authority (ADR-035 §3).

use serde::{Deserialize, Serialize};

/// Result of comparing a snapshot's schema version against the engine's
/// current maximum supported version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapshotCompat {
    /// Snapshot version matches engine version exactly. Safe to use as-is.
    Compatible,
    /// Snapshot version is older than engine version. Upcasting is required.
    NeedsUpcast { from: u16, to: u16 },
    /// Snapshot version is newer than engine or is zero (invalid).
    /// Discard and rebuild from event log.
    Incompatible { snapshot: u16, engine: u16 },
}

impl SnapshotCompat {
    /// Returns `true` if this result is `Compatible`.
    #[must_use]
    pub fn is_compatible(self) -> bool {
        matches!(self, Self::Compatible)
    }
}

/// Returns `true` when the snapshot version exactly matches the engine version.
///
/// For the full compatibility assessment (including upcast and incompatible
/// cases), use [`check_snapshot_compat`].
#[must_use]
pub fn is_snapshot_compatible(snapshot_version: u16, engine_version: u16) -> bool {
    snapshot_version == engine_version
}

/// Full compatibility assessment between a snapshot's schema version and the
/// engine's current maximum supported schema version.
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
