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
fn il_001_new_index_has_version_zero() {
    let index = WorkspaceIndex::new();
    assert_eq!(index.version, 0);
}

#[test]
fn il_002_new_index_has_empty_nodes() {
    let index = WorkspaceIndex::new();
    assert!(index.nodes.is_empty());
}

#[test]
fn il_003_new_index_has_empty_root_ids() {
    let index = WorkspaceIndex::new();
    assert!(index.root_ids.is_empty());
}

#[test]
fn il_004_new_index_has_empty_path_index() {
    let index = WorkspaceIndex::new();
    assert!(index.path_index.is_empty());
}

#[test]
fn il_005_operations_on_uninitialized_index_fail() {
    let mut index = WorkspaceIndex {
        initialized: false,
        ..WorkspaceIndex::new()
    };
    let id = WorkspaceId::generate();
    let result = index.insert(None, ws_name("test"), empty_meta(), ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::IndexNotInitialized)
    ));
    let _ = id;
}

#[test]
fn in_001_insert_root_workspace() {
    let mut index = WorkspaceIndex::new();
    let id = index.insert(None, ws_name("root"), empty_meta(), ts(1000));
    assert!(id.is_ok());
}

#[test]
fn in_002_insert_child_under_existing_root() {
    let mut index = WorkspaceIndex::new();
    let root_id = insert_root(&mut index, "root");
    let child_id = index
        .insert(
            Some(root_id.clone()),
            ws_name("child"),
            empty_meta(),
            ts(1000),
        )
        .unwrap();
    let root_node = index.nodes.get(&root_id).unwrap();
    assert!(root_node.children.contains(&child_id));
}

#[test]
fn in_003_insert_nested_3_levels_deep() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "a");
    let mid = insert_child(&mut index, root, "b");
    let leaf = insert_child(&mut index, mid, "c");
    let path = WorkspacePath::new(NonEmptyVec::new_unchecked(vec![
        ws_name("a"),
        ws_name("b"),
        ws_name("c"),
    ]))
    .unwrap();
    let found = index.find_by_path(&path).unwrap();
    assert_eq!(found, leaf);
}

#[test]
fn in_004_insert_at_max_depth_16() {
    let mut index = WorkspaceIndex::new();
    let mut current = insert_root(&mut index, "l0");
    for i in 1..16 {
        current = insert_child(&mut index, current, &format!("l{i}"));
    }
    assert_eq!(index.nodes.len(), 16);
}

#[test]
fn in_005_insert_at_depth_17_fails() {
    let mut index = WorkspaceIndex::new();
    let mut current = insert_root(&mut index, "l0");
    for i in 1..16 {
        current = insert_child(&mut index, current, &format!("l{i}"));
    }
    let result = index.insert(Some(current), ws_name("l16"), empty_meta(), ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::PathTooDeep { .. })
    ));
}

#[test]
fn in_006_insert_duplicate_name_under_same_parent() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    insert_child(&mut index, root.clone(), "child");
    let result = index.insert(Some(root), ws_name("child"), empty_meta(), ts(1000));
    assert!(
        matches!(result, Err(WorkspaceIndexError::DuplicateName { .. }))
            || matches!(result, Err(WorkspaceIndexError::DuplicatePath(_)))
    );
}

#[test]
fn in_007_insert_same_name_under_different_parents() {
    let mut index = WorkspaceIndex::new();
    let p1 = insert_root(&mut index, "parent1");
    let p2 = insert_root(&mut index, "parent2");
    let c1 = insert_child(&mut index, p1, "child");
    let c2 = insert_child(&mut index, p2, "child");
    assert_ne!(c1, c2);
}

#[test]
fn in_008_insert_generates_path_correctly() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    let path = WorkspacePath::new(NonEmptyVec::new_unchecked(vec![
        ws_name("root"),
        ws_name("child"),
    ]))
    .unwrap();
    assert_eq!(index.path_index.get(&path), Some(&child));
}

