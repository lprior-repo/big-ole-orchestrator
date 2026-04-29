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
fn so_001_insert_a_insert_b_under_a_delete_a() {
    let mut index = WorkspaceIndex::new();
    let a = insert_root(&mut index, "a");
    let b = insert_child(&mut index, a, "b");
    index.delete(a).unwrap();
    assert!(!index.nodes.contains_key(&b));
    assert!(index.nodes.is_empty());
}

#[test]
fn so_002_insert_a_insert_b_move_b_to_root() {
    let mut index = WorkspaceIndex::new();
    let a = insert_root(&mut index, "a");
    let b = insert_child(&mut index, a.clone(), "b");
    index.move_workspace(b.clone(), None, ts(2000)).unwrap();
    assert!(index.root_ids.contains(&b));
    let a_node = index.nodes.get(&a).unwrap();
    assert!(!a_node.children.contains(&b));
}

#[test]
fn so_003_insert_abc_move_a_under_c_cyclic() {
    let mut index = WorkspaceIndex::new();
    let a = insert_root(&mut index, "a");
    let b = insert_child(&mut index, a.clone(), "b");
    let c = insert_child(&mut index, b, "c");
    let result = index.move_workspace(a, Some(c), ts(2000));
    assert!(matches!(
        result,
        Err(WorkspaceIndexError::CyclicMoveDetected { .. })
    ));
}

#[test]
fn so_004_build_16_level_delete_level_8() {
    let mut index = WorkspaceIndex::new();
    let mut current = insert_root(&mut index, "l0");
    let mut level_8 = None;
    for i in 1..=15 {
        current = insert_child(&mut index, current, &format!("l{i}"));
        if i == 7 {
            level_8 = Some(current.clone());
        }
    }
    assert_eq!(index.nodes.len(), 16);
    if let Some(l8) = level_8 {
        index.delete(l8).unwrap();
        assert!(index.nodes.len() < 16);
    }
}

#[test]
fn so_005_insert_100_roots_delete_every_other() {
    let mut index = WorkspaceIndex::new();
    let mut ids = vec![];
    for i in 0..100 {
        ids.push(insert_root(&mut index, &format!("ws-{i}")));
    }
    assert_eq!(index.root_ids.len(), 100);
    for i in (0..100).step_by(2) {
        index.delete(ids[i].clone()).unwrap();
    }
    assert_eq!(index.root_ids.len(), 50);
}

#[test]
fn so_006_insert_tree_move_subtree_5_times() {
    let mut index = WorkspaceIndex::new();
    let mut parents = vec![];
    for i in 0..6 {
        parents.push(insert_root(&mut index, &format!("p{i}")));
    }
    let sub = insert_child(&mut index, parents[0].clone(), "sub");
    for i in 1..6 {
        index
            .move_workspace(
                sub.clone(),
                Some(parents[i].clone()),
                ts(1000 + i as u64 * 100),
            )
            .unwrap();
    }
    let node = index.nodes.get(&sub).unwrap();
    assert_eq!(node.parent_id, Some(parents[5].clone()));
}

#[test]
fn so_007_insert_update_metadata_move_delete_parent() {
    let mut index = WorkspaceIndex::new();
    let parent = insert_root(&mut index, "parent");
    let mut meta = WorkspaceMetadata::empty();
    meta.entries.insert("env".to_string(), "test".to_string());
    let child = index
        .insert(Some(parent), ws_name("child"), meta, ts(1000))
        .unwrap();
    index
        .update_metadata(child.clone(), empty_meta(), ts(2000))
        .unwrap();
    index.delete(index.root_ids[0].clone()).unwrap();
    assert!(!index.nodes.contains_key(&child));
}

#[test]
fn so_008_interleaved_insert_delete() {
    let mut index = WorkspaceIndex::new();
    let mut ids = vec![];
    for i in 0..20 {
        let id = insert_root(&mut index, &format!("ws-{i}"));
        ids.push(id);
        if i % 3 == 0 && !ids.is_empty() {
            let last = ids.pop().unwrap();
            if index.nodes.contains_key(&last) {
                index.delete(last).unwrap();
            }
        }
    }
    let total_nodes = index.nodes.len();
    let total_roots = index.root_ids.len();
    assert_eq!(total_nodes, total_roots);
}
