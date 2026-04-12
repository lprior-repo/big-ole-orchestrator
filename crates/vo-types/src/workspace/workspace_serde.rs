use super::*;
use crate::*;

fn ts(v: u64) -> TimestampMs {
    TimestampMs::try_from(v).unwrap()
}

fn ws_name(s: &str) -> WorkspaceName {
    WorkspaceName::parse(s).unwrap()
}

fn empty_meta() -> WorkspaceMetadata {
    WorkspaceMetadata::empty()
}

fn insert_root(index: &mut WorkspaceIndex, name: &str) -> WorkspaceId {
    index
        .insert(None, ws_name(name), empty_meta(), ts(1000))
        .unwrap()
}

#[test]
fn se_001_workspace_node_json_roundtrip() {
    let mut index = WorkspaceIndex::new();
    let id = insert_root(&mut index, "root");
    let node = index.find_by_id(id).unwrap();
    let json = serde_json::to_string(&node).unwrap();
    let restored: WorkspaceNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node, restored);
}

#[test]
fn se_002_workspace_index_json_roundtrip() {
    let mut index = WorkspaceIndex::new();
    for i in 0..3 {
        insert_root(&mut index, &format!("ws-{i}"));
    }
    let json = serde_json::to_string(&index).unwrap();
    let restored: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(index, restored);
}

#[test]
fn se_003_workspace_path_json_roundtrip() {
    let path = super::WorkspacePath::new(crate::NonEmptyVec::new_unchecked(vec![
        ws_name("a"),
        ws_name("b"),
        ws_name("c"),
    ]))
    .unwrap();
    let json = serde_json::to_string(&path).unwrap();
    let restored: super::WorkspacePath = serde_json::from_str(&json).unwrap();
    assert_eq!(path, restored);
}

#[test]
fn se_004_workspace_index_error_json_roundtrip() {
    let err = WorkspaceIndexError::WorkspaceNotFound(WorkspaceId::generate());
    let json = serde_json::to_string(&err).unwrap();
    let restored: WorkspaceIndexError = serde_json::from_str(&json).unwrap();
    assert_eq!(err, restored);
}

#[test]
fn se_005_snapshot_serialization_deterministic() {
    let mut index = WorkspaceIndex::new();
    for i in 0..5 {
        insert_root(&mut index, &format!("ws-{i}"));
    }
    let json1 = serde_json::to_string(&index).unwrap();
    let json2 = serde_json::to_string(&index).unwrap();
    assert_eq!(json1, json2);
}