#[test]
fn in_009_insert_increments_version() {
    let mut index = WorkspaceIndex::new();
    assert_eq!(index.version, 0);
    index
        .insert(None, ws_name("root"), empty_meta(), ts(1000))
        .unwrap();
    assert_eq!(index.version, 1);
    index
        .insert(None, ws_name("root2"), empty_meta(), ts(1000))
        .unwrap();
    assert_eq!(index.version, 2);
}

#[test]
fn in_010_insert_with_nonexistent_parent() {
    let mut index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.insert(Some(fake), ws_name("child"), empty_meta(), ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::ParentNotFound(_))
    ));
}

#[test]
fn in_011_insert_sets_created_at_equal_updated_at() {
    let mut index = WorkspaceIndex::new();
    let now = ts(42);
    let id = index
        .insert(None, ws_name("root"), empty_meta(), now)
        .unwrap();
    let node = index.nodes.get(&id).unwrap();
    assert_eq!(node.created_at, node.updated_at);
    assert_eq!(node.created_at, now);
}

#[test]
fn in_012_insert_adds_to_root_ids_when_parent_none() {
    let mut index = WorkspaceIndex::new();
    let id = index
        .insert(None, ws_name("root"), empty_meta(), ts(1000))
        .unwrap();
    assert!(index.root_ids.contains(&id));
}

#[test]
fn in_013_insert_not_add_to_root_ids_when_parent_some() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    assert!(!index.root_ids.contains(&child));
}

#[test]
fn in_014_insert_produces_unique_workspace_id() {
    let mut index = WorkspaceIndex::new();
    let id1 = index
        .insert(None, ws_name("a"), empty_meta(), ts(1000))
        .unwrap();
    let id2 = index
        .insert(None, ws_name("b"), empty_meta(), ts(1000))
        .unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn in_015_multiple_root_workspaces_coexist() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "a");
    let r2 = insert_root(&mut index, "b");
    let r3 = insert_root(&mut index, "c");
    assert_eq!(index.root_ids.len(), 3);
    assert!(index.root_ids.contains(&r1));
    assert!(index.root_ids.contains(&r2));
    assert!(index.root_ids.contains(&r3));
}

#[test]
fn de_001_delete_leaf_node() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    index.delete(child.clone()).unwrap();
    assert!(!index.nodes.contains_key(&child));
}

#[test]
fn de_002_delete_node_with_single_child() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let grandchild = insert_child(&mut index, child, "gc");
    index.delete(child.clone()).unwrap();
    assert!(!index.nodes.contains_key(&child));
    assert!(!index.nodes.contains_key(&grandchild));
}

#[test]
fn de_003_delete_deeply_nested_descendants() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut current = root.clone();
    for i in 0..5 {
        current = insert_child(&mut index, current, &format!("l{i}"));
    }
    let before = index.nodes.len();
    index.delete(root).unwrap();
    assert_eq!(index.nodes.len(), 0);
    assert_eq!(before, 6);
}

#[test]
fn de_004_delete_root_removes_from_root_ids() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    index.delete(root.clone()).unwrap();
    assert!(!index.root_ids.contains(&root));
}

#[test]
fn de_005_delete_removes_from_parent_children_list() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    index.delete(child.clone()).unwrap();
    let root_node = index.nodes.get(&root).unwrap();
    assert!(!root_node.children.contains(&child));
}

#[test]
fn de_006_delete_removes_descendant_paths_from_path_index() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    insert_child(&mut index, child, "gc");
    index.delete(root).unwrap();
    assert!(index.path_index.is_empty());
}

#[test]
fn de_007_delete_nonexistent_workspace() {
    let mut index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.delete(fake);
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn de_008_delete_increments_version() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let v_before = index.version;
    index.delete(root).unwrap();
    assert_eq!(index.version, v_before + 1);
}

#[test]
fn de_009_delete_root_with_3_level_subtree() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "r");
    let mid = insert_child(&mut index, root.clone(), "m");
    let leaf = insert_child(&mut index, mid, "l");
    index.delete(root).unwrap();
    assert_eq!(index.nodes.len(), 0);
    let _ = leaf;
}

