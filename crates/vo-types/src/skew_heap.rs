use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewNode<T> {
    pub value: T,
    pub left: Option<Box<SkewNode<T>>>,
    pub right: Option<Box<SkewNode<T>>>,
}

impl<T> SkewNode<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            left: None,
            right: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkewHeap<T> {
    root: Option<Box<SkewNode<T>>>,
    len: usize,
}

impl<T> Default for SkewHeap<T> {
    fn default() -> Self {
        Self { root: None, len: 0 }
    }
}

impl<T: Ord> SkewHeap<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn find_min(&self) -> Option<&T> {
        self.root.as_ref().map(|n| &n.value)
    }

    fn merge_nodes(
        a: Option<Box<SkewNode<T>>>,
        b: Option<Box<SkewNode<T>>>,
    ) -> Option<Box<SkewNode<T>>> {
        match (a, b) {
            (None, None) => None,
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (Some(mut a), Some(mut b)) => {
                if a.value <= b.value {
                    let right = std::mem::replace(&mut a.right, None);
                    a.right = Self::merge_nodes(right, Some(b));
                    Some(a)
                } else {
                    let right = std::mem::replace(&mut b.right, None);
                    b.right = Self::merge_nodes(right, Some(a));
                    Some(b)
                }
            }
        }
    }

    pub fn insert(&mut self, value: T) {
        let node = Some(Box::new(SkewNode::new(value)));
        self.root = Self::merge_nodes(std::mem::take(&mut self.root), node);
        self.len += 1;
    }

    pub fn merge(&mut self, other: &mut SkewHeap<T>) {
        self.root = Self::merge_nodes(
            std::mem::take(&mut self.root),
            std::mem::take(&mut other.root),
        );
        self.len += other.len;
        other.len = 0;
    }

    pub fn delete_min(&mut self) -> Option<T> {
        let root = self.root.take()?;
        let min_value = root.value;
        self.root = Self::merge_nodes(root.left, root.right);
        self.len -= 1;
        Some(min_value)
    }

    pub fn into_sorted_vec(mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.len);
        while let Some(v) = self.delete_min() {
            result.push(v);
        }
        result
    }
}

impl<T: Ord> Extend<T> for SkewHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<T: Ord> FromIterator<T> for SkewHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut heap = Self::new();
        heap.extend(iter);
        heap
    }
}

impl<T: Ord + Clone> SkewHeap<T> {
    fn verify_heap_property(node: &Option<Box<SkewNode<T>>>) -> bool {
        match node {
            None => true,
            Some(n) => {
                let left_ok = match &n.left {
                    None => true,
                    Some(child) => n.value <= child.value && Self::verify_heap_property(&n.left),
                };
                let right_ok = match &n.right {
                    None => true,
                    Some(child) => n.value <= child.value && Self::verify_heap_property(&n.right),
                };
                left_ok && right_ok
            }
        }
    }

    pub fn is_valid_heap(&self) -> bool {
        Self::verify_heap_property(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_heap_is_empty() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn default_is_empty() {
        let heap: SkewHeap<i32> = SkewHeap::default();
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn insert_increases_len() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(5);
        assert_eq!(heap.len(), 1);
        heap.insert(3);
        assert_eq!(heap.len(), 2);
    }

    #[test]
    fn insert_single_element() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(42);
        assert_eq!(heap.len(), 1);
        assert_eq!(heap.find_min(), Some(&42));
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn find_min_returns_smallest() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.find_min(), Some(&3));
    }

