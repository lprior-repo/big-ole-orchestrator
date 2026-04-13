use itertools::Itertools;

#[must_use]
pub fn is_retryable_error(error: &str) -> bool {
    matches!(error, "at_capacity")
}

#[must_use]
pub fn is_sorted<T: PartialOrd + Clone>(iter: impl Iterator<Item = T>) -> bool {
    iter.tuple_windows().all(|(prev, curr)| prev <= curr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_retryable_error_at_capacity() {
        assert!(is_retryable_error("at_capacity"));
    }

    #[test]
    fn is_retryable_error_other_errors() {
        assert!(!is_retryable_error("internal_error"));
        assert!(!is_retryable_error(""));
        assert!(!is_retryable_error("timeout"));
        assert!(!is_retryable_error("AT_CAPACITY"));
    }

    #[test]
    fn is_sorted_empty() {
        let v: Vec<u32> = vec![];
        assert!(is_sorted(v.into_iter()));
    }

    #[test]
    fn is_sorted_single() {
        assert!(is_sorted(vec![42].into_iter()));
    }

    #[test]
    fn is_sorted_ascending() {
        assert!(is_sorted(vec![1, 2, 3, 4, 5].into_iter()));
    }

    #[test]
    fn is_sorted_equal_elements() {
        assert!(is_sorted(vec![3, 3, 3].into_iter()));
    }

    #[test]
    fn is_sorted_descending() {
        assert!(!is_sorted(vec![5, 4, 3, 2, 1].into_iter()));
    }

    #[test]
    fn is_sorted_partially_sorted() {
        assert!(!is_sorted(vec![1, 3, 2, 4].into_iter()));
    }
}
