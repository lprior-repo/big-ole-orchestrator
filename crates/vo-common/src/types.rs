//! Type definitions for vo-common.

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    fn event_id_behaves_as_string() {
        let eid: EventId = "evt-abc-123".into();
        assert_eq!(eid.len(), 11);
        assert_eq!(eid.as_str(), "evt-abc-123");
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
}
