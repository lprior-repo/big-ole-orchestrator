//! Cuckoo Filter implementation for probabilistic deduplication.
//!
//! A Cuckoo filter is a probabilistic data structure that supports:
//! - Insert: O(1) average time complexity
//! - Lookup: O(1) average time complexity
//! - Delete: O(1) average time complexity
//!
//! Compared to Bloom filters, Cuckoo filters provide:
//! - Deletion support (Bloom filters cannot delete)
//! - Higher space efficiency for similar false positive rates
//! - Better cache locality
//!
//! # Usage
//!
//! ```
//! use vo_types::cuckoo::{CuckooFilter, CuckooFilterItem};
//!
//! let mut filter = CuckooFilter::new();
//! let item = CuckooFilterItem::parse("my-item").expect("valid item");
//! filter.insert(&item);
//! assert!(filter.contains(&item));
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::ParseError;

const MAX_CUCKOO_ITEM_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CuckooFilterItem(pub(crate) String);

impl CuckooFilterItem {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "CuckooFilterItem";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.chars().count() > MAX_CUCKOO_ITEM_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_CUCKOO_ITEM_LEN,
                actual: input.chars().count(),
            });
        }
        Ok(Self(input.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CuckooFilterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CuckooFilterItem {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CuckooFilterItem> for String {
    fn from(value: CuckooFilterItem) -> String {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuckooFilter {
    #[serde(skip)]
    inner: cuckoofilter::CuckooFilter,
    capacity: usize,
}

impl CuckooFilter {
    pub fn new() -> Self {
        Self::with_capacity(100_000)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: cuckoofilter::CuckooFilter::with_capacity(capacity),
            capacity,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn contains(&self, item: &CuckooFilterItem) -> bool {
        self.inner.contains(item.as_str().as_bytes())
    }

    pub fn insert(&mut self, item: &CuckooFilterItem) -> bool {
        self.inner.insert(item.as_str().as_bytes()).is_ok()
    }

    pub fn delete(&mut self, item: &CuckooFilterItem) -> bool {
        self.inner.delete(item.as_str().as_bytes())
    }

    pub fn false_positive_rate(&self) -> f64 {
        let total = self.capacity as f64;
        let load = self.len() as f64;
        if total == 0.0 {
            return 0.0;
        }
        load / total
    }
}

impl Default for CuckooFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CuckooFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CuckooFilter {{ capacity: {}, len: {}, fpr: {:.4} }}",
            self.capacity,
            self.len(),
            self.false_positive_rate()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuckoo_filter_item_parses_valid_input() {
        let item = CuckooFilterItem::parse("test-item").expect("valid item");
        assert_eq!(item.as_str(), "test-item");
    }

    #[test]
    fn cuckoo_filter_item_parses_unicode() {
        let item =
            CuckooFilterItem::parse("item-日本語-123").expect("valid unicode item");
        assert_eq!(item.as_str(), "item-日本語-123");
    }

    #[test]
    fn cuckoo_filter_item_rejects_empty() {
        let result = CuckooFilterItem::parse("");
        assert!(matches!(result, Err(ParseError::Empty { .. })));
    }

    #[test]
    fn cuckoo_filter_item_rejects_over_max_length() {
        let long_input = "a".repeat(MAX_CUCKOO_ITEM_LEN + 1);
        let result = CuckooFilterItem::parse(&long_input);
        assert!(matches!(
            result,
            Err(ParseError::ExceedsMaxLength { actual, .. }) if actual == MAX_CUCKOO_ITEM_LEN + 1
        ));
    }

    #[test]
    fn cuckoo_filter_item_accepts_max_length() {
        let max_input = "x".repeat(MAX_CUCKOO_ITEM_LEN);
        let item = CuckooFilterItem::parse(&max_input).expect("valid max length item");
        assert_eq!(item.as_str(), max_input);
    }

    #[test]
    fn cuckoo_filter_starts_empty() {
        let filter = CuckooFilter::new();
        assert!(filter.is_empty());
        assert_eq!(filter.len(), 0);
    }

    #[test]
    fn cuckoo_filter_insert_and_contains() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("test-item").expect("valid item");

        assert!(!filter.contains(&item));
        assert!(filter.insert(&item));
        assert!(filter.contains(&item));
    }

    #[test]
    fn cuckoo_filter_insert_duplicate_returns_false() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("dup-item").expect("valid item");

        assert!(filter.insert(&item));
        assert!(!filter.insert(&item));
    }

    #[test]
    fn cuckoo_filter_delete_existing_item() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("delete-me").expect("valid item");

        filter.insert(&item);
        assert!(filter.contains(&item));
        assert!(filter.delete(&item));
        assert!(!filter.contains(&item));
    }

    #[test]
    fn cuckoo_filter_delete_nonexistent_returns_false() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("not-inserted").expect("valid item");

        assert!(!filter.delete(&item));
    }

    #[test]
    fn cuckoo_filter_delete_already_deleted_returns_false() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("delete-twice").expect("valid item");

        filter.insert(&item);
        filter.delete(&item);
        assert!(!filter.delete(&item));
    }

    #[test]
    fn cuckoo_filter_multiple_items() {
        let mut filter = CuckooFilter::new();
        let items: Vec<CuckooFilterItem> = (0..100)
            .map(|i| CuckooFilterItem::parse(&format!("item-{}", i)).expect("valid")
            .collect();

        for item in &items {
            assert!(!filter.contains(item));
            filter.insert(item);
        }

        assert_eq!(filter.len(), 100);

        for item in &items {
            assert!(filter.contains(item));
        }
    }

    #[test]
    fn cuckoo_filter_with_capacity() {
        let filter = CuckooFilter::with_capacity(500);
        assert_eq!(filter.capacity(), 500);
    }

    #[test]
    fn cuckoo_filter_serde_roundtrip() {
        let mut filter = CuckooFilter::new();
        let item = CuckooFilterItem::parse("serde-test").expect("valid item");
        filter.insert(&item);

        let json = serde_json::to_string(&filter).expect("serialize");
        let recovered: CuckooFilter = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(recovered.len(), 1);
        assert!(recovered.contains(&item));
    }

    #[test]
    fn cuckoo_filter_display_shows_stats() {
        let mut filter = CuckooFilter::with_capacity(1000);
        let item = CuckooFilterItem::parse("display-test").expect("valid item");
        filter.insert(&item);

        let display = format!("{}", filter);
        assert!(display.contains("capacity: 1000"));
        assert!(display.contains("len: 1"));
    }

    #[test]
    fn cuckoo_filter_false_positive_rate_calculation() {
        let mut filter = CuckooFilter::with_capacity(1000);

        assert_eq!(filter.false_positive_rate(), 0.0);

        for i in 0..500 {
            let item = CuckooFilterItem::parse(&format!("item-{}", i)).expect("valid");
            filter.insert(&item);
        }

        let fpr = filter.false_positive_rate();
        assert!(fpr > 0.0);
        assert!(fpr < 1.0);
        assert!((fpr - 0.5).abs() < 0.01);
    }

    #[test]
    fn cuckoo_filter_equality() {
        let mut filter1 = CuckooFilter::new();
        let mut filter2 = CuckooFilter::new();
        let item = CuckooFilterItem::parse("equal-item").expect("valid item");

        filter1.insert(&item);
        filter2.insert(&item);

        assert_eq!(filter1, filter2);

        let mut filter3 = CuckooFilter::new();
        assert_ne!(filter1, filter3);
    }
}