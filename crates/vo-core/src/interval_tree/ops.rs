use crate::interval_tree::types::{Interval, IntervalNode, IntervalTreeError};

pub(crate) fn merge_nodes<T: Ord, V>(
    left: Option<Box<IntervalNode<T, V>>>,
    right: Option<Box<IntervalNode<T, V>>>,
) -> Option<Box<IntervalNode<T, V>>> {
    match (left, right) {
        (None, None) => None,
        (Some(l), None) => Some(l),
        (None, Some(r)) => Some(r),
        (Some(mut l), Some(r)) => {
            let mut current = &mut Some(l);
            while current.as_ref().map_or(false, |n| n.right.is_some()) {
                let right_max = current
                    .as_mut()
                    .unwrap()
                    .right
                    .as_mut()
                    .map_or(&current.as_ref().unwrap().interval.end, |n| &n.max_end);
                current.as_mut().unwrap().max_end = std::cmp::max(
                    current.as_ref().unwrap().interval.end.clone(),
                    right_max.clone(),
                );
                current = &mut current.as_mut().unwrap().right;
            }
            *current = Some(r);
            l
        }
    }
}

pub(crate) fn update_max_end<T: Ord, V>(node: &mut Box<IntervalNode<T, V>>) {
    let max_left = node
        .left
        .as_ref()
        .map_or(&node.interval.end, |n| &n.max_end);
    let max_right = node
        .right
        .as_ref()
        .map_or(&node.interval.end, |n| &n.max_end);
    node.max_end = std::cmp::max(
        node.interval.end.clone(),
        std::cmp::max(max_left.clone(), max_right.clone()),
    );
}

pub(crate) fn recalculate_max<T: Ord, V>(node: &mut Box<IntervalNode<T, V>>) {
    let left_max = node
        .left
        .as_mut()
        .map_or(&node.interval.end, |n| &n.max_end);
    let right_max = node
        .right
        .as_mut()
        .map_or(&node.interval.end, |n| &n.max_end);
    node.max_end = std::cmp::max(
        node.interval.end.clone(),
        std::cmp::max(left_max.clone(), right_max.clone()),
    );
}
