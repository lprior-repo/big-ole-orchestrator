use crate::kdtree::types::{KdtreeError, Point, AABB};

pub(crate) enum Node {
    Leaf {
        points: Vec<Point>,
    },
    Branch {
        dim: usize,
        split_val: f64,
        left: Box<Node>,
        right: Box<Node>,
    },
}

pub(crate) fn insert_node(
    node: &mut Node,
    bounds: &AABB,
    point: Point,
    depth: usize,
    capacity: usize,
    max_depth: usize,
) -> Result<(), KdtreeError> {
    let splitting_dim = depth % bounds.dimension();

    match node {
        Node::Leaf { points } => {
            if points.len() < capacity || depth >= max_depth {
                points.push(point);
                Ok(())
            } else {
                let existing: Vec<Point> = std::mem::take(points);
                let mid_val = existing
                    .iter()
                    .map(|p| p.get_coord(splitting_dim))
                    .sum::<f64>()
                    / existing.len() as f64;

                let mut children = Box::new((
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                ));

                for p in existing {
                    let child_idx = if p.get_coord(splitting_dim) < mid_val {
                        0
                    } else {
                        1
                    };
                    let (left_bounds, right_bounds) = bounds.split(splitting_dim, mid_val);
                    let target_bounds = if child_idx == 0 {
                        &left_bounds
                    } else {
                        &right_bounds
                    };
                    insert_node(
                        &mut children[child_idx],
                        target_bounds,
                        p,
                        depth + 1,
                        capacity,
                        max_depth,
                    )?;
                }

                let (left_bounds, right_bounds) = bounds.split(splitting_dim, mid_val);
                let child_idx = if point.get_coord(splitting_dim) < mid_val {
                    0
                } else {
                    1
                };
                let target_bounds = if child_idx == 0 {
                    &left_bounds
                } else {
                    &right_bounds
                };
                insert_node(
                    &mut children[child_idx],
                    target_bounds,
                    point,
                    depth + 1,
                    capacity,
                    max_depth,
                )?;

                *node = Node::Branch {
                    dim: splitting_dim,
                    split_val: mid_val,
                    left: children.0,
                    right: children.1,
                };
                Ok(())
            }
        }
        Node::Branch {
            dim,
            split_val,
            left,
            right,
        } => {
            let (left_bounds, right_bounds) = bounds.split(*dim, *split_val);
            if point.get_coord(*dim) < *split_val {
                insert_node(left, &left_bounds, point, depth + 1, capacity, max_depth)
            } else {
                insert_node(right, &right_bounds, point, depth + 1, capacity, max_depth)
            }
        }
    }
}

pub(crate) fn query_node(node: &Node, bounds: &AABB, region: &AABB, result: &mut Vec<Point>) {
    if !bounds.intersects(region) {
        return;
    }

    match node {
        Node::Leaf { points } => {
            for p in points {
                if region.contains_point(p) {
                    result.push(p.clone());
                }
            }
        }
        Node::Branch {
            dim,
            split_val,
            left,
            right,
        } => {
            let (left_bounds, right_bounds) = bounds.split(*dim, *split_val);
            query_node(left, &left_bounds, region, result);
            query_node(right, &right_bounds, region, result);
        }
    }
}

pub(crate) fn count_node(node: &Node) -> usize {
    match node {
        Node::Leaf { points } => points.len(),
        Node::Branch { left, right, .. } => count_node(left) + count_node(right),
    }
}
