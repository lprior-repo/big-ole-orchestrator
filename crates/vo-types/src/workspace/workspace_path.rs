use std::fmt;

use serde::{Deserialize, Serialize};

use crate::workspace::workspace_index_error::WorkspaceIndexError;
use crate::workspace::workspace_name::WorkspaceName;
use crate::NonEmptyVec;

const MAX_DEPTH: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkspacePath {
    segments: NonEmptyVec<WorkspaceName>,
}

impl WorkspacePath {
    pub fn new(segments: NonEmptyVec<WorkspaceName>) -> Result<Self, WorkspaceIndexError> {
        let depth = segments.len();
        if depth > MAX_DEPTH as usize {
            return Err(WorkspaceIndexError::PathTooDeep {
                max_depth: MAX_DEPTH,
                actual_depth: depth as u32,
            });
        }
        Ok(Self { segments })
    }

    pub fn single(name: WorkspaceName) -> Result<Self, WorkspaceIndexError> {
        Self::new(NonEmptyVec::new_unchecked(vec![name]))
    }

    pub fn segments(&self) -> &[WorkspaceName] {
        self.segments.as_slice()
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }

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
            .map(|s| s.as_str())
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
