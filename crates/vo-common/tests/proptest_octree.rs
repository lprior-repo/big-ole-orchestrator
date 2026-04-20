//! Proptest suite for an Octree. Self-contained inline implementation.

use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min: Vec3,
    max: Vec3,
}

impl Bounds {
    fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
    fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
}

#[derive(Debug, Clone)]
struct Octree<T: Clone> {
    bounds: Bounds,
    data: Vec<(Vec3, T)>,
    children: Option<Box<[Octree<T>; 8]>>,
}

impl<T: Clone> Octree<T> {
    const CAPACITY: usize = 8;

    fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            data: Vec::new(),
            children: None,
        }
    }

    fn insert(&mut self, point: Vec3, value: T) -> bool {
        if !self.bounds.contains(point) {
            return false;
        }
        if self.children.is_none() && self.data.len() < Self::CAPACITY {
            self.data.push((point, value));
            return true;
        }
        self.subdivide();
        for child in self.children.as_mut().unwrap().iter_mut() {
            if child.insert(point, value) {
                return true;
            }
        }
        false
    }

    fn subdivide(&mut self) {
        if self.children.is_some() {
            return;
        }
        let m = Vec3::new(
            (self.bounds.min.x + self.bounds.max.x) / 2.0,
            (self.bounds.min.y + self.bounds.max.y) / 2.0,
            (self.bounds.min.z + self.bounds.max.z) / 2.0,
        );
        let corners = [
            Vec3::new(self.bounds.min.x, self.bounds.min.y, self.bounds.min.z),
            Vec3::new(m.x, self.bounds.min.y, self.bounds.min.z),
            Vec3::new(self.bounds.min.x, m.y, self.bounds.min.z),
            Vec3::new(m.x, m.y, self.bounds.min.z),
            Vec3::new(self.bounds.min.x, self.bounds.min.y, m.z),
            Vec3::new(m.x, self.bounds.min.y, m.z),
            Vec3::new(self.bounds.min.x, m.y, m.z),
            Vec3::new(m.x, m.y, m.z),
        ];
        let opp = [
            Vec3::new(m.x, m.y, m.z),
            Vec3::new(self.bounds.max.x, m.y, m.z),
            Vec3::new(m.x, self.bounds.max.y, m.z),
            Vec3::new(self.bounds.max.x, self.bounds.max.y, m.z),
            Vec3::new(m.x, m.y, self.bounds.max.z),
            Vec3::new(self.bounds.max.x, m.y, self.bounds.max.z),
            Vec3::new(m.x, self.bounds.max.y, self.bounds.max.z),
            Vec3::new(self.bounds.max.x, self.bounds.max.y, self.bounds.max.z),
        ];
        self.children = Some(Box::new(std::array::from_fn(|i| {
            Octree::new(Bounds::new(corners[i], opp[i]))
        })));
        let drained: Vec<_> = self.data.drain(..).collect();
        for (pt, val) in drained {
            for child in self.children.as_mut().unwrap().iter_mut() {
                if child.insert(pt, val.clone()) {
                    break;
                }
            }
        }
    }

    fn query_range(&self, range: &Bounds) -> Vec<&T> {
        let mut out = Vec::new();
        self.collect_range(range, &mut out);
        out
    }

    fn collect_range<'a>(&'a self, range: &Bounds, out: &mut Vec<&'a T>) {
        let overlaps = self.bounds.min.x <= range.max.x
            && self.bounds.max.x >= range.min.x
            && self.bounds.min.y <= range.max.y
            && self.bounds.max.y >= range.min.y
            && self.bounds.min.z <= range.max.z
            && self.bounds.max.z >= range.min.z;
        if !overlaps {
            return;
        }
        for (pt, val) in &self.data {
            if range.contains(*pt) {
                out.push(val);
            }
        }
        if let Some(children) = &self.children {
            for child in children.iter() {
                child.collect_range(range, out);
            }
        }
    }

    fn len(&self) -> usize {
        let mut n = self.data.len();
        if let Some(children) = &self.children {
            for child in children.iter() {
                n += child.len();
            }
        }
        n
    }
}

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
        for (i, &(x, y, z)) in pts.iter().enumerate() {
            tree.insert(Vec3::new(x, y, z), i as i32);
        }
        prop_assert_eq!(tree.len(), pts.len());
        for (i, &(x, y, z)) in pts.iter().enumerate() {
            let pt = Vec3::new(x, y, z);
            let r = Bounds::new(pt, pt);
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
        for (i, &(x, y, z)) in pts.iter().enumerate() {
            tree.insert(Vec3::new(x, y, z), i as i32);
            prop_assert_eq!(tree.len(), i + 1);
        }
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
