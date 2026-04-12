#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod plugin_id_tests {
    use crate::plugin::{InstanceKey, PluginId, PluginName, PluginVersion};

    fn make_plugin_id(name: &str) -> PluginId {
        let name = PluginName::new(name).unwrap();
        let version = PluginVersion::new(1, 0, 0);
        let instance_key = InstanceKey::new();
        PluginId::new(name, version, instance_key)
    }

    #[test]
    fn plugin_id_constructs_with_all_fields() {
        let id = make_plugin_id("merge-resolver");
        assert_eq!(id.name().as_str(), "merge-resolver");
        assert_eq!(id.version().major(), 1);
        assert_eq!(id.version().minor(), 0);
        assert_eq!(id.version().patch(), 0);
    }

    #[test]
    fn plugin_id_equality_reflexive() {
        let name = PluginName::new("test-plugin").unwrap();
        let version = PluginVersion::new(1, 0, 0);
        let key = InstanceKey::new();
        let id1 = PluginId::new(name.clone(), version, key.clone());
        let id2 = PluginId::new(name, version, key);
        assert_eq!(id1, id2);
    }

    #[test]
    fn plugin_id_inequality_different_instance_key() {
        let name = PluginName::new("test-plugin").unwrap();
        let version = PluginVersion::new(1, 0, 0);
        let id1 = PluginId::new(name.clone(), version, InstanceKey::new());
        let id2 = PluginId::new(name, version, InstanceKey::new());
        assert_ne!(id1, id2);
    }

    #[test]
    fn plugin_id_inequality_different_name() {
        let version = PluginVersion::new(1, 0, 0);
        let key = InstanceKey::new();
        let id1 = PluginId::new(PluginName::new("plugin-a").unwrap(), version, key.clone());
        let id2 = PluginId::new(PluginName::new("plugin-b").unwrap(), version, key);
        assert_ne!(id1, id2);
    }

    #[test]
    fn plugin_id_inequality_different_version() {
        let name = PluginName::new("test-plugin").unwrap();
        let key = InstanceKey::new();
        let id1 = PluginId::new(name.clone(), PluginVersion::new(1, 0, 0), key.clone());
        let id2 = PluginId::new(name, PluginVersion::new(2, 0, 0), key);
        assert_ne!(id1, id2);
    }

    #[test]
    fn plugin_id_display_includes_name_version_and_key() {
        let id = make_plugin_id("blob-connector");
        let display = format!("{id}");
        assert!(display.starts_with("blob-connector@"));
        assert!(display.contains("1.0.0#"));
    }

    #[test]
    fn plugin_id_instance_key_returns_key() {
        let name = PluginName::new("test").unwrap();
        let version = PluginVersion::new(1, 0, 0);
        let key = InstanceKey::new();
        let id = PluginId::new(name, version, key.clone());
        assert_eq!(id.instance_key().0, key.0);
    }

    #[test]
    fn plugin_id_serializes_to_json_with_all_fields() {
        let id = make_plugin_id("merge-resolver");
        let json = serde_json::to_value(&id).expect("serialization should succeed");
        assert!(json.get("name").is_some());
        assert!(json.get("version").is_some());
        assert!(json.get("instance_key").is_some());
    }

    #[test]
    fn plugin_id_json_roundtrip_preserves_fields() {
        let id = make_plugin_id("roundtrip-test");
        let json = serde_json::to_string(&id).expect("serialize");
        let restored: PluginId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id.name().as_str(), restored.name().as_str());
        assert_eq!(id.version(), restored.version());
    }
}
