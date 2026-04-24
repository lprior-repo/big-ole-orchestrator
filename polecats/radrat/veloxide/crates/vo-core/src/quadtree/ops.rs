use crate::quadtree::types::{Point, QuadtreeError, AABB};

pub(crate) enum Node {
    Leaf { points: Vec<Point> },
    Branch { children: Box<[Node; 4]> },
}

pub(crate) fn quadrant_index(bounds: &[AABB; 4], x: f64, y: f64) -> usize {
    for (i, b) in bounds.iter().enumerate() {
        if b.contains_point(x, y) {
            return i;
        }
    }
    3
}

pub(crate) fn insert_node(
    node: &mut Node,
    bounds: &AABB,
    point: Point,
    depth: usize,
    capacity: usize,
    max_depth: usize,
) -> Result<(), QuadtreeError> {
    match node {
        Node::Leaf { points } => {
            if points.len() < capacity || depth >= max_depth {
                points.push(point);
                Ok(())
            } else {
                let existing: Vec<Point> = std::mem::take(points);
                let child_bounds = bounds.subdivide();
                let mut children = Box::new([
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                    Node::Leaf { points: Vec::new() },
                ]);
                for p in existing {
                    let qi = quadrant_index(&child_bounds, p.x, p.y);
                    insert_node(
                        &mut children[qi],
                        &child_bounds[qi],
                        p,
                        depth + 1,
                        capacity,
                        max_depth,
                    )?;
                }
                let qi = quadrant_index(&child_bounds, point.x, point.y);
                insert_node(
                    &mut children[qi],
                    &child_bounds[qi],
                    point,
                    depth + 1,
                    capacity,
                    max_depth,
                )?;
                *node = Node::Branch { children };
                Ok(())
            }
        }
        Node::Branch { children } => {
            let child_bounds = bounds.subdivide();
            let qi = quadrant_index(&child_bounds, point.x, point.y);
            insert_node(
                &mut children[qi],
                &child_bounds[qi],
                point,
                depth + 1,
                capacity,
                max_depth,
            )
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
                if region.contains_point(p.x, p.y) {
                    result.push(p.clone());
                }
            }
        }
        Node::Branch { children } => {
            let child_bounds = bounds.subdivide();
            for i in 0..4 {
                query_node(&children[i], &child_bounds[i], region, result);
            }
        }
    }
}

pub(crate) fn count_node(node: &Node) -> usize {
    match node {
        Node::Leaf { points } => points.len(),
        Node::Branch { children } => children.iter().map(count_node).sum(),
    }
}
