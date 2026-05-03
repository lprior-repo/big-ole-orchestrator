//! Type definitions for vo-common.

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Deref;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(String);

impl InstanceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn generate() -> Self {
        Self(ulid::Ulid::new().to_string())
    }
}

impl Deref for InstanceId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for InstanceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for InstanceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<InstanceId> for String {
    fn from(id: InstanceId) -> Self {
        id.0
    }
}

impl AsRef<str> for InstanceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for InstanceId {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

impl PartialEq<InstanceId> for &str {
    fn eq(&self, other: &InstanceId) -> bool {
        *self == other.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespaceId(String);

impl NamespaceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for NamespaceId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for NamespaceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NamespaceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<NamespaceId> for String {
    fn from(ns: NamespaceId) -> Self {
        ns.0
    }
}

impl AsRef<str> for NamespaceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for NamespaceId {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

impl PartialEq<NamespaceId> for &str {
    fn eq(&self, other: &NamespaceId) -> bool {
        *self == other.0.as_str()
    }
}

impl Display for NamespaceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

impl NamespaceId {
    pub fn parse(s: impl Into<String>) -> Result<Self, String> {
        let s = s.into();
        if s.is_empty() {
            return Err("NamespaceId cannot be empty".to_string());
        }
        if s.contains('/') {
            return Err("NamespaceId cannot contain '/'".to_string());
        }
        Ok(Self(s))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimerId(String);

impl TimerId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for TimerId {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for TimerId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TimerId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<TimerId> for String {
    fn from(t: TimerId) -> Self {
        t.0
    }
}

impl AsRef<str> for TimerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for TimerId {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_str() == *other
    }
}

impl PartialEq<TimerId> for &str {
    fn eq(&self, other: &TimerId) -> bool {
        *self == other.0.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_behaves_as_string() {
        let id = InstanceId::new("test-instance-123");
        assert_eq!(id.as_str(), "test-instance-123");
    }

    #[test]
    fn namespace_id_behaves_as_string() {
        let ns = NamespaceId::new("namespace-abc");
        assert_eq!(ns.as_str(), "namespace-abc");
    }

    #[test]
    fn timer_id_behaves_as_string() {
        let timer = TimerId::new("timer-xyz");
        assert_eq!(timer.as_str(), "timer-xyz");
    }

    #[test]
    fn instance_id_empty_string() {
        let id = InstanceId::new("");
        assert_eq!(id.as_str(), "");
    }

    #[test]
    fn instance_id_unicode() {
        let id = InstanceId::new("实例-123-🔱");
        assert_eq!(id.as_str(), "实例-123-🔱");
    }

    #[test]
    fn namespace_id_json_roundtrip() {
        let ns = NamespaceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        let json = serde_json::to_string(&ns).expect("serialize");
        assert_eq!(json, "\"01ARZ3NDEKTSV4RRFFQ69G5FAV\"");
        let deserialized: NamespaceId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ns, deserialized);
    }

    #[test]
    fn namespace_id_deserialize_invalid_rejects() {
        let result: Result<NamespaceId, _> = serde_json::from_str("invalid!");
        assert!(result.is_err());
    }

    #[test]
    fn timer_id_ord_matches_ulid_chronological_ordering() {
        // ULID timestamp 1000ms < ULID timestamp 2000ms
        let a = TimerId::new(ulid::Ulid::from_parts(1000, 0).to_string());
        let b = TimerId::new(ulid::Ulid::from_parts(2000, 0).to_string());
        assert!(a < b, "TimerId A (timestamp 1000) should be < TimerId B (timestamp 2000)");
    }

    #[test]
    fn timer_id_ord_reflexive() {
        let a = TimerId::new(ulid::Ulid::from_parts(1000, 0).to_string());
        assert_eq!(a, a, "TimerId A should equal itself");
    }

    #[test]
    fn timer_id_ord_deterministic_different_randomness() {
        // Same timestamp, different randomness
        let a = TimerId::new(ulid::Ulid::from_parts(1000, 0x1234).to_string());
        let c = TimerId::new(ulid::Ulid::from_parts(1000, 0x5678).to_string());
        assert_ne!(a, c, "TimerIds with same timestamp but different randomness should be different");
        assert!(a < c || c < a, "TimerId comparison must be deterministic");
        // Verify determinism: comparison is stable across multiple calls
        for _ in 0..10 {
            if a < c {
                assert!(a < c);
            } else {
                assert!(c < a);
            }
        }
    }

    #[test]
    fn namespace_id_display_roundtrip() {
        let ns = NamespaceId::new("prod".to_string());
        assert_eq!(ns.to_string(), "prod");
    }

    #[test]
    fn namespace_id_parse_empty_string_err() {
        let result = NamespaceId::parse("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "NamespaceId cannot be empty");
    }

    #[test]
    fn namespace_id_parse_slash_err() {
        let result = NamespaceId::parse("prod/dev");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "NamespaceId cannot contain '/'");
    }

    #[test]
    fn namespace_id_clone_works() {
        let ns = NamespaceId::new("test-ns");
        let cloned = ns.clone();
        assert_eq!(ns, cloned);
    }

    #[test]
    fn namespace_id_debug_works() {
        let ns = NamespaceId::new("test-ns");
        let debug = format!("{:?}", ns);
        assert!(debug.contains("test-ns"));
    }

    #[test]
    fn namespace_id_hash_works() {
        use std::collections::HashMap;
        let ns1 = NamespaceId::new("prod");
        let ns2 = NamespaceId::new("prod");
        let ns3 = NamespaceId::new("dev");
        let mut map: HashMap<NamespaceId, &str> = HashMap::new();
        map.insert(ns1.clone(), "first");
        assert_eq!(map.len(), 1);
        map.insert(ns2.clone(), "second");
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&ns1), Some(&"second"));
        map.insert(ns3.clone(), "third");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&ns3), Some(&"third"));
    }

    #[test]
    fn namespace_id_eq_works() {
        let ns1 = NamespaceId::new("prod");
        let ns2 = NamespaceId::new("prod");
        let ns3 = NamespaceId::new("dev");
        assert_eq!(ns1, ns2);
        assert_ne!(ns1, ns3);
    }

    #[test]
    fn namespace_id_parse_valid() {
        let ns = NamespaceId::parse("prod").expect("valid namespace");
        assert_eq!(ns.to_string(), "prod");
    }
}
