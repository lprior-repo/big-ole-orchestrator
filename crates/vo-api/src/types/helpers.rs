use itertools::Itertools;

#[must_use]
pub fn is_retryable_error(error: &str) -> bool {
    matches!(
        error,
        "at_capacity" | "internal_error" | "timeout" | "service_unavailable" | "rate_limited"
    )
}

#[must_use]
pub fn is_sorted<T: PartialOrd + Clone>(iter: impl Iterator<Item = T>) -> bool {
    iter.tuple_windows().all(|(prev, curr)| prev <= curr)
}
