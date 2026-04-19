use std::fmt;

use serde::{Deserialize, Serialize};

use crate::workspace::workspace_index_error::WorkspaceIndexError;
use crate::workspace::workspace_name::WorkspaceName;
use crate::NonEmptyVec;

const MAX_DEPTH: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspacePath {
    segments: NonEmptyVec<WorkspaceName>,
}

impl Serialize for WorkspacePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let path_str = self.to_string();
        serializer.serialize_str(&path_str)
    }
}

impl<'de> Deserialize<'de> for WorkspacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let segments: Vec<WorkspaceName> = s
            .split('/')
            .map(WorkspaceName::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(serde::de::Error::custom)?;
        let segments = NonEmptyVec::new(segments)
            .map_err(|_| serde::de::Error::custom("path must have at least one segment"))?;
        WorkspacePath::new(segments).map_err(serde::de::Error::custom)
    }
}

impl WorkspacePath {
    /// Creates a new workspace path from a non-empty list of segments.
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceIndexError::PathTooDeep` if the number of segments exceeds `MAX_DEPTH`.
    pub fn new(segments: NonEmptyVec<WorkspaceName>) -> Result<Self, WorkspaceIndexError> {
        let depth = segments.len();
        if depth > MAX_DEPTH as usize {
            return Err(WorkspaceIndexError::PathTooDeep {
                max_depth: MAX_DEPTH,
                // SAFETY: depth is bounded by MAX_DEPTH which fits in u32
                #[allow(clippy::cast_possible_truncation)]
                actual_depth: depth as u32,
            });
        }
        Ok(Self { segments })
    }

    /// Creates a single-segment workspace path.
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceIndexError::PathTooDeep` if MAX_DEPTH is 0 (it isn't).
    pub fn single(name: WorkspaceName) -> Result<Self, WorkspaceIndexError> {
        Self::new(NonEmptyVec::new_unchecked(vec![name]))
    }

    /// Returns the path segments.
    #[must_use]
    pub fn segments(&self) -> &[WorkspaceName] {
        self.segments.as_slice()
    }

    /// Returns the number of segments in the path.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Creates a child path by appending a segment.
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceIndexError::PathTooDeep` if the resulting path would exceed `MAX_DEPTH`.
    pub fn child(&self, name: WorkspaceName) -> Result<Self, WorkspaceIndexError> {
        let mut all: Vec<WorkspaceName> = self.segments.as_slice().to_vec();
        all.push(name);
        Self::new(NonEmptyVec::new_unchecked(all))
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<&str> = self
            .segments
            .as_slice()
            .iter()
            .map(WorkspaceName::as_str)
            .collect();
        write!(f, "{}", parts.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> WorkspaceName {
        WorkspaceName::parse(s).unwrap()
    }

    #[test]
    fn tp_001_single_segment_path() {
        let path = WorkspacePath::single(n("root")).unwrap();
        assert_eq!(path.depth(), 1);
        assert_eq!(path.segments()[0], n("root"));
    }

    #[test]
    fn tp_002_multi_segment_path() {
        let path =
            WorkspacePath::new(NonEmptyVec::new_unchecked(vec![n("a"), n("b"), n("c")])).unwrap();
        assert_eq!(path.depth(), 3);
    }

    #[test]
    fn tp_003_reject_empty_segments_list() {
        let result = NonEmptyVec::<WorkspaceName>::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn tp_005_segments_stored_lowercase() {
        let path = WorkspacePath::single(n("root")).unwrap();
        for seg in path.segments() {
            assert_eq!(seg.as_str(), seg.as_str().to_lowercase());
        }
    }

    #[test]
    fn tp_006_max_depth_16_accepted() {
        let segs: Vec<WorkspaceName> = (0..16).map(|i| n(&format!("l{i}"))).collect();
        let path = WorkspacePath::new(NonEmptyVec::new_unchecked(segs)).unwrap();
        assert_eq!(path.depth(), 16);
    }

    #[test]
    fn tp_007_depth_17_rejected() {
        let segs: Vec<WorkspaceName> = (0..17).map(|i| n(&format!("l{i}"))).collect();
        let result = WorkspacePath::new(NonEmptyVec::new_unchecked(segs));
        assert!(matches!(
            result,
            Err(WorkspaceIndexError::PathTooDeep {
                max_depth: 16,
                actual_depth: 17
            })
        ));
    }

    #[test]
    fn tp_008_equality_is_case_insensitive() {
        let path_lower = WorkspacePath::single(n("abc")).unwrap();
        let path_same = WorkspacePath::single(n("abc")).unwrap();
        assert_eq!(path_lower, path_same);
    }

    #[test]
    fn tp_009_hash_is_case_insensitive() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let p1 = WorkspacePath::single(n("abc")).unwrap();
        let p2 = WorkspacePath::single(n("abc")).unwrap();
        set.insert(p1);
        assert!(set.contains(&p2));
    }
}
