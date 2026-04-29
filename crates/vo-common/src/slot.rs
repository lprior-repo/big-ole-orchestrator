//! Slot-based value model for zero-allocation workflow execution.
//!
//! Variables are pre-allocated as fixed array indices (`SlotIdx`) at compile time.
//! At runtime, the VM reads/writes values directly into a `Vec<SlotValue>` indexed
//! by `SlotIdx` — no HashMap lookups, no string matching, no heap allocation on
//! the hot path for primitive types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of slots per workflow (u16::MAX).
pub const MAX_SLOTS: u16 = u16::MAX;

/// Compile-time index into the slot array.
///
/// Each named variable in a workflow gets a unique `SlotIdx` assigned by
/// `SlotAllocator` during compilation. At runtime, state machines index
/// directly into a flat `Vec<SlotValue>` using this index — no string lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SlotIdx(pub u16);

impl SlotIdx {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// A value stored in a workflow slot.
///
/// Designed for minimal branching on the hot path. Primitive variants (Null,
/// Bool, I64, F64) are stack-local. Heap variants (Str, Bytes, List, Map)
/// only allocate when actually used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum SlotValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<SlotValue>),
    Map(Vec<(String, SlotValue)>),
    Timestamp(u64),
}

impl SlotValue {
    /// Returns `true` if the value is `Null`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns `true` if the value is a boolean.
    #[must_use]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Extract the bool value, if this is a `Bool`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    /// Extract the i64 value, if this is an `I64`.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::I64(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    /// Extract the f64 value, if this is an `F64`.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::F64(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    /// Extract the str value, if this is a `Str`.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        if let Self::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }

    /// Extract the bytes value, if this is `Bytes`.
    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let Self::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }

    /// Extract the list value, if this is a `List`.
    #[must_use]
    pub fn as_list(&self) -> Option<&[SlotValue]> {
        if let Self::List(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Extract the timestamp millis, if this is a `Timestamp`.
    #[must_use]
    pub fn as_timestamp(&self) -> Option<u64> {
        if let Self::Timestamp(ts) = self {
            Some(*ts)
        } else {
            None
        }
    }

    /// Look up a key in a `Map` value. Returns `None` if not a Map or key absent.
    #[must_use]
    pub fn map_get(&self, key: &str) -> Option<&SlotValue> {
        if let Self::Map(entries) = self {
            entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
}

impl Default for SlotValue {
    fn default() -> Self {
        Self::Null
    }
}

impl From<bool> for SlotValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for SlotValue {
    fn from(n: i64) -> Self {
        Self::I64(n)
    }
}

impl From<f64> for SlotValue {
    fn from(n: f64) -> Self {
        Self::F64(n)
    }
}

impl From<String> for SlotValue {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&str> for SlotValue {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

impl From<Vec<u8>> for SlotValue {
    fn from(b: Vec<u8>) -> Self {
        Self::Bytes(b)
    }
}

impl From<u64> for SlotValue {
    fn from(ts: u64) -> Self {
        Self::Timestamp(ts)
    }
}

/// Compile-time allocator that maps variable names to slot indices.
///
/// Used during workflow compilation. Each named variable gets a unique `SlotIdx`.
/// The allocator enforces the `MAX_SLOTS` limit and produces a flat slot array
/// layout for the runtime.
#[derive(Debug, Clone)]
pub struct SlotAllocator {
    /// Variable name → assigned SlotIdx
    slots: HashMap<String, SlotIdx>,
    /// SlotIdx → variable name (for debugging/reflection)
    names: Vec<String>,
    /// Next available index
    next: u16,
}

/// Error from slot allocation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SlotAllocError {
    /// No more slot indices available (exceeded u16::MAX).
    #[error("slot limit exceeded: cannot allocate more than {limit} slots")]
    LimitExceeded { limit: u16 },
    /// A variable with this name was already allocated.
    #[error("duplicate slot name: {name}")]
    DuplicateName { name: String },
}

impl SlotAllocator {
    /// Create a new allocator with no slots assigned.
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            names: Vec::new(),
            next: 0,
        }
    }

    /// Allocate a slot for a named variable.
    ///
    /// Returns the assigned `SlotIdx`. Errors on duplicate names or if
    /// the slot limit is exceeded.
    pub fn alloc(&mut self, name: impl Into<String>) -> Result<SlotIdx, SlotAllocError> {
        let name = name.into();
        if self.slots.contains_key(&name) {
            return Err(SlotAllocError::DuplicateName { name });
        }
        if self.next == MAX_SLOTS {
            return Err(SlotAllocError::LimitExceeded { limit: MAX_SLOTS });
        }
        let idx = SlotIdx(self.next);
        self.next += 1;
        self.slots.insert(name.clone(), idx);
        self.names.push(name);
        Ok(idx)
    }

    /// Look up the slot index for a previously allocated variable.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<SlotIdx> {
        self.slots.get(name).copied()
    }

    /// Look up the variable name for a slot index.
    #[must_use]
    pub fn name_of(&self, idx: SlotIdx) -> Option<&str> {
        self.names.get(idx.as_usize()).map(|s| s.as_str())
    }

    /// Number of allocated slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns `true` if no slots have been allocated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Create a slot array pre-filled with `Null` values sized for this allocator.
    #[must_use]
    pub fn create_slot_array(&self) -> Vec<SlotValue> {
        vec![SlotValue::Null; self.len()]
    }

    /// Iterate over all (name, SlotIdx) pairs in allocation order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, SlotIdx)> {
        self.names.iter().map(move |name| {
            let idx = self.slots[name];
            (name.as_str(), idx)
        })
    }
}

