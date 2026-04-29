//! Proptest suite for Octree.

use proptest::prelude::*;
use vo_common::{Bounds, Octree, Vec3};

proptest! {
    #[test]
    fn query_finds_inserted_points(
        pts in proptest::collection::vec(
            (proptest::num::f64::NORMAL, proptest::num::f64::NORMAL, proptest::num::f64::NORMAL),
            1..30
        )
    ) {
        let bounds = Bounds::new(Vec3::new(-1e6, -1e6, -1e6), Vec3::new(1e6, 1e6, 1e6));
        let mut tree = Octree::new(bounds);
        let mut valid_pts = Vec::new();
        for &(x, y, z) in pts.iter() {
            let pt = Vec3::new(x, y, z);
            if bounds.contains(pt) {
                tree.insert(pt, valid_pts.len() as i32);
                valid_pts.push(pt);
            }
        }
        prop_assert_eq!(tree.len(), valid_pts.len());
        for (i, pt) in valid_pts.iter().enumerate() {
            let r = Bounds::new(*pt, *pt);
            let found = tree.query_range(&r);
            prop_assert!(found.contains(&( &(i as i32) )), "missing point at {:?}", pt);
        }
    }

    #[test]
    fn query_outside_bounds_returns_empty(
        pts in proptest::collection::vec(
            (proptest::num::f64::NORMAL, proptest::num::f64::NORMAL, proptest::num::f64::NORMAL),
            0..20
        )
    ) {
        let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 100.0, 100.0));
        let mut tree = Octree::new(bounds);
        for (i, &(x, y, z)) in pts.iter().enumerate() {
            tree.insert(Vec3::new(x, y, z), i as i32);
        }
        let far = Bounds::new(Vec3::new(200.0, 200.0, 200.0), Vec3::new(300.0, 300.0, 300.0));
        prop_assert!(tree.query_range(&far).is_empty());
    }

    #[test]
    fn insert_outside_bounds_rejected(
        pt in (proptest::num::f64::NORMAL, proptest::num::f64::NORMAL, proptest::num::f64::NORMAL)
    ) {
        let bounds = Bounds::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let mut tree = Octree::new(bounds);
        let p = Vec3::new(pt.0, pt.1, pt.2);
        if !bounds.contains(p) {
            prop_assert!(!tree.insert(p, 0));
            prop_assert_eq!(tree.len(), 0);
        }
    }

    #[test]
    fn len_tracks_insertions(
        pts in proptest::collection::vec(
            (proptest::num::f64::NORMAL, proptest::num::f64::NORMAL, proptest::num::f64::NORMAL),
            0..25
        )
    ) {
        let bounds = Bounds::new(Vec3::new(-1e9, -1e9, -1e9), Vec3::new(1e9, 1e9, 1e9));
        let mut tree = Octree::new(bounds);
        prop_assert_eq!(tree.len(), 0);
        let mut count = 0;
        for &(x, y, z) in pts.iter() {
            let pt = Vec3::new(x, y, z);
            if bounds.contains(pt) {
                tree.insert(pt, count as i32);
                count += 1;
            }
        }
        prop_assert_eq!(tree.len(), count);
    }
}

#[test]
fn duplicate_points_are_stored() {
    let bounds = Bounds::new(Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0));
    let mut tree = Octree::new(bounds);
    let pt = Vec3::new(1.0, 2.0, 3.0);
    assert!(tree.insert(pt, 1));
    assert!(tree.insert(pt, 2));
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.query_range(&Bounds::new(pt, pt)).len(), 2);
}

#[test]
fn zero_volume_query_at_point() {
    let bounds = Bounds::new(
        Vec3::new(-100.0, -100.0, -100.0),
        Vec3::new(100.0, 100.0, 100.0),
    );
    let mut tree = Octree::new(bounds);
    tree.insert(Vec3::new(5.0, -3.0, 0.0), 99);
    let miss = Vec3::new(5.0, -3.0, 0.001);
    assert!(tree.query_range(&Bounds::new(miss, miss)).is_empty());
}
