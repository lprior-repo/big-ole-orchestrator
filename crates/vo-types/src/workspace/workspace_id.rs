use std::fmt;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

static LAST_ULID: Mutex<Option<Ulid>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkspaceId(Ulid);

impl Serialize for WorkspaceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkspaceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let ulid = Ulid::from_string(&s).map_err(serde::de::Error::custom)?;
        Ok(Self(ulid))
    }
}

impl WorkspaceId {
    #[allow(clippy::expect_used)]
    pub fn generate() -> Self {
        #[allow(clippy::expect_used)]
        let mut last = LAST_ULID.lock().expect("LAST_ULID mutex poisoned");
        let ulid = Ulid::new();
        let ulid = match *last {
            Some(prev) if ulid <= prev => Ulid::from(u128::from(prev) + 1),
            _ => ulid,
        };
        *last = Some(ulid);
        Self(ulid)
    }

    pub fn from_ulid(ulid: Ulid) -> Self {
        Self(ulid)
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ti_001_generate_unique_ids_in_sequence() {
        let mut ids = HashSet::new();
        for _ in 0..100 {
            let id = WorkspaceId::generate();
            assert!(ids.insert(id), "duplicate ID generated");
        }
    }

    #[test]
    fn ti_002_ids_are_time_ordered() {
        let id1 = WorkspaceId::generate();
        let id2 = WorkspaceId::generate();
        assert!(id1 < id2, "ULIDs should be monotonically ordered");
    }

    #[test]
    fn ti_003_serde_roundtrip_preserves_value() {
        let id = WorkspaceId::generate();
        let json = serde_json::to_string(&id).unwrap();
        let restored: WorkspaceId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn ti_005_rapid_generation_must_be_monotonic() {
        let mut ids = vec![];
        for _ in 0..100 {
            ids.push(WorkspaceId::generate());
        }
        for window in ids.windows(2) {
            assert!(window[0] < window[1], "rapid generation must be monotonic");
        }
    }

    #[test]
    fn ti_004_display_format_is_parseable() {
        let id = WorkspaceId::generate();
        let display = id.to_string();
        assert_eq!(display.len(), 26);
        assert!(display.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