impl Default for SlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_idx_new_and_accessors() {
        let idx = SlotIdx::new(42);
        assert_eq!(idx.as_u16(), 42);
        assert_eq!(idx.as_usize(), 42);
    }

    #[test]
    fn slot_idx_ordering() {
        let a = SlotIdx::new(1);
        let b = SlotIdx::new(2);
        assert!(a < b);
        assert_eq!(a, SlotIdx::new(1));
    }

    #[test]
    fn slot_value_null_default() {
        assert_eq!(SlotValue::default(), SlotValue::Null);
        assert!(SlotValue::Null.is_null());
    }

    #[test]
    fn slot_value_type_checks() {
        assert!(SlotValue::Bool(true).is_bool());
        assert!(!SlotValue::I64(0).is_bool());
    }

    #[test]
    fn slot_value_as_accessors() {
        assert_eq!(SlotValue::Bool(true).as_bool(), Some(true));
        assert_eq!(SlotValue::Bool(false).as_bool(), Some(false));
        assert_eq!(SlotValue::Null.as_bool(), None);

        assert_eq!(SlotValue::I64(-42).as_i64(), Some(-42));
        assert_eq!(SlotValue::F64(3.14).as_f64(), Some(3.14));
        assert_eq!(SlotValue::Str("hello".into()).as_str(), Some("hello"));
        assert_eq!(SlotValue::Bytes(vec![1, 2, 3]).as_bytes(), Some(&[1, 2, 3][..]));
        assert_eq!(SlotValue::Timestamp(999).as_timestamp(), Some(999));
        assert_eq!(SlotValue::Null.as_i64(), None);
    }

    #[test]
    fn slot_value_list_accessor() {
        let list = SlotValue::List(vec![SlotValue::I64(1), SlotValue::I64(2)]);
        assert_eq!(list.as_list(), Some(&[SlotValue::I64(1), SlotValue::I64(2)][..]));
        assert_eq!(SlotValue::Null.as_list(), None);
    }

    #[test]
    fn slot_value_map_get() {
        let map = SlotValue::Map(vec![
            ("key".into(), SlotValue::I64(42)),
            ("other".into(), SlotValue::Bool(true)),
        ]);
        assert_eq!(map.map_get("key"), Some(&SlotValue::I64(42)));
        assert_eq!(map.map_get("other"), Some(&SlotValue::Bool(true)));
        assert_eq!(map.map_get("missing"), None);
        assert_eq!(SlotValue::Null.map_get("key"), None);
    }

    #[test]
    fn slot_value_from_conversions() {
        let v: SlotValue = true.into();
        assert_eq!(v, SlotValue::Bool(true));

        let v: SlotValue = 42i64.into();
        assert_eq!(v, SlotValue::I64(42));

        let v: SlotValue = 3.14f64.into();
        assert_eq!(v, SlotValue::F64(3.14));

        let v: SlotValue = String::from("hello").into();
        assert_eq!(v, SlotValue::Str("hello".into()));

        let v: SlotValue = "world".into();
        assert_eq!(v, SlotValue::Str("world".into()));

        let v: SlotValue = vec![1u8, 2, 3].into();
        assert_eq!(v, SlotValue::Bytes(vec![1, 2, 3]));

        let v: SlotValue = 12345u64.into();
        assert_eq!(v, SlotValue::Timestamp(12345));
    }

    #[test]
    fn slot_value_serde_roundtrip() {
        let values = vec![
            SlotValue::Null,
            SlotValue::Bool(true),
            SlotValue::I64(-42),
            SlotValue::F64(3.14),
            SlotValue::Str("hello".into()),
            SlotValue::Bytes(vec![1, 2, 3]),
            SlotValue::List(vec![SlotValue::I64(1), SlotValue::Bool(false)]),
            SlotValue::Map(vec![("k".into(), SlotValue::I64(1))]),
            SlotValue::Timestamp(999),
        ];
        for value in values {
            let json = serde_json::to_string(&value).unwrap();
            let decoded: SlotValue = serde_json::from_str(&json).unwrap();
            assert_eq!(value, decoded, "roundtrip failed for {json}");
        }
    }

    #[test]
    fn allocator_basic() {
        let mut alloc = SlotAllocator::new();
        let a = alloc.alloc("x").unwrap();
        let b = alloc.alloc("y").unwrap();
        assert_eq!(a, SlotIdx::new(0));
        assert_eq!(b, SlotIdx::new(1));
        assert_eq!(alloc.get("x"), Some(a));
        assert_eq!(alloc.get("y"), Some(b));
        assert_eq!(alloc.get("z"), None);
        assert_eq!(alloc.len(), 2);
    }

    #[test]
    fn allocator_name_lookup() {
        let mut alloc = SlotAllocator::new();
        alloc.alloc("alpha").unwrap();
        alloc.alloc("beta").unwrap();
        assert_eq!(alloc.name_of(SlotIdx::new(0)), Some("alpha"));
        assert_eq!(alloc.name_of(SlotIdx::new(1)), Some("beta"));
        assert_eq!(alloc.name_of(SlotIdx::new(99)), None);
    }

    #[test]
    fn allocator_duplicate_rejected() {
        let mut alloc = SlotAllocator::new();
        alloc.alloc("x").unwrap();
        let err = alloc.alloc("x").unwrap_err();
        assert_eq!(err, SlotAllocError::DuplicateName { name: "x".into() });
    }

    #[test]
    fn allocator_limit_exceeded() {
        let mut alloc = SlotAllocator::new();
        // Fill to just before the limit
        for i in 0..MAX_SLOTS {
            alloc.alloc(format!("s{i}")).unwrap();
        }
        assert_eq!(alloc.len(), MAX_SLOTS as usize);
        let err = alloc.alloc("overflow").unwrap_err();
        assert_eq!(err, SlotAllocError::LimitExceeded { limit: MAX_SLOTS });
    }

    #[test]
    fn allocator_create_slot_array() {
        let mut alloc = SlotAllocator::new();
        alloc.alloc("a").unwrap();
        alloc.alloc("b").unwrap();
        alloc.alloc("c").unwrap();
        let arr = alloc.create_slot_array();
        assert_eq!(arr.len(), 3);
        assert!(arr.iter().all(|v| v.is_null()));
    }

    #[test]
    fn allocator_iter_returns_allocation_order() {
        let mut alloc = SlotAllocator::new();
        alloc.alloc("first").unwrap();
        alloc.alloc("second").unwrap();
        alloc.alloc("third").unwrap();
        let pairs: Vec<_> = alloc.iter().collect();
        assert_eq!(pairs[0], ("first", SlotIdx::new(0)));
        assert_eq!(pairs[1], ("second", SlotIdx::new(1)));
        assert_eq!(pairs[2], ("third", SlotIdx::new(2)));
    }

    #[test]
    fn allocator_default_is_empty() {
        let alloc = SlotAllocator::default();
        assert!(alloc.is_empty());
        assert_eq!(alloc.len(), 0);
    }
}
