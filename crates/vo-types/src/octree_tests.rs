use super::octree::*;

fn make_tree() -> Octree<i32> {
    Octree::new(BoundingBox::centered(100.0), OctreeConfig::new(4, 4))
}

#[test]
fn create_empty_octree() {
    let tree: Octree<i32> = Octree::new(BoundingBox::centered(100.0), OctreeConfig::default());
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
}

#[test]
fn insert_and_count() {
    let mut tree = make_tree();
    assert!(tree.is_empty());
    tree.insert(Point3::new(1.0, 2.0, 3.0), 42).unwrap();
    assert_eq!(tree.len(), 1);
    assert!(!tree.is_empty());
    tree.insert(Point3::new(-10.0, 5.0, 7.0), 99).unwrap();
    assert_eq!(tree.len(), 2);
}

#[test]
fn insert_out_of_bounds_is_error() {
    let mut tree = make_tree();
    let result = tree.insert(Point3::new(200.0, 0.0, 0.0), 1);
    assert!(matches!(result, Err(OctreeError::OutOfBounds { .. })));
}

#[test]
fn query_empty_tree() {
    let tree = make_tree();
    let results = tree.query(BoundingBox::centered(10.0));
    assert!(results.is_empty());
}

#[test]
fn query_single_point() {
    let mut tree = make_tree();
    let p = Point3::new(1.0, 2.0, 3.0);
    tree.insert(p, 42).unwrap();
    let results = tree.query(BoundingBox::centered(5.0));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, 42);
    assert_eq!(results[0].point, p);
}

#[test]
fn query_excludes_far_points() {
    let mut tree = make_tree();
    tree.insert(Point3::new(1.0, 1.0, 1.0), 1).unwrap();
    tree.insert(Point3::new(50.0, 50.0, 50.0), 2).unwrap();
    tree.insert(Point3::new(-50.0, -50.0, -50.0), 3).unwrap();
    let results = tree.query(BoundingBox::centered(2.0));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, 1);
}

#[test]
fn nearest_empty_tree() {
    let tree = make_tree();
    assert!(tree.nearest(&Point3::ORIGIN).is_none());
}

#[test]
fn nearest_single_point() {
    let mut tree = make_tree();
    tree.insert(Point3::new(5.0, 5.0, 5.0), 10).unwrap();
    let result = tree.nearest(&Point3::ORIGIN);
    assert!(result.is_some());
    assert_eq!(result.unwrap().value, 10);
}

#[test]
fn nearest_picks_closest() {
    let mut tree = make_tree();
    tree.insert(Point3::new(1.0, 0.0, 0.0), 1).unwrap();
    tree.insert(Point3::new(10.0, 0.0, 0.0), 2).unwrap();
    tree.insert(Point3::new(50.0, 0.0, 0.0), 3).unwrap();
    let result = tree.nearest(&Point3::new(2.0, 0.0, 0.0));
    assert_eq!(result.unwrap().value, 1);
}

#[test]
fn subdivision_happens() {
    let mut tree = make_tree(); // bucket_size=4
    for i in 0..5 {
        let offset = i as f64 * 0.1;
        tree.insert(Point3::new(offset, offset, offset), i).unwrap();
    }
    assert_eq!(tree.len(), 5);
    match tree.root() {
        OctreeNode::Interior { .. } => {}
        OctreeNode::Leaf { entries } => {
            panic!(
                "Expected interior node after exceeding bucket_size, got leaf with {} entries",
                entries.len()
            );
        }
    }
}

#[test]
fn max_depth_respected() {
    let config = OctreeConfig::new(1, 1);
    let mut tree = Octree::new(BoundingBox::centered(100.0), config);
    for i in 0..10 {
        let offset = i as f64;
        tree.insert(Point3::new(offset, offset, offset), i).unwrap();
    }
    assert_eq!(tree.len(), 10);
    let results = tree.query(BoundingBox::centered(100.0));
    assert_eq!(results.len(), 10);
}

#[test]
fn entries_returns_all() {
    let mut tree = make_tree();
    for i in 0..20 {
        let offset = i as f64;
        tree.insert(Point3::new(offset, offset, offset), i).unwrap();
    }
    assert_eq!(tree.entries().len(), 20);
}

#[test]
fn serde_roundtrip() {
    let mut tree = make_tree();
    tree.insert(Point3::new(1.0, 2.0, 3.0), 42).unwrap();
    tree.insert(Point3::new(10.0, 20.0, 30.0), 99).unwrap();
    let json = serde_json::to_string(&tree).unwrap();
    let restored: Octree<i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored, tree);
}

#[test]
fn query_after_many_inserts() {
    let mut tree = make_tree();
    for x in -5..=5i32 {
        for y in -5..=5i32 {
            for z in -5..=5i32 {
                tree.insert(
                    Point3::new(x as f64, y as f64, z as f64),
                    x * 100 + y * 10 + z,
                )
                .unwrap();
            }
        }
    }
    assert_eq!(tree.len(), 1331);
    let results = tree.query(BoundingBox::centered(1.5));
    assert_eq!(results.len(), 27);
    let n = tree.nearest(&Point3::ORIGIN).unwrap();
    assert_eq!(n.point, Point3::ORIGIN);
}

#[test]
fn point3_distance() {
    let a = Point3::new(1.0, 2.0, 3.0);
    let b = Point3::new(4.0, 6.0, 3.0);
    assert!((a.distance(&b) - 5.0).abs() < 1e-10);
    assert!((a.distance_sq(&b) - 25.0).abs() < 1e-10);
}

#[test]
fn bounding_box_octant_roundtrip() {
    let bb = BoundingBox::centered(10.0);
    let p = Point3::new(3.0, -2.0, 7.0);
    let idx = bb.octant_index(&p).unwrap();
    let child = bb.octant(idx);
    assert!(child.contains(&p));
}
