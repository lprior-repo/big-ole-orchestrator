//! Tests for NamespaceId
//!
//! These tests verify the NamespaceId type's display format,
//! parsing validation, and core trait implementations.

#[cfg(test)]
mod tests {
    use super::super::{NamespaceId, NamespaceIdParseError};
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    #[test]
    fn namespace_id_new_and_display_roundtrip() {
        let ns = NamespaceId::new("prod".to_string());
        assert_eq!(ns.to_string(), "prod");
    }

    #[test]
    fn namespace_id_parse_valid_string() {
        let ns = NamespaceId::parse("valid-namespace").unwrap();
        assert_eq!(ns.as_str(), "valid-namespace");
    }

    #[test]
    fn namespace_id_parse_empty_string_returns_err() {
        let result = NamespaceId::parse("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), NamespaceIdParseError::Empty);
    }

    #[test]
    fn namespace_id_parse_string_with_slash_returns_err() {
        let result = NamespaceId::parse("ns/prod");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), NamespaceIdParseError::ContainsSlash);
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
        let debug_str = format!("{:?}", ns);
        assert!(debug_str.contains("test-ns"));
    }

    #[test]
    fn namespace_id_hash_works() {
        let ns1 = NamespaceId::new("hash-ns-1");
        let ns2 = NamespaceId::new("hash-ns-2");
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        ns1.hash(&mut hasher);
        ns2.hash(&mut hasher);
        let hash1 = hasher.finish();

        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        ns2.hash(&mut hasher2);
        ns1.hash(&mut hasher2);
        let hash2 = hasher2.finish();

        assert_ne!(hash1, hash2, "Different namespaces should have different hashes");
    }

    #[test]
    fn namespace_id_eq_works() {
        let ns1 = NamespaceId::new("same-ns");
        let ns2 = NamespaceId::new("same-ns");
        let ns3 = NamespaceId::new("different-ns");

        assert_eq!(ns1, ns2);
        assert_ne!(ns1, ns3);
    }

    #[test]
    fn namespace_id_as_hashmap_key() {
        let mut map: HashMap<NamespaceId, &'static str> = HashMap::new();
        map.insert(NamespaceId::new("key1".to_string()), "value1");
        map.insert(NamespaceId::new("key2".to_string()), "value2");

        assert_eq!(map.get(&NamespaceId::new("key1".to_string())), Some(&"value1"));
        assert_eq!(map.get(&NamespaceId::new("key2".to_string())), Some(&"value2"));
        assert_eq!(map.len(), 2);

        let removed = map.remove(&NamespaceId::new("key1".to_string()));
        assert_eq!(removed, Some("value1"));
        assert_eq!(map.len(), 1);
    }
}