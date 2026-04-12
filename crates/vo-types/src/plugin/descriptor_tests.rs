#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod descriptor_tests {
    use crate::plugin::{
        CapabilityId, InstanceKey, IsolationLevel, PluginDescriptor, PluginId, PluginName,
        PluginVersion, PluginVersionConstraint, ResourceBudget, SchemaVersion, VersionRange,
    };

    fn make_descriptor() -> PluginDescriptor {
        let name = PluginName::new("test-plugin").unwrap();
        let version = PluginVersion::new(1, 0, 0);
        let id = PluginId::new(name, version, InstanceKey::new());
        PluginDescriptor {
            id,
            schema_version: SchemaVersion(1),
            capabilities: vec![CapabilityId::new("merge-resolver")],
            dependencies: vec![],
            resource_requirements: ResourceBudget {
                memory_bytes: 1024,
                cpu_units: 1,
                max_instances: 1,
            },
            isolation_level: IsolationLevel::SharedRuntime,
        }
    }

    #[test]
    fn descriptor_constructs_with_required_fields() {
        let desc = make_descriptor();
        assert_eq!(desc.id.name().as_str(), "test-plugin");
        assert_eq!(desc.capabilities.len(), 1);
        assert_eq!(desc.dependencies.len(), 0);
    }

    #[test]
    fn descriptor_id_returns_plugin_id() {
        let desc = make_descriptor();
        assert_eq!(desc.id.name().as_str(), "test-plugin");
    }

    #[test]
    fn descriptor_capabilities_returns_vec() {
        let desc = make_descriptor();
        assert_eq!(desc.capabilities[0].as_str(), "merge-resolver");
    }

    #[test]
    fn descriptor_dependencies_returns_vec() {
        let dep = PluginVersionConstraint {
            name: PluginName::new("base-lib").unwrap(),
            range: VersionRange(">=1.0.0".to_string()),
        };
        let mut desc = make_descriptor();
        desc.dependencies.push(dep);
        assert_eq!(desc.dependencies.len(), 1);
        assert_eq!(desc.dependencies[0].name.as_str(), "base-lib");
    }

    #[test]
    fn descriptor_isolation_level_returns_level() {
        let desc = make_descriptor();
        assert_eq!(desc.isolation_level, IsolationLevel::SharedRuntime);
    }

    #[test]
    fn descriptor_serializes_to_json() {
        let desc = make_descriptor();
        let json = serde_json::to_value(&desc).expect("serialization");
        assert!(json.get("id").is_some());
        assert!(json.get("schema_version").is_some());
        assert!(json.get("capabilities").is_some());
        assert!(json.get("isolation_level").is_some());
    }

    #[test]
    fn descriptor_json_roundtrip() {
        let desc = make_descriptor();
        let json = serde_json::to_string(&desc).expect("serialize");
        let restored: PluginDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(desc.id.name().as_str(), restored.id.name().as_str());
    }

    #[test]
    fn descriptor_multiple_capabilities() {
        let mut desc = make_descriptor();
        desc.capabilities.push(CapabilityId::new("blob-sink"));
        desc.capabilities
            .push(CapabilityId::new("cache-invalidator"));
        assert_eq!(desc.capabilities.len(), 3);
    }

    #[test]
    fn descriptor_isolation_levels_all_variants() {
        for level in [
            IsolationLevel::SharedRuntime,
            IsolationLevel::IsolatedActor,
            IsolationLevel::Process,
        ] {
            let mut desc = make_descriptor();
            desc.isolation_level = level.clone();
            assert_eq!(desc.isolation_level, level);
        }
    }
}
