use serde::{Deserialize, Serialize};

use crate::string_newtype;
use crate::ParseError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(pub(crate) String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LeaseKey(pub(crate) String);

impl NodeId {
    /// Parse a `NodeId` from a ULID string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty, has invalid length, or contains an invalid ULID.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "NodeId";
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() != 26 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: format!("expected 26 characters, got {}", input.len()),
            });
        }
        let ulid = ulid::Ulid::from_string(input).map_err(|e| ParseError::InvalidFormat {
            type_name: TYPE_NAME,
            reason: format!("invalid ULID: {e}"),
        })?;
        if ulid.0 == 0 {
            return Err(ParseError::InvalidFormat {
                type_name: TYPE_NAME,
                reason: "nil ULID value not permitted".to_string(),
            });
        }
        Ok(Self(ulid.to_string()))
    }

    /// Generate a new random `NodeId`.
    #[must_use]
    pub fn generate() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
string_newtype!(NodeId);

impl LeaseKey {
    /// Parse a `LeaseKey` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ParseError` if the input is empty or exceeds max length.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        const TYPE_NAME: &str = "LeaseKey";
        const MAX_LEN: usize = 256;
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.chars().count() > MAX_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_LEN,
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
string_newtype!(LeaseKey);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_instantiate_and_compare_nodeid_and_leasekey() {
        let node_a = NodeId::generate();
        let node_b = NodeId::generate();
        assert_ne!(node_a, node_b);

        let key_a = LeaseKey::parse("partition-1").unwrap();
        let key_b = LeaseKey::parse("partition-2").unwrap();
        assert_ne!(key_a, key_b);
        assert_eq!(key_a, LeaseKey::parse("partition-1").unwrap());
    }

    #[test]
    fn can_instantiate_and_compare_nodeid_and_leasekey_duplicate_for_schema() {
        let node = NodeId::generate();
        let same = node.clone();
        assert_eq!(node, same);

        let key = LeaseKey::parse("my-partition").unwrap();
        let same_key = LeaseKey::parse("my-partition").unwrap();
        assert_eq!(key, same_key);
    }

    #[test]
    fn construction_of_leasekey_with_empty_string_fails() {
        let result = LeaseKey::parse("");
        assert!(result.is_err());
        let err = result.err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("LeaseKey"));
        assert!(msg.contains("empty"));
    }

    #[test]
    fn construction_of_leasekey_with_empty_string_fails_duplicate_for_schema() {
        assert!(LeaseKey::parse("").is_err());
    }

    #[test]
    fn node_id_parse_rejects_empty() {
        assert!(matches!(
            NodeId::parse(""),
            Err(ParseError::Empty {
                type_name: "NodeId"
            })
        ));
    }

    #[test]
    fn node_id_parse_rejects_invalid_length() {
        assert!(matches!(
            NodeId::parse("short"),
            Err(ParseError::InvalidFormat {
                type_name: "NodeId",
                ..
            })
        ));
    }

    #[test]
    fn node_id_parse_rejects_invalid_ulid() {
        assert!(matches!(
            NodeId::parse("invalid_characters_abc"),
            Err(ParseError::InvalidFormat {
                type_name: "NodeId",
                ..
            })
        ));
    }

    #[test]
    fn node_id_parse_accepts_valid_ulid() {
        let ulid = ulid::Ulid::new();
        let id = NodeId::parse(&ulid.to_string()).unwrap();
        assert_eq!(id.as_str(), ulid.to_string());
    }

    #[test]
    fn node_id_round_trips_via_serde() {
        let node = NodeId::generate();
        let json = serde_json::to_string(&node).unwrap();
        let restored: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(node, restored);
    }

    #[test]
    fn node_id_display_shows_inner_string() {
        let node = NodeId::generate();
        let display = format!("{node}");
        assert_eq!(display, node.as_str());
    }

    #[test]
    fn node_id_try_from_string_delegates_to_parse() {
        let ulid = ulid::Ulid::new();
        let node = NodeId::try_from(ulid.to_string()).unwrap();
        assert_eq!(node.as_str(), ulid.to_string());
    }

    #[test]
    fn node_id_try_from_empty_string_returns_error() {
        assert!(NodeId::try_from(String::new()).is_err());
    }

    #[test]
    fn lease_key_parse_accepts_valid_string() {
        let key = LeaseKey::parse("workers/partition-42").unwrap();
        assert_eq!(key.as_str(), "workers/partition-42");
    }

    #[test]
    fn lease_key_parse_rejects_empty() {
        assert!(matches!(
            LeaseKey::parse(""),
            Err(ParseError::Empty {
                type_name: "LeaseKey"
            })
        ));
    }

    #[test]
    fn lease_key_parse_rejects_exceeds_max_length() {
        let long_input = "a".repeat(257);
        assert!(matches!(
            LeaseKey::parse(&long_input),
            Err(ParseError::ExceedsMaxLength {
                type_name: "LeaseKey",
                max: 256,
                actual: 257
            })
        ));
    }

    #[test]
    fn lease_key_parse_accepts_max_length() {
        let max_input = "a".repeat(256);
        let key = LeaseKey::parse(&max_input).unwrap();
        assert_eq!(key.as_str(), max_input);
    }

    #[test]
    fn lease_key_round_trips_via_serde() {
        let key = LeaseKey::parse("test-lease").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        let restored: LeaseKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, restored);
    }

    #[test]
    fn lease_key_display_shows_inner_string() {
        let key = LeaseKey::parse("my-lease").unwrap();
        let display = format!("{key}");
        assert_eq!(display, "my-lease");
    }

    #[test]
    fn lease_key_try_from_string_delegates_to_parse() {
        let key = LeaseKey::try_from("valid-key".to_string()).unwrap();
        assert_eq!(key.as_str(), "valid-key");
    }

    #[test]
    fn lease_key_try_from_empty_string_returns_error() {
        assert!(LeaseKey::try_from(String::new()).is_err());
    }

    #[test]
    fn lease_key_from_into_string_round_trip() {
        let original = "partition/acme/workflow-1";
        let key = LeaseKey::parse(original).unwrap();
        let back: String = String::from(key);
        assert_eq!(back, original);
    }

    #[test]
    fn node_id_from_into_string_round_trip() {
        let node = NodeId::generate();
        let s: String = String::from(node.clone());
        let restored = NodeId::parse(&s).unwrap();
        assert_eq!(node, restored);
    }

    #[test]
    fn node_id_hash_is_deterministic() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let node = NodeId::generate();
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        node.hash(&mut h1);
        node.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn lease_key_hash_is_deterministic() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let key = LeaseKey::parse("deterministic-key").unwrap();
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        key.hash(&mut h1);
        key.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