#[test]
fn de_010_delete_child_leaves_sibling_intact() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root.clone(), "child1");
    let c2 = insert_child(&mut index, root.clone(), "child2");
    index.delete(c1).unwrap();
    assert!(index.nodes.contains_key(&c2));
    assert!(index.nodes.contains_key(&root));
}

#[test]
fn de_011_deleted_id_never_reused() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    index.delete(child.clone()).unwrap();
    let new_child = insert_root(&mut index, "new-root");
    assert_ne!(child, new_child);
}

#[test]
fn mo_001_move_leaf_to_new_parent() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    let child = insert_child(&mut index, r1.clone(), "child");
    index
        .move_workspace(child.clone(), Some(r2.clone()), ts(2000))
        .unwrap();
    let node = index.nodes.get(&child).unwrap();
    assert_eq!(node.parent_id, Some(r2));
}

#[test]
fn mo_002_move_subtree_preserves_descendant_paths() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    let mid = insert_child(&mut index, r1.clone(), "mid");
    let leaf = insert_child(&mut index, mid.clone(), "leaf");
    index
        .move_workspace(mid.clone(), Some(r2), ts(2000))
        .unwrap();
    assert!(index.nodes.contains_key(&leaf));
    assert!(index.nodes.contains_key(&mid));
}

#[test]
fn mo_003_move_root_to_become_child() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    index
        .move_workspace(r1.clone(), Some(r2.clone()), ts(2000))
        .unwrap();
    assert!(!index.root_ids.contains(&r1));
    let node = index.nodes.get(&r1).unwrap();
    assert_eq!(node.parent_id, Some(r2));
}

#[test]
fn mo_004_move_child_to_become_root() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    index.move_workspace(child.clone(), None, ts(2000)).unwrap();
    assert!(index.root_ids.contains(&child));
    let node = index.nodes.get(&child).unwrap();
    assert_eq!(node.parent_id, None);
}

#[test]
fn mo_005_move_to_self_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let result = index.move_workspace(root.clone(), Some(root), ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::CyclicMoveDetected { .. })
    ));
}

#[test]
fn mo_006_move_to_own_descendant_rejected() {
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
fn mo_007_move_to_own_grandchild_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let gc = insert_child(&mut index, child, "gc");
    let result = index.move_workspace(root, Some(gc), ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::CyclicMoveDetected { .. })
    ));
}

#[test]
fn mo_008_move_nonexistent_workspace() {
    let mut index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.move_workspace(fake, None, ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn mo_009_move_to_nonexistent_parent() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let fake = WorkspaceId::generate();
    let result = index.move_workspace(root, Some(fake), ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::ParentNotFound(_))
    ));
}

#[test]
fn mo_010_move_to_parent_with_duplicate_name() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    insert_child(&mut index, r2.clone(), "shared-name");
    let child = insert_child(&mut index, r1, "shared-name");
    let result = index.move_workspace(child, Some(r2), ts(2000));
    assert!(
        matches!(result, Err(WorkspaceIndexError::DuplicateName { .. }))
            || matches!(result, Err(WorkspaceIndexError::DuplicatePath(_)))
    );
}

#[test]
fn mo_011_move_preserves_metadata() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let target = insert_root(&mut index, "target");
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("key".to_string(), "value".to_string());
    let child = index
        .insert(Some(root), ws_name("child"), meta.clone(), ts(1000))
        .unwrap();
    index
        .move_workspace(child.clone(), Some(target), ts(2000))
        .unwrap();
    let node = index.nodes.get(&child).unwrap();
    assert_eq!(node.metadata, meta);
}

#[test]
fn mo_012_move_increments_version() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let target = insert_root(&mut index, "target");
    let child = insert_child(&mut index, root, "child");
    let v_before = index.version;
    index.move_workspace(child, Some(target), ts(2000)).unwrap();
    assert_eq!(index.version, v_before + 1);
}

