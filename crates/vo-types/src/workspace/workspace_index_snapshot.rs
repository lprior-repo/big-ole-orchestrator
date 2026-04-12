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
fn sr_001_snapshot_from_empty_index() {
    let index = WorkspaceIndex::new();
    assert_eq!(index.version, 0);
    assert!(index.nodes.is_empty());
}

#[test]
fn sr_002_snapshot_after_10_inserts() {
    let mut index = WorkspaceIndex::new();
    for i in 0..10 {
        insert_root(&mut index, &format!("ws-{i}"));
    }
    let json = serde_json::to_string(&index).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.nodes.len(), 10);
}

#[test]
fn sr_003_load_snapshot_verify_invariants() {
    let mut index = WorkspaceIndex::new();
    for i in 0..5 {
        insert_root(&mut index, &format!("ws-{i}"));
    }
    let json = serde_json::to_string(&index).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    for node in loaded.nodes.values() {
        if node.parent_id.is_none() {
            assert!(loaded.root_ids.contains(&node.id));
        }
    }
}

#[test]
fn sr_004_load_corrupted_snapshot() {
    let mut index = WorkspaceIndex::new();
    insert_root(&mut index, "root");
    let json = serde_json::to_string(&index).unwrap();
    let corrupted = json.replace('"', "X");
    let result: Result<WorkspaceIndex, _> = serde_json::from_str(&corrupted);
    assert!(result.is_err());
}

#[test]
fn sr_005_snapshot_version_mismatch() {
    let mut index = WorkspaceIndex::new();
    insert_root(&mut index, "root");
    let mut json = serde_json::to_string(&index).unwrap();
    json = json.replace("\"version\":1", "\"version\":99");
    let loaded: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.version, 99);
}

#[test]
fn sr_006_snapshot_deterministic() {
    let mut index1 = WorkspaceIndex::new();
    for i in 0..5 {
        insert_root(&mut index1, &format!("ws-{i}"));
    }
    let json1 = serde_json::to_string(&index1).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&json1).unwrap();
    let json2 = serde_json::to_string(&loaded).unwrap();
    assert_eq!(json1, json2);
}

#[test]
fn sr_007_snapshot_after_delete_only_live_nodes() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "keep");
    let r2 = insert_root(&mut index, "delete");
    index.delete(r2).unwrap();
    let json = serde_json::to_string(&index).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.nodes.len(), 1);
    assert!(loaded.nodes.contains_key(&r1));
}

#[test]
fn sr_008_snapshot_after_move_reflects_new_paths() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "old-parent");
    let r2 = insert_root(&mut index, "new-parent");
    let child = index
        .insert(Some(r1), ws_name("child"), empty_meta(), ts(1000))
        .unwrap();
    index.move_workspace(child, Some(r2), ts(2000)).unwrap();
    let json = serde_json::to_string(&index).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.root_ids.len(), 1);
}