    #[test]
    fn find_min_none_when_empty() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert_eq!(heap.find_min(), None);
    }

    #[test]
    fn delete_min_returns_smallest() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        assert_eq!(heap.delete_min(), Some(3));
        assert_eq!(heap.find_min(), Some(&5));
    }

    #[test]
    fn delete_min_empty_heap() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        assert_eq!(heap.delete_min(), None);
    }

    #[test]
    fn delete_min_single_element() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(42);
        assert_eq!(heap.delete_min(), Some(42));
        assert!(heap.is_empty());
    }

    #[test]
    fn delete_min_decreases_len() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(1);
        heap.insert(2);
        heap.insert(3);
        assert_eq!(heap.len(), 3);
        heap.delete_min();
        assert_eq!(heap.len(), 2);
        heap.delete_min();
        assert_eq!(heap.len(), 1);
        heap.delete_min();
        assert_eq!(heap.len(), 0);
    }

    #[test]
    fn merge_two_heaps() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap2.insert(3);
        heap2.insert(7);

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 4);
        assert_eq!(heap1.find_min(), Some(&1));
        assert_eq!(heap2.len(), 0);
        assert!(heap2.is_empty());
    }

    #[test]
    fn merge_two_heaps_drain_all() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap2.insert(3);
        heap2.insert(7);

        heap1.merge(&mut heap2);

        assert_eq!(heap1.delete_min(), Some(1));
        assert_eq!(heap1.delete_min(), Some(3));
        assert_eq!(heap1.delete_min(), Some(5));
        assert_eq!(heap1.delete_min(), Some(7));
        assert!(heap1.is_empty());
    }

    #[test]
    fn merge_with_empty_heap() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: SkewHeap<i32> = SkewHeap::new();

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 2);
        assert_eq!(heap1.find_min(), Some(&1));
    }

    #[test]
    fn merge_empty_into_nonempty() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();

        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap2.insert(3);

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 1);
        assert_eq!(heap1.find_min(), Some(&3));
        assert_eq!(heap2.len(), 0);
    }

    #[test]
    fn merge_empty_into_empty() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap1.merge(&mut heap2);
        assert!(heap1.is_empty());
        assert_eq!(heap1.len(), 0);
    }

    #[test]
    fn delete_min_maintains_heap_property() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..100).rev() {
            heap.insert(i);
        }
        for i in 0..100 {
            assert_eq!(heap.delete_min(), Some(i));
        }
        assert!(heap.is_empty());
    }

    #[test]
    fn delete_min_maintains_heap_property_reverse_insert() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in 0..100 {
            heap.insert(i);
        }
        for i in 0..100 {
            assert_eq!(heap.delete_min(), Some(i));
        }
        assert!(heap.is_empty());
    }

    #[test]
    fn insert_duplicate_values() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(5);
        heap.insert(5);
        heap.insert(5);
        assert_eq!(heap.len(), 3);
        assert_eq!(heap.find_min(), Some(&5));
        assert_eq!(heap.delete_min(), Some(5));
        assert_eq!(heap.delete_min(), Some(5));
        assert_eq!(heap.delete_min(), Some(5));
        assert!(heap.is_empty());
    }

    #[test]
    fn insert_negative_values() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(-1);
        heap.insert(-5);
        heap.insert(3);
        assert_eq!(heap.find_min(), Some(&-5));
        assert_eq!(heap.delete_min(), Some(-5));
        assert_eq!(heap.delete_min(), Some(-1));
        assert_eq!(heap.delete_min(), Some(3));
    }

    #[test]
    fn insert_large_values() {
        let mut heap: SkewHeap<i64> = SkewHeap::new();
        heap.insert(i64::MAX);
        heap.insert(i64::MIN);
        heap.insert(0);
        assert_eq!(heap.find_min(), Some(&i64::MIN));
        assert_eq!(heap.delete_min(), Some(i64::MIN));
        assert_eq!(heap.delete_min(), Some(0));
        assert_eq!(heap.delete_min(), Some(i64::MAX));
    }

    #[test]
    fn from_iter_creates_heap() {
        let heap: SkewHeap<i32> = vec![5, 3, 7, 1, 9].into_iter().collect();
        assert_eq!(heap.len(), 5);
        assert_eq!(heap.find_min(), Some(&1));
    }

    #[test]
    fn extend_adds_elements() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(1);
        heap.extend(vec![5, 3, 7]);
        assert_eq!(heap.len(), 4);
        assert_eq!(heap.find_min(), Some(&1));
    }

    #[test]
    fn heap_property_after_single_insert() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(42);
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn heap_property_after_many_inserts() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..50).rev() {
            heap.insert(i);
        }
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn heap_property_after_delete_min() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..50).rev() {
            heap.insert(i);
        }
        heap.delete_min();
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn heap_property_after_merge() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        for i in (0..25).rev() {
            heap1.insert(i);
        }
        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        for i in (25..50).rev() {
            heap2.insert(i);
        }
        heap1.merge(&mut heap2);
        assert!(heap1.is_valid_heap());
    }

    #[test]
    fn heap_property_after_interleaved_ops() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..20).rev() {
            heap.insert(i);
        }
        heap.delete_min();
        heap.delete_min();
        heap.insert(-5);
        heap.insert(100);
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn merge_chain_three_heaps() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        heap1.insert(10);
        heap1.insert(1);

        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap2.insert(5);
        heap2.insert(3);

        let mut heap3: SkewHeap<i32> = SkewHeap::new();
        heap3.insert(7);
        heap3.insert(2);

        heap1.merge(&mut heap2);
        heap1.merge(&mut heap3);

        assert_eq!(heap1.len(), 6);
        assert_eq!(heap1.find_min(), Some(&1));
        assert!(heap1.is_valid_heap());

        let mut sorted: Vec<i32> = Vec::new();
        while let Some(v) = heap1.delete_min() {
            sorted.push(v);
        }
        assert_eq!(sorted, vec![1, 2, 3, 5, 7, 10]);
    }

    #[test]
    fn merge_chain_large() {
        let mut merged: SkewHeap<i32> = SkewHeap::new();
        for batch in 0..10 {
            let mut heap: SkewHeap<i32> = SkewHeap::new();
            for i in 0..20 {
                heap.insert(batch * 20 + i);
            }
            merged.merge(&mut heap);
        }
        assert_eq!(merged.len(), 200);
        assert!(merged.is_valid_heap());
        assert_eq!(merged.find_min(), Some(&0));
    }

    #[test]
    fn into_sorted_vec_empty() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        assert_eq!(heap.into_sorted_vec(), Vec::<i32>::new());
    }

    #[test]
    fn into_sorted_vec_single() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(42);
        assert_eq!(heap.into_sorted_vec(), vec![42]);
    }

    #[test]
    fn into_sorted_vec_multiple() {
        let heap: SkewHeap<i32> = vec![5, 3, 7, 1, 9].into_iter().collect();
        assert_eq!(heap.into_sorted_vec(), vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn into_sorted_vec_duplicates() {
        let heap: SkewHeap<i32> = vec![3, 1, 3, 1, 2].into_iter().collect();
        assert_eq!(heap.into_sorted_vec(), vec![1, 1, 2, 3, 3]);
    }

    #[test]
    fn into_sorted_vec_reverse() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..100).rev() {
            heap.insert(i);
        }
        let sorted = heap.into_sorted_vec();
        for (i, &v) in sorted.iter().enumerate() {
            assert_eq!(v, i as i32);
        }
    }

    #[test]
    fn merge_heaps_same_min() {
        let mut heap1: SkewHeap<i32> = SkewHeap::new();
        heap1.insert(1);
        heap1.insert(5);

        let mut heap2: SkewHeap<i32> = SkewHeap::new();
        heap2.insert(1);
        heap2.insert(3);

        heap1.merge(&mut heap2);

        assert_eq!(heap1.len(), 4);
        assert_eq!(heap1.find_min(), Some(&1));
        assert!(heap1.is_valid_heap());
    }

    #[test]
    fn merge_self_not_possible_but_merge_clone() {
        let mut heap: SkewHeap<i32> = vec![1, 2, 3].into_iter().collect();
        let mut clone = heap.clone();
        heap.merge(&mut clone);
        assert_eq!(heap.len(), 6);
        assert!(heap.is_valid_heap());
    }

    #[test]
    fn serde_roundtrip() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(5);
        heap.insert(3);
        heap.insert(7);
        let json = serde_json::to_string(&heap).unwrap();
        let back: SkewHeap<i32> = serde_json::from_str(&json).unwrap();
        assert_eq!(heap.len(), back.len());
        assert_eq!(heap.find_min(), back.find_min());
        assert_eq!(heap.into_sorted_vec(), back.into_sorted_vec());
    }

    #[test]
    fn serde_roundtrip_empty() {
        let heap: SkewHeap<i32> = SkewHeap::new();
        let json = serde_json::to_string(&heap).unwrap();
        let back: SkewHeap<i32> = serde_json::from_str(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn clone_preserves_structure() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..20).rev() {
            heap.insert(i);
        }
        let mut clone = heap.clone();
        assert_eq!(heap.len(), clone.len());
        assert_eq!(heap.find_min(), clone.find_min());
        assert_eq!(heap.delete_min(), clone.delete_min());
        assert!(clone.is_valid_heap());
    }

    #[test]
    fn debug_format() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(1);
        heap.insert(2);
        let debug = format!("{:?}", heap);
        assert!(debug.contains("SkewHeap"));
    }

    #[test]
    fn string_values() {
        let mut heap: SkewHeap<&str> = SkewHeap::new();
        heap.insert("cherry");
        heap.insert("apple");
        heap.insert("banana");
        assert_eq!(heap.find_min(), Some(&"apple"));
        assert_eq!(heap.delete_min(), Some("apple"));
        assert_eq!(heap.delete_min(), Some("banana"));
        assert_eq!(heap.delete_min(), Some("cherry"));
    }

    #[test]
    fn tuple_values() {
        let mut heap: SkewHeap<(i32, &str)> = SkewHeap::new();
        heap.insert((2, "b"));
        heap.insert((1, "a"));
        heap.insert((3, "c"));
        assert_eq!(heap.delete_min(), Some((1, "a")));
        assert_eq!(heap.delete_min(), Some((2, "b")));
        assert_eq!(heap.delete_min(), Some((3, "c")));
    }

    #[test]
    fn stress_sequential_delete_all() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in (0..1000).rev() {
            heap.insert(i);
        }
        for i in 0..1000 {
            assert_eq!(heap.delete_min(), Some(i));
        }
        assert!(heap.is_empty());
    }

    #[test]
    fn stress_interleaved_insert_delete() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        for i in 0..100 {
            heap.insert(i);
            heap.insert(200 - i);
        }
        for _ in 0..50 {
            heap.delete_min();
        }
        for i in 300..400 {
            heap.insert(i);
        }
        let sorted = heap.into_sorted_vec();
        for w in sorted.windows(2) {
            assert!(w[0] <= w[1]);
        }
    }

    #[test]
    fn empty_after_draining_all_elements() {
        let mut heap: SkewHeap<i32> = SkewHeap::new();
        heap.insert(1);
        heap.insert(2);
        heap.delete_min();
        heap.delete_min();
        assert_eq!(heap.delete_min(), None);
        assert!(heap.is_empty());
        assert_eq!(heap.len(), 0);
    }
}

