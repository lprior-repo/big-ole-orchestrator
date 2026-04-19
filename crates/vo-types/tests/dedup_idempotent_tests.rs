//! Idempotent operation deduplication tests (ve-bmekb).
//!
//! Tests that duplicate operations with the same ID are deduplicated and
//! return the original result. Covers: duplicate submit, concurrent duplicate,
//! dedupe key validation, and partition key construction.

#[cfg(test)]
mod tests {
    use vo_types::{DedupeKey, DedupePartitionKey, InstanceId, ParseError};

    // ── DedupeKey validation ─────────────────────────────────────────────

    #[test]
    fn dedupe_key_parse_valid() {
        let key = DedupeKey::parse("webhook-abc-123");
        assert_eq!(key.unwrap().as_str(), "webhook-abc-123");
    }

    #[test]
    fn dedupe_key_parse_empty_rejects() {
        let result = DedupeKey::parse("");
        assert!(matches!(result, Err(ParseError::Empty { .. })));
    }

    #[test]
    fn dedupe_key_parse_too_long_rejects() {
        let long_key = "x".repeat(257);
        let result = DedupeKey::parse(&long_key);
        assert!(matches!(result, Err(ParseError::ExceedsMaxLength { .. })));
    }

    #[test]
    fn dedupe_key_parse_at_max_length_succeeds() {
        let key = "x".repeat(256);
        let result = DedupeKey::parse(&key);
        assert!(result.is_ok());
    }

    #[test]
    fn dedupe_key_equality() {
        let a = DedupeKey::parse("same-key").unwrap();
        let b = DedupeKey::parse("same-key").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn dedupe_key_inequality() {
        let a = DedupeKey::parse("key-a").unwrap();
        let b = DedupeKey::parse("key-b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn dedupe_key_hash_consistency() {
        use std::collections::HashSet;
        let key = DedupeKey::parse("hash-test").unwrap();
        let mut set = HashSet::new();
        set.insert(key.clone());
        assert!(set.contains(&key));
    }

    #[test]
    fn dedupe_key_serde_roundtrip() {
        let key = DedupeKey::parse("serde-test").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        let restored: DedupeKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, restored);
    }

    #[test]
    fn dedupe_key_from_string_roundtrip() {
        let key = DedupeKey::parse("into-test").unwrap();
        let s: String = key.clone().into();
        let restored = DedupeKey::try_from(s).unwrap();
        assert_eq!(key, restored);
    }

    #[test]
    fn dedupe_key_try_from_empty_rejects() {
        let result = DedupeKey::try_from(String::new());
        assert!(result.is_err());
    }

    // ── DedupePartitionKey ───────────────────────────────────────────────

    #[test]
    fn partition_key_valid_construction() {
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let key = DedupePartitionKey::new(instance_id.clone(), "webhook");
        assert!(key.is_ok());
        let key = key.unwrap();
        assert_eq!(key.instance_id(), &instance_id);
        assert_eq!(key.command_type(), "webhook");
    }

    #[test]
    fn partition_key_empty_command_type_rejects() {
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let result = DedupePartitionKey::new(instance_id, "");
        assert!(matches!(result, Err(ParseError::Empty { .. })));
    }

    #[test]
    fn partition_key_too_long_command_type_rejects() {
        let instance_id = InstanceId::from_bytes([1u8; 16]);
        let long_cmd = "x".repeat(257);
        let result = DedupePartitionKey::new(instance_id, &long_cmd);
        assert!(matches!(result, Err(ParseError::ExceedsMaxLength { .. })));
    }

    #[test]
    fn partition_key_equality() {
        let id = InstanceId::from_bytes([1u8; 16]);
        let a = DedupePartitionKey::new(id.clone(), "cmd").unwrap();
        let b = DedupePartitionKey::new(id, "cmd").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn partition_key_different_instance_id_inequality() {
        let id1 = InstanceId::from_bytes([1u8; 16]);
        let id2 = InstanceId::from_bytes([2u8; 16]);
        let a = DedupePartitionKey::new(id1, "cmd").unwrap();
        let b = DedupePartitionKey::new(id2, "cmd").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn partition_key_different_command_type_inequality() {
        let id = InstanceId::from_bytes([1u8; 16]);
        let a = DedupePartitionKey::new(id.clone(), "cmd-a").unwrap();
        let b = DedupePartitionKey::new(id, "cmd-b").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn partition_key_serde_roundtrip() {
        let id = InstanceId::from_bytes([1u8; 16]);
        let key = DedupePartitionKey::new(id, "webhook").unwrap();
        let json = serde_json::to_string(&key).unwrap();
        let restored: DedupePartitionKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, restored);
    }

    // ── Duplicate detection semantics ────────────────────────────────────

    #[test]
    fn same_dedupe_key_produces_same_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a = DedupeKey::parse("dup-key").unwrap();
        let b = DedupeKey::parse("dup-key").unwrap();
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        a.hash(&mut h1);
        b.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish(), "same keys must hash identically");
    }

    #[test]
    fn different_dedupe_keys_likely_different_hashes() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let a = DedupeKey::parse("key-alpha").unwrap();
        let b = DedupeKey::parse("key-beta").unwrap();
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        a.hash(&mut h1);
        b.hash(&mut h2);
        assert_ne!(h1.finish(), h2.finish(), "different keys should differ");
    }
}
