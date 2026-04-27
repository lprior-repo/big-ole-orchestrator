#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod event_tests {
    use crate::plugin::*;

    fn make_plugin_id(name: &str) -> PluginId {
        PluginId::new(
            PluginName::new(name).unwrap(),
            PluginVersion::new(1, 0, 0),
            InstanceKey::new(),
        )
    }

    fn make_descriptor(name: &str) -> PluginDescriptor {
        PluginDescriptor {
            id: make_plugin_id(name),
            schema_version: SchemaVersion(1),
            capabilities: vec![CapabilityId::new("test-cap")],
            dependencies: vec![],
            resource_requirements: ResourceBudget {
                memory_bytes: 512,
                cpu_units: 1,
                max_instances: 1,
            },
            isolation_level: IsolationLevel::SharedRuntime,
        }
    }

    #[test]
    fn install_plugin_event_constructs() {
        let desc = make_descriptor("installer");
        let artifact = PluginArtifact {
            artifact_ref: ArtifactRef("ref://path/to/artifact".to_string()),
            checksum: crate::BinaryHash("abc123".to_string()),
            schema_version: SchemaVersion(1),
        };
        let event = HotLoadEvent::InstallPlugin {
            descriptor: desc,
            artifact,
        };
        assert!(matches!(event, HotLoadEvent::InstallPlugin { .. }));
    }

    #[test]
    fn uninstall_plugin_event_constructs() {
        let event = HotLoadEvent::UninstallPlugin {
            plugin_id: make_plugin_id("to-remove"),
        };
        assert!(matches!(event, HotLoadEvent::UninstallPlugin { .. }));
    }

    #[test]
    fn activate_plugin_event_constructs() {
        let event = HotLoadEvent::ActivatePlugin {
            plugin_id: make_plugin_id("activator"),
        };
        assert!(matches!(event, HotLoadEvent::ActivatePlugin { .. }));
    }

    #[test]
    fn deactivate_plugin_event_constructs() {
        let event = HotLoadEvent::DeactivatePlugin {
            plugin_id: make_plugin_id("deactivator"),
        };
        assert!(matches!(event, HotLoadEvent::DeactivatePlugin { .. }));
    }

    #[test]
    fn reload_plugin_event_constructs() {
        let desc = make_descriptor("reloader");
        let event = HotLoadEvent::ReloadPlugin {
            plugin_id: make_plugin_id("reloader"),
            new_descriptor: desc,
        };
        assert!(matches!(event, HotLoadEvent::ReloadPlugin { .. }));
    }

    #[test]
    fn health_check_event_constructs() {
        let event = HotLoadEvent::PluginHealthCheck {
            plugin_id: make_plugin_id("health-check"),
        };
        assert!(matches!(event, HotLoadEvent::PluginHealthCheck { .. }));
    }

    #[test]
    fn event_serializes_to_json() {
        let event = HotLoadEvent::ActivatePlugin {
            plugin_id: make_plugin_id("serde-test"),
        };
        let json = serde_json::to_value(&event).expect("serialize");
        assert!(json.is_object());
    }

    #[test]
    fn event_json_roundtrip() {
        let event = HotLoadEvent::UninstallPlugin {
            plugin_id: make_plugin_id("roundtrip"),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let restored: HotLoadEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, restored);
    }
}