#[cfg(test)]
#[cfg(feature = "proptest")]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_insert_delete_sorted(vals: Vec<i32>) {
            let mut heap: SkewHeap<i32> = SkewHeap::new();
            for &v in &vals {
                heap.insert(v);
            }
            let mut sorted = vals.clone();
            sorted.sort();
            let drained: Vec<i32> = heap.into_sorted_vec();
            prop_assert_eq!(drained, sorted);
        }

        #[test]
        fn proptest_merge_preserves_sorted_order(
            vals1: Vec<i32>,
            vals2: Vec<i32>,
        ) {
            let mut heap1: SkewHeap<i32> = vals1.iter().cloned().collect();
            let mut heap2: SkewHeap<i32> = vals2.iter().cloned().collect();
            let expected_len = heap1.len() + heap2.len();
            heap1.merge(&mut heap2);
            prop_assert_eq!(heap1.len(), expected_len);
            let mut sorted: Vec<i32> = vals1.clone();
            sorted.extend(vals2);
            sorted.sort();
            let drained: Vec<i32> = heap1.into_sorted_vec();
            prop_assert_eq!(drained, sorted);
        }

        #[test]
        fn proptest_heap_property_always_holds(vals: Vec<i32>) {
            let mut heap: SkewHeap<i32> = SkewHeap::new();
            for &v in &vals {
                heap.insert(v);
                prop_assert!(heap.is_valid_heap());
            }
            while !heap.is_empty() {
                heap.delete_min();
                prop_assert!(heap.is_valid_heap());
            }
        }
    }
}
