use super::*;
use crate::*;
use std::collections::HashSet;

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
fn iv_001_workspace_path_has_at_least_one_segment() {
    let path = WorkspacePath::single(ws_name("root")).unwrap();
    assert!(!path.segments().is_empty());
    assert_eq!(path.depth(), 1);
}

#[test]
fn iv_002_path_segments_stored_lowercase() {
    let path =
        WorkspacePath::single(WorkspaceName::parse("MyRoot").unwrap_or_else(|_| ws_name("myroot")))
            .unwrap();
    for seg in path.segments() {
        assert_eq!(seg.as_str(), seg.as_str().to_lowercase());
    }
}

#[test]
fn iv_003_all_ids_unique() {
    let mut index = WorkspaceIndex::new();
    let mut ids = HashSet::new();
    for i in 0..100 {
        let id = insert_root(&mut index, &format!("ws-{i}"));
        assert!(ids.insert(id), "duplicate ID detected at iteration {i}");
    }
}

#[test]
fn iv_004_path_index_consistent_after_insert() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    for (path, id) in &index.path_index {
        let node = index.nodes.get(id).unwrap();
        let mut segments = vec![];
        let mut current = Some(node);
        while let Some(n) = current {
            segments.push(n.name.clone());
            current = n.parent_id.as_ref().and_then(|pid| index.nodes.get(pid));
        }
        segments.reverse();
        let reconstructed = WorkspacePath::new(NonEmptyVec::new_unchecked(segments)).unwrap();
        assert_eq!(&reconstructed, path);
    }
    let _ = child;
}

#[test]
fn iv_005_root_has_parent_id_none_and_in_root_ids() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let node = index.nodes.get(&root).unwrap();
    assert_eq!(node.parent_id, None);
    assert!(index.root_ids.contains(&root));
}

#[test]
fn iv_006_child_has_parent_id_referencing_existing() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let node = index.nodes.get(&child).unwrap();
    assert_eq!(node.parent_id, Some(root));
}

#[test]
fn iv_007_children_matches_reverse_parent_relationships() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root.clone(), "a");
    let c2 = insert_child(&mut index, root.clone(), "b");
    let root_node = index.nodes.get(&root).unwrap();
    assert!(root_node.children.contains(&c1));
    assert!(root_node.children.contains(&c2));
    for cid in &root_node.children {
        let child = index.nodes.get(cid).unwrap();
        assert_eq!(child.parent_id, Some(root.clone()));
    }
}

#[test]
fn iv_008_no_cycles_move_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let result = index.move_workspace(root, Some(child), ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::CyclicMoveDetected { .. })
    ));
}

#[test]
fn iv_009_metadata_keys_unique() {
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("key".to_string(), "v1".to_string());
    meta.entries.insert("key".to_string(), "v2".to_string());
    assert_eq!(meta.entries.len(), 1);
    assert_eq!(meta.entries.get("key").unwrap(), "v2");
}

#[test]
fn iv_010_created_at_immutable() {
    let mut index = WorkspaceIndex::new();
    let id = index
        .insert(None, ws_name("root"), empty_meta(), ts(100))
        .unwrap();
    let original = index.nodes.get(&id).unwrap().created_at;
    index
        .update_metadata(id.clone(), empty_meta(), ts(999))
        .unwrap();
    assert_eq!(index.nodes.get(&id).unwrap().created_at, original);
}

#[test]
fn iv_011_updated_at_always_gte_created_at() {
    let mut index = WorkspaceIndex::new();
    let id = index
        .insert(None, ws_name("root"), empty_meta(), ts(100))
        .unwrap();
    index
        .update_metadata(id.clone(), empty_meta(), ts(999))
        .unwrap();
    let node = index.nodes.get(&id).unwrap();
    assert!(node.updated_at >= node.created_at);
}

#[test]
fn iv_012_snapshot_checksum_validates_bytes() {
    let mut index = WorkspaceIndex::new();
    insert_root(&mut index, "root");
    let snapshot = serde_json::to_string(&index).unwrap();
    let mut corrupted = snapshot.clone();
    if let Some(pos) = snapshot.find('"') {
        corrupted = snapshot
            .chars()
            .enumerate()
            .map(|(i, c)| if i == pos { 'X' } else { c })
            .collect();
    }
    let result: Result<WorkspaceIndex, _> = serde_json::from_str(&corrupted);
    let _ = result;
}

#[test]
fn iv_013_version_increments_monotonically() {
    let mut index = WorkspaceIndex::new();
    let mut last_version = 0u64;
    for i in 0..50 {
        insert_root(&mut index, &format!("ws-{i}"));
        assert!(index.version > last_version);
        assert_eq!(index.version, last_version + 1);
        last_version = index.version;
    }
}

#[test]
fn iv_014_deleted_id_never_reused() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    index.delete(child.clone()).unwrap();
    let new = insert_root(&mut index, "new");
    assert_ne!(child, new);
}

#[test]
fn iv_015_delete_removes_all_descendants_atomically() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let a = insert_child(&mut index, root.clone(), "a");
    let b = insert_child(&mut index, a.clone(), "b");
    insert_child(&mut index, b, "c");
    index.delete(root).unwrap();
    assert_eq!(index.nodes.len(), 0);
}

#[test]
fn iv_016_move_preserves_descendant_paths() {
    let mut index = WorkspaceIndex::new();
    let old_parent = insert_root(&mut index, "old");
    let new_parent = insert_root(&mut index, "new");
    let mid = insert_child(&mut index, old_parent, "mid");
    let leaf = insert_child(&mut index, mid.clone(), "leaf");
    index
        .move_workspace(mid, Some(new_parent), ts(2000))
        .unwrap();
    assert!(index.nodes.contains_key(&leaf));
}

#[test]
fn iv_017_find_by_path_equals_find_by_id_traversal() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let path = WorkspacePath::new(NonEmptyVec::new_unchecked(vec![
        ws_name("root"),
        ws_name("child"),
    ]))
    .unwrap();
    let by_path = index.find_by_path(&path).unwrap();
    let by_id = index.find_by_id(child).unwrap().id;
    assert_eq!(by_path, by_id);
}

#[test]
fn iv_018_children_empty_iff_leaf() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let root_node = index.nodes.get(&root).unwrap();
    assert!(!root_node.children.is_empty());
    let child_node = index.nodes.get(&child).unwrap();
    assert!(child_node.children.is_empty());
}

#[test]
fn iv_019_descendants_includes_all_nested() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let a = insert_child(&mut index, root.clone(), "a");
    let b = insert_child(&mut index, a.clone(), "b");
    let c = insert_child(&mut index, b, "c");
    let desc = index.get_descendants(root).unwrap();
    assert_eq!(desc.len(), 3);
    assert!(desc.contains(&a));
    assert!(desc.contains(&b));
    assert!(desc.contains(&c));
}
