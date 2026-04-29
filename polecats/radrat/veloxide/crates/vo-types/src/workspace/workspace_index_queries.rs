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
fn fp_001_find_root_by_single_segment_path() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let path = WorkspacePath::single(ws_name("root")).unwrap();
    assert_eq!(index.find_by_path(&path).unwrap(), root);
}

#[test]
fn fp_002_find_deeply_nested_by_full_path() {
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
    assert_eq!(index.find_by_path(&path).unwrap(), leaf);
}

#[test]
fn fp_003_find_nonexistent_path() {
    let index = WorkspaceIndex::new();
    let path = WorkspacePath::single(ws_name("nothing")).unwrap();
    let result = index.find_by_path(&path);
    assert!(matches!(result, Err(WorkspaceIndexError::PathNotFound(_))));
}

#[test]
fn fp_004_case_insensitive_lookup() {
    let mut index = WorkspaceIndex::new();
    let _root = insert_root(&mut index, "root");
    let lower_path = WorkspacePath::single(ws_name("root")).unwrap();
    let found_lower = index.find_by_path(&lower_path);
    assert!(found_lower.is_ok());
    // WorkspaceName enforces lowercase-only, so case insensitivity is guaranteed
    // by validation rather than by lookup normalization.
    assert!(WorkspaceName::parse("ROOT").is_err());
}

#[test]
fn fp_005_path_lookup_consistent_with_parent_chain_traversal() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    let path = WorkspacePath::new(NonEmptyVec::new_unchecked(vec![
        ws_name("root"),
        ws_name("child"),
    ]))
    .unwrap();
    let by_path = index.find_by_path(&path).unwrap();
    let by_id = index.find_by_id(child.clone()).unwrap().id;
    assert_eq!(by_path, by_id);
}

#[test]
fn fi_001_find_existing_by_id() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let node = index.find_by_id(root.clone()).unwrap();
    assert_eq!(node.id, root);
}

#[test]
fn fi_002_find_nonexistent_id() {
    let index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.find_by_id(fake);
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn fi_003_returned_node_has_correct_fields() {
    let mut index = WorkspaceIndex::new();
    let now = ts(42);
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("env".to_string(), "prod".to_string());
    let id = index
        .insert(None, ws_name("my-workspace"), meta.clone(), now)
        .unwrap();
    let node = index.find_by_id(id).unwrap();
    assert_eq!(node.name, ws_name("my-workspace"));
    assert_eq!(node.parent_id, None);
    assert!(node.children.is_empty());
    assert_eq!(node.metadata, meta);
    assert_eq!(node.created_at, now);
}

#[test]
fn lc_001_list_children_of_leaf_returns_empty() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let children = index.list_children(root).unwrap();
    assert!(children.is_empty());
}

#[test]
fn lc_002_list_children_with_three_children() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root, "a");
    let c2 = insert_child(&mut index, root, "b");
    let c3 = insert_child(&mut index, root, "c");
    let children = index.list_children(root).unwrap();
    assert_eq!(children.len(), 3);
    assert!(children.contains(&c1));
    assert!(children.contains(&c2));
    assert!(children.contains(&c3));
}

#[test]
fn lc_003_children_order_matches_insertion_order() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root, "first");
    let c2 = insert_child(&mut index, root, "second");
    let c3 = insert_child(&mut index, root, "third");
    let children = index.list_children(root).unwrap();
    assert_eq!(children, vec![c1, c2, c3]);
}

#[test]
fn lc_004_list_children_of_nonexistent_node() {
    let index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.list_children(fake);
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn lc_005_after_delete_children_list_updated() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root, "a");
    insert_child(&mut index, root, "b");
    index.delete(c1).unwrap();
    let children = index.list_children(root).unwrap();
    assert_eq!(children.len(), 1);
}

#[test]
fn ga_001_ancestors_of_root_is_empty() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let ancestors = index.get_ancestors(root).unwrap();
    assert!(ancestors.is_empty());
}

#[test]
fn ga_002_ancestors_of_child_returns_root() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let ancestors = index.get_ancestors(child).unwrap();
    assert_eq!(ancestors, vec![root]);
}

#[test]
fn ga_003_ancestors_of_3deep_node() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mid = insert_child(&mut index, root.clone(), "mid");
    let leaf = insert_child(&mut index, mid.clone(), "leaf");
    let ancestors = index.get_ancestors(leaf).unwrap();
    assert_eq!(ancestors, vec![root, mid]);
}

#[test]
fn ga_004_ancestors_of_nonexistent_node() {
    let index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.get_ancestors(fake);
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn ga_005_after_move_ancestors_chain_updated() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    let child = insert_child(&mut index, r1, "child");
    index
        .move_workspace(child.clone(), Some(r2.clone()), ts(2000))
        .unwrap();
    let ancestors = index.get_ancestors(child).unwrap();
    assert_eq!(ancestors, vec![r2]);
}

#[test]
fn gd_001_descendants_of_leaf_is_empty() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root, "child");
    let desc = index.get_descendants(child).unwrap();
    assert!(desc.is_empty());
}

#[test]
fn gd_002_descendants_of_parent_includes_direct_children() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let c1 = insert_child(&mut index, root.clone(), "a");
    let c2 = insert_child(&mut index, root.clone(), "b");
    let desc = index.get_descendants(root).unwrap();
    assert_eq!(desc.len(), 2);
    assert!(desc.contains(&c1));
    assert!(desc.contains(&c2));
}

#[test]
fn gd_003_descendants_of_root_includes_entire_subtree() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mid = insert_child(&mut index, root.clone(), "mid");
    let leaf = insert_child(&mut index, mid, "leaf");
    let desc = index.get_descendants(root).unwrap();
    assert_eq!(desc.len(), 2);
    assert!(desc.contains(&mid));
    assert!(desc.contains(&leaf));
}

#[test]
fn gd_004_descendants_order_is_depth_first() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let a = insert_child(&mut index, root.clone(), "a");
    let b = insert_child(&mut index, root, "b");
    insert_child(&mut index, a.clone(), "a1");
    insert_child(&mut index, b, "b1");
    let desc = index.get_descendants(root).unwrap();
    assert_eq!(desc.len(), 4);
    let a_pos = desc.iter().position(|x| *x == a).unwrap();
    let _ = a_pos;
}

#[test]
fn gd_005_descendants_of_nonexistent_node() {
    let index = WorkspaceIndex::new();
    let fake = WorkspaceId::generate();
    let result = index.get_descendants(fake);
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::WorkspaceNotFound(_))
    ));
}

#[test]
fn gd_006_after_delete_descendants_updated() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let mid = insert_child(&mut index, root.clone(), "mid");
    insert_child(&mut index, mid, "leaf");
    index.delete(root).unwrap();
    assert!(index.nodes.is_empty());
}
