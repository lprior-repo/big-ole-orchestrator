#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod transition_tests {
    use crate::plugin::lifecycle::apply_plugin_transition;
    use crate::plugin::types::{PluginState, PluginTransition};
    use crate::plugin::*;

    fn make_descriptor(name: &str) -> PluginDescriptor {
        let id = PluginId::new(
            PluginName::new(name).unwrap(),
            PluginVersion::new(1, 0, 0),
            InstanceKey::new(),
        );
        PluginDescriptor {
            id,
            schema_version: SchemaVersion(1),
            capabilities: vec![],
            dependencies: vec![],
            resource_requirements: ResourceBudget {
                memory_bytes: 1024,
                cpu_units: 1,
                max_instances: 1,
            },
            isolation_level: IsolationLevel::SharedRuntime,
        }
    }

    fn make_error() -> PluginHotLoadError {
        PluginHotLoadError::new(
            PluginErrorCategory::LoadFailure,
            PluginErrorDetail::PluginNotFound(PluginId::new(
                PluginName::new("missing").unwrap(),
                PluginVersion::new(1, 0, 0),
                InstanceKey::new(),
            )),
            PluginErrorContext::DuringLoad,
        )
    }

    fn make_failed_state() -> PluginState {
        PluginState::Failed(PluginFailureContext {
            error: make_error(),
            timestamp_ms: 1000,
        })
    }

    #[test]
    fn register_transitions_to_registered() {
        let desc = make_descriptor("test-plugin");
        let result =
            apply_plugin_transition(PluginState::Unloaded, PluginTransition::Register(desc));
        assert_eq!(result.unwrap(), PluginState::Registered);
    }

    #[test]
    fn register_from_failed_transitions_to_registered() {
        let desc = make_descriptor("recovery-plugin");
        let result = apply_plugin_transition(make_failed_state(), PluginTransition::Register(desc));
        assert_eq!(result.unwrap(), PluginState::Registered);
    }

    #[test]
    fn load_transitions_registered_to_loading() {
        let result = apply_plugin_transition(
            PluginState::Registered,
            PluginTransition::Load {
                expected_version: PluginVersion::new(1, 0, 0),
            },
        );
        assert_eq!(result.unwrap(), PluginState::Loading);
    }

    #[test]
    fn activate_transitions_loading_to_active() {
        let result = apply_plugin_transition(PluginState::Loading, PluginTransition::Activate);
        assert_eq!(result.unwrap(), PluginState::Active);
    }

    #[test]
    fn quiesce_transitions_active_to_quiescing() {
        let result = apply_plugin_transition(PluginState::Active, PluginTransition::Quiesce);
        assert_eq!(result.unwrap(), PluginState::Quiescing);
    }

    #[test]
    fn unload_transitions_quiescing_to_unloaded() {
        let result = apply_plugin_transition(PluginState::Quiescing, PluginTransition::Unload);
        assert_eq!(result.unwrap(), PluginState::Unloaded);
    }

    #[test]
    fn fail_from_active_transitions_to_failed() {
        let result = apply_plugin_transition(
            PluginState::Active,
            PluginTransition::Fail {
                error: make_error(),
            },
        );
        assert!(matches!(result.unwrap(), PluginState::Failed(_)));
    }

    #[test]
    fn fail_from_loading_transitions_to_failed() {
        let result = apply_plugin_transition(
            PluginState::Loading,
            PluginTransition::Fail {
                error: make_error(),
            },
        );
        assert!(matches!(result.unwrap(), PluginState::Failed(_)));
    }

    #[test]
    fn fail_from_quiescing_transitions_to_failed() {
        let result = apply_plugin_transition(
            PluginState::Quiescing,
            PluginTransition::Fail {
                error: make_error(),
            },
        );
        assert!(matches!(result.unwrap(), PluginState::Failed(_)));
    }

    #[test]
    fn fail_from_registered_transitions_to_failed() {
        let result = apply_plugin_transition(
            PluginState::Registered,
            PluginTransition::Fail {
                error: make_error(),
            },
        );
        assert!(matches!(result.unwrap(), PluginState::Failed(_)));
    }

    #[test]
    fn load_from_active_rejected() {
        let result = apply_plugin_transition(
            PluginState::Active,
            PluginTransition::Load {
                expected_version: PluginVersion::new(1, 0, 0),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn activate_from_registered_rejected() {
        let result = apply_plugin_transition(PluginState::Registered, PluginTransition::Activate);
        assert!(result.is_err());
    }

    #[test]
    fn unload_from_active_rejected() {
        let result = apply_plugin_transition(PluginState::Active, PluginTransition::Unload);
        assert!(result.is_err());
    }

    #[test]
    fn quiesce_from_loading_rejected() {
        let result = apply_plugin_transition(PluginState::Loading, PluginTransition::Quiesce);
        assert!(result.is_err());
    }

    #[test]
    fn register_from_active_rejected() {
        let desc = make_descriptor("bad-register");
        let result = apply_plugin_transition(PluginState::Active, PluginTransition::Register(desc));
        assert!(result.is_err());
    }

    #[test]
    fn load_from_unloaded_rejected() {
        let result = apply_plugin_transition(
            PluginState::Unloaded,
            PluginTransition::Load {
                expected_version: PluginVersion::new(1, 0, 0),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn unload_from_unloaded_rejected() {
        let result = apply_plugin_transition(PluginState::Unloaded, PluginTransition::Unload);
        assert!(result.is_err());
    }

    #[test]
    fn activate_from_unloaded_rejected() {
        let result = apply_plugin_transition(PluginState::Unloaded, PluginTransition::Activate);
        assert!(result.is_err());
    }

    #[test]
    fn quiesce_from_unloaded_rejected() {
        let result = apply_plugin_transition(PluginState::Unloaded, PluginTransition::Quiesce);
        assert!(result.is_err());
    }

    #[test]
    fn load_from_failed_rejected() {
        let result = apply_plugin_transition(
            make_failed_state(),
            PluginTransition::Load {
                expected_version: PluginVersion::new(1, 0, 0),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn activate_from_failed_rejected() {
        let result = apply_plugin_transition(make_failed_state(), PluginTransition::Activate);
        assert!(result.is_err());
    }

    #[test]
    fn quiesce_from_failed_rejected() {
        let result = apply_plugin_transition(make_failed_state(), PluginTransition::Quiesce);
        assert!(result.is_err());
    }

    #[test]
    fn unload_from_failed_rejected() {
        let result = apply_plugin_transition(make_failed_state(), PluginTransition::Unload);
        assert!(result.is_err());
    }

    #[test]
    fn reload_from_active_transitions() {
        let desc = make_descriptor("reload-plugin");
        let result = apply_plugin_transition(
            PluginState::Active,
            PluginTransition::Reload {
                new_descriptor: desc,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn reload_from_non_active_rejected() {
        let desc = make_descriptor("reload-plugin");
        let result = apply_plugin_transition(
            PluginState::Registered,
            PluginTransition::Reload {
                new_descriptor: desc,
            },
        );
        assert!(result.is_err());
    }
}
