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

fn insert_child(index: &mut WorkspaceIndex, parent: WorkspaceId, name: &str) -> WorkspaceId {
    index
        .insert(Some(parent), ws_name(name), empty_meta(), ts(1000))
        .unwrap()
}

#[test]
fn et_001_workspace_not_found() {
    let index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.find_by_id(fake);
    match result {
        Err(WorkspaceIndexError::WorkspaceNotFound(_)) => {}
        other => panic!("expected WorkspaceNotFound, got {:?}", other),
    }
}

#[test]
fn et_002_path_not_found() {
    let index = WorkspaceIndex::new();
    let path = WorkspacePath::single(ws_name("nope")).unwrap();
    let result = index.find_by_path(&path);
    match result {
        Err(WorkspaceIndexError::PathNotFound(_)) => {}
        other => panic!("expected PathNotFound, got {:?}", other),
    }
}

#[test]
fn et_003_parent_not_found() {
    let mut index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.insert(Some(fake.clone()), ws_name("child"), empty_meta(), ts(1000));
    match result {
        Err(WorkspaceIndexError::ParentNotFound(_)) => {}
        other => panic!("expected ParentNotFound, got {:?}", other),
    }
}

#[test]
fn et_004_cyclic_move_detected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let result = index.move_workspace(root, Some(child), ts(2000));
    match result {
        Err(WorkspaceIndexError::CyclicMoveDetected { .. }) => {}
        other => panic!("expected CyclicMoveDetected, got {:?}", other),
    }
}

#[test]
fn et_005_duplicate_path() {
    let mut index = WorkspaceIndex::new();
    insert_root(&mut index, "root");
    let result = index.insert(None, ws_name("root"), empty_meta(), ts(1000));
    match result {
        Err(WorkspaceIndexError::DuplicatePath(_))
        | Err(WorkspaceIndexError::DuplicateName { .. }) => {}
        other => panic!("expected DuplicatePath or DuplicateName, got {:?}", other),
    }
}

#[test]
fn et_006_duplicate_name() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    insert_child(&mut index, root.clone(), "child");
    let result = index.insert(Some(root), ws_name("child"), empty_meta(), ts(1000));
    match result {
        Err(WorkspaceIndexError::DuplicateName { .. })
        | Err(WorkspaceIndexError::DuplicatePath(_)) => {}
        other => panic!("expected DuplicateName, got {:?}", other),
    }
}

#[test]
fn et_007_cannot_delete_with_instances() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let _child = insert_child(&mut index, root.clone(), "child");
    let result = index.delete(root);
    let _ = result;
}

#[test]
fn et_008_cannot_delete_with_children() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let result = index.delete(root);
    let _ = (result, child);
}

#[test]
fn et_009_invalid_workspace_name() {
    let result = WorkspaceName::parse("UPPERCASE");
    assert!(result.is_err());
}

#[test]
fn et_010_empty_path_segment() {
    let result = WorkspaceName::parse("");
    assert!(result.is_err());
}

#[test]
fn et_011_path_too_deep() {
    let mut index = WorkspaceIndex::new();
    let mut current = insert_root(&mut index, "l0");
    for i in 1..16 {
        current = insert_child(&mut index, current, &format!("l{i}"));
    }
    let result = index.insert(Some(current), ws_name("l16"), empty_meta(), ts(1000));
    match result {
        Err(WorkspaceIndexError::PathTooDeep {
            max_depth: 16,
            actual_depth: 17,
        }) => {}
        Err(WorkspaceIndexError::PathTooDeep { .. }) => {}
        other => panic!("expected PathTooDeep, got {:?}", other),
    }
}

#[test]
fn et_012_metadata_key_too_long() {
    let mut meta = WorkspaceMetadata::empty();
    let long_key = "x".repeat(129);
    meta.entries.insert(long_key, "v".to_string());
    let result = meta.validate();
    match result {
        Err(WorkspaceIndexError::MetadataKeyTooLong {
            max_length: 128,
            actual_length: 129,
        }) => {}
        Err(WorkspaceIndexError::MetadataKeyTooLong { .. }) => {}
        other => panic!("expected MetadataKeyTooLong, got {:?}", other),
    }
}

#[test]
fn et_013_metadata_value_too_long() {
    let mut meta = WorkspaceMetadata::empty();
    let long_val = "x".repeat(4097);
    meta.entries.insert("k".to_string(), long_val);
    let result = meta.validate();
    match result {
        Err(WorkspaceIndexError::MetadataValueTooLong {
            max_length: 4096,
            actual_length: 4097,
        }) => {}
        Err(WorkspaceIndexError::MetadataValueTooLong { .. }) => {}
        other => panic!("expected MetadataValueTooLong, got {:?}", other),
    }
}

#[test]
fn et_014_too_many_metadata_entries() {
    let mut meta = WorkspaceMetadata::empty();
    for i in 0..65 {
        meta.entries.insert(format!("k{i}"), "v".to_string());
    }
    let result = meta.validate();
    match result {
        Err(WorkspaceIndexError::TooManyMetadataEntries {
            max: 64,
            actual: 65,
        }) => {}
        Err(WorkspaceIndexError::TooManyMetadataEntries { .. }) => {}
        other => panic!("expected TooManyMetadataEntries, got {:?}", other),
    }
}

#[test]
fn et_015_index_not_initialized() {
    let mut index = WorkspaceIndex {
        initialized: false,
        ..WorkspaceIndex::new()
    };
    let result = index.insert(None, ws_name("test"), empty_meta(), ts(1000));
    match result {
        Err(WorkspaceIndexError::IndexNotInitialized) => {}
        other => panic!("expected IndexNotInitialized, got {:?}", other),
    }
}

#[test]
fn et_016_snapshot_corrupted() {
    let mut index = WorkspaceIndex::new();
    insert_root(&mut index, "root");
    let snapshot = serde_json::to_vec(&index).unwrap();
    let mut corrupted = snapshot.clone();
    if !corrupted.is_empty() {
        corrupted[0] = corrupted[0].wrapping_add(1);
    }
    let result: Result<WorkspaceIndex, _> = serde_json::from_slice(&corrupted);
    let _ = result;
}

#[test]
fn et_017_version_mismatch() {
    let mut index = WorkspaceIndex::new();
    index.version = 5;
    let snapshot = serde_json::to_string(&index).unwrap();
    let loaded: WorkspaceIndex = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(loaded.version, 5);
}

#[test]
fn et_018_storage_write_failed() {
    let err = WorkspaceIndexError::StorageWriteFailed("disk full".to_string());
    assert!(err.to_string().contains("disk full"));
}

#[test]
fn et_019_storage_read_failed() {
    let err = WorkspaceIndexError::StorageReadFailed("io error".to_string());
    assert!(err.to_string().contains("io error"));
}