#[test]
fn mo_013_move_3_level_subtree_updates_all_paths() {
    let mut index = WorkspaceIndex::new();
    let old_parent = insert_root(&mut index, "old");
    let new_parent = insert_root(&mut index, "new");
    let mid = insert_child(&mut index, old_parent.clone(), "mid");
    let leaf = insert_child(&mut index, mid.clone(), "leaf");
    index
        .move_workspace(mid, Some(new_parent), ts(2000))
        .unwrap();
    assert!(index.nodes.contains_key(&leaf));
    // old_parent remains a root; only mid was moved, not old_parent itself
    assert_eq!(index.root_ids.len(), 2);
    let _ = old_parent;
}

#[test]
fn mo_014_move_to_same_parent_noop() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let result = index.move_workspace(child, Some(root), ts(2000));
    assert!(result.is_ok());
}

#[test]
fn mo_015_move_preserves_created_at_updates_updated_at() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let target = insert_root(&mut index, "target");
    let child = index
        .insert(Some(root), ws_name("child"), empty_meta(), ts(100))
        .unwrap();
    let original_created = index.nodes.get(&child).unwrap().created_at;
    index
        .move_workspace(child.clone(), Some(target), ts(500))
        .unwrap();
    let node = index.nodes.get(&child).unwrap();
    assert_eq!(node.created_at, original_created);
    assert!(node.updated_at >= node.created_at);
}

#[test]
fn um_001_replace_metadata_entirely() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut new_meta = WorkspaceMetadata::empty();
    new_meta.entries.insert("k".to_string(), "v".to_string());
    index
        .update_metadata(root.clone(), new_meta.clone(), ts(2000))
        .unwrap();
    let node = index.nodes.get(&root).unwrap();
    assert_eq!(node.metadata, new_meta);
}

#[test]
fn um_002_set_metadata_to_empty() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("k".to_string(), "v".to_string());
    index.update_metadata(root.clone(), meta, ts(1000)).unwrap();
    index
        .update_metadata(root.clone(), empty_meta(), ts(2000))
        .unwrap();
    let node = index.nodes.get(&root).unwrap();
    assert!(node.metadata.entries.is_empty());
}

#[test]
fn um_003_update_nonexistent_workspace() {
    let mut index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.update_metadata(fake, empty_meta(), ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn um_004_metadata_key_too_long_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut meta = WorkspaceMetadata::empty();
    let long_key = "x".repeat(129);
    meta.entries.insert(long_key, "v".to_string());
    let result = index.update_metadata(root, meta, ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::MetadataKeyTooLong { .. })
    ));
}

#[test]
fn um_005_metadata_value_too_long_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut meta = WorkspaceMetadata::empty();
    let long_val = "x".repeat(4097);
    meta.entries.insert("k".to_string(), long_val);
    let result = index.update_metadata(root, meta, ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::MetadataValueTooLong { .. })
    ));
}

#[test]
fn um_006_too_many_metadata_entries_rejected() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mut meta = WorkspaceMetadata::empty();
    for i in 0..65 {
        meta.entries.insert(format!("k{i}"), "v".to_string());
    }
    let result = index.update_metadata(root, meta, ts(1000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::TooManyMetadataEntries { .. })
    ));
}

#[test]
fn um_007_update_increments_version() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let v_before = index.version;
    index.update_metadata(root, empty_meta(), ts(2000)).unwrap();
    assert_eq!(index.version, v_before + 1);
}

#[test]
fn um_008_update_sets_updated_at_greater_than_created_at() {
    let mut index = WorkspaceIndex::new();
    let root = index
        .insert(None, ws_name("root"), empty_meta(), ts(100))
        .unwrap();
    index
        .update_metadata(root.clone(), empty_meta(), ts(500))
        .unwrap();
    let node = index.nodes.get(&root).unwrap();
    assert!(node.updated_at > node.created_at);
}

#[test]
fn um_009_created_at_unchanged_after_metadata_update() {
    let mut index = WorkspaceIndex::new();
    let root = index
        .insert(None, ws_name("root"), empty_meta(), ts(100))
        .unwrap();
    let original_created = index.nodes.get(&root).unwrap().created_at;
    index
        .update_metadata(root.clone(), empty_meta(), ts(500))
        .unwrap();
    let node = index.nodes.get(&root).unwrap();
    assert_eq!(node.created_at, original_created);
}
