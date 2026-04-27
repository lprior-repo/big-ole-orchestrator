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
fn ec_001_insert_into_freshly_created_empty_index() {
    let mut index = WorkspaceIndex::new();
    let id = insert_root(&mut index, "first");
    assert_eq!(index.version, 1);
    assert!(index.nodes.contains_key(&id));
}

#[test]
fn ec_002_delete_the_only_root() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "only");
    index.delete(root).unwrap();
    assert_eq!(index.nodes.len(), 0);
    assert_eq!(index.root_ids.len(), 0);
    assert_eq!(index.path_index.len(), 0);
}

#[test]
fn ec_003_move_to_same_parent_idempotent() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let child = insert_child(&mut index, root.clone(), "child");
    let v_before = index.version;
    index.move_workspace(child, Some(root), ts(2000)).unwrap();
    assert!(index.version > v_before);
}

#[test]
fn ec_004_insert_with_max_length_name() {
    let long_name = "a".repeat(64);
    let mut index = WorkspaceIndex::new();
    let id = index
        .insert(
            None,
            WorkspaceName::parse(&long_name).unwrap(),
            empty_meta(),
            ts(1000),
        )
        .unwrap();
    let node = index.nodes.get(&id).unwrap();
    assert_eq!(node.name.as_str().len(), 64);
}

#[test]
fn ec_005_find_by_path_with_single_segment() {
    let mut index = WorkspaceIndex::new();
    let root = insert_root(&mut index, "root");
    let path = WorkspacePath::single(ws_name("root")).unwrap();
    assert_eq!(index.find_by_path(&path).unwrap(), root);
}

#[test]
fn ec_006_insert_child_then_immediately_delete_parent() {
    let mut index = WorkspaceIndex::new();
    let parent = insert_root(&mut index, "parent");
    let child = insert_child(&mut index, parent, "child");
    let root_id = index.root_ids[0].clone();
    index.delete(root_id).unwrap();
    assert!(!index.nodes.contains_key(&child));
}

#[test]
fn ec_007_insert_move_then_delete() {
    let mut index = WorkspaceIndex::new();
    let r1 = insert_root(&mut index, "r1");
    let r2 = insert_root(&mut index, "r2");
    let child = insert_child(&mut index, r1, "child");
    index
        .move_workspace(child.clone(), Some(r2), ts(2000))
        .unwrap();
    index.delete(child).unwrap();
    assert_eq!(index.nodes.len(), 2);
}

#[test]
fn ec_008_metadata_64_entries_all_boundary_sizes() {
    let mut meta = WorkspaceMetadata::empty();
    for i in 0..64 {
        let key = format!("k{:03}", i);
        let val = "v".repeat(100);
        meta.entries.insert(key, val);
    }
    assert_eq!(meta.entries.len(), 64);
    assert!(meta.validate().is_ok());
}

#[test]
fn ec_009_tree_with_only_roots_no_nesting() {
    let mut index = WorkspaceIndex::new();
    for i in 0..10 {
        insert_root(&mut index, &format!("root-{i}"));
    }
    assert_eq!(index.root_ids.len(), 10);
    assert_eq!(index.nodes.len(), 10);
    for node in index.nodes.values() {
        assert!(node.is_root());
        assert!(node.is_leaf());
    }
}

#[test]
fn ec_010_insert_with_metadata_then_clear() {
    let mut index = WorkspaceIndex::new();
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("key".to_string(), "val".to_string());
    let id = index.insert(None, ws_name("root"), meta, ts(1000)).unwrap();
    index
        .update_metadata(id.clone(), empty_meta(), ts(2000))
        .unwrap();
    let node = index.nodes.get(&id).unwrap();
    assert!(node.metadata.entries.is_empty());
}

#[test]
fn ec_011_ulid_monotonicity_under_rapid_generation() {
    let mut ids = vec![];
    for _ in 0..100 {
        ids.push(WorkspaceId::generate());
    }
    for window in ids.windows(2) {
        assert!(window[0].as_ulid() < window[1].as_ulid());
    }
}

#[test]
fn ec_012_path_with_max_64_char_segments_at_all_16_levels() {
    let seg = "a".repeat(64);
    let name = WorkspaceName::parse(&seg).unwrap();
    let mut segments = vec![];
    for _ in 0..16 {
        segments.push(name.clone());
    }
    let path = WorkspacePath::new(NonEmptyVec::new_unchecked(segments)).unwrap();
    assert_eq!(path.depth(), 16);
}
