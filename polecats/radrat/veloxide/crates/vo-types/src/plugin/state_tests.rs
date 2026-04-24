#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod state_tests {
    use crate::plugin::types::PluginState;

    #[test]
    fn registered_is_not_terminal() {
        assert!(!PluginState::Registered.is_terminal());
    }

    #[test]
    fn loading_is_not_terminal() {
        assert!(!PluginState::Loading.is_terminal());
    }

    #[test]
    fn active_is_not_terminal() {
        assert!(!PluginState::Active.is_terminal());
    }

    #[test]
    fn quiescing_is_not_terminal() {
        assert!(!PluginState::Quiescing.is_terminal());
    }

    #[test]
    fn unloaded_is_terminal() {
        assert!(PluginState::Unloaded.is_terminal());
    }

    #[test]
    fn failed_is_terminal() {
        let ctx = crate::plugin::PluginFailureContext {
            error: crate::plugin::PluginHotLoadError::new(
                crate::plugin::PluginErrorCategory::LoadFailure,
                crate::plugin::PluginErrorDetail::PluginNotFound(crate::plugin::PluginId::new(
                    crate::plugin::PluginName::new("test").unwrap(),
                    crate::plugin::PluginVersion::new(1, 0, 0),
                    crate::plugin::InstanceKey::new(),
                )),
                crate::plugin::PluginErrorContext::DuringLoad,
            ),
            timestamp_ms: 1000,
        };
        assert!(PluginState::Failed(ctx).is_terminal());
    }

    #[test]
    fn registered_valid_transitions_is_load_only() {
        let transitions = PluginState::Registered.get_valid_transitions();
        assert!(transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Load { .. })));
    }

    #[test]
    fn active_valid_transitions_includes_quiesce_and_fail() {
        let transitions = PluginState::Active.get_valid_transitions();
        let has_quiesce = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Quiesce));
        let has_fail = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Fail { .. }));
        assert!(has_quiesce, "Active should allow Quiesce");
        assert!(has_fail, "Active should allow Fail");
    }

    #[test]
    fn failed_valid_transitions_is_register_only() {
        let ctx = crate::plugin::PluginFailureContext {
            error: crate::plugin::PluginHotLoadError::new(
                crate::plugin::PluginErrorCategory::LoadFailure,
                crate::plugin::PluginErrorDetail::PluginNotFound(crate::plugin::PluginId::new(
                    crate::plugin::PluginName::new("test").unwrap(),
                    crate::plugin::PluginVersion::new(1, 0, 0),
                    crate::plugin::InstanceKey::new(),
                )),
                crate::plugin::PluginErrorContext::DuringLoad,
            ),
            timestamp_ms: 1000,
        };
        let transitions = PluginState::Failed(ctx).get_valid_transitions();
        assert!(transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Register(_))));
        assert_eq!(transitions.len(), 1, "Failed should only allow Register");
    }

    #[test]
    fn unloaded_valid_transitions_is_register_only() {
        let transitions = PluginState::Unloaded.get_valid_transitions();
        assert!(transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Register(_))));
        assert_eq!(transitions.len(), 1, "Unloaded should only allow Register");
    }

    #[test]
    fn loading_valid_transitions_includes_activate_and_fail() {
        let transitions = PluginState::Loading.get_valid_transitions();
        let has_activate = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Activate));
        let has_fail = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Fail { .. }));
        assert!(has_activate, "Loading should allow Activate");
        assert!(has_fail, "Loading should allow Fail");
    }

    #[test]
    fn quiescing_valid_transitions_includes_unload_and_fail() {
        let transitions = PluginState::Quiescing.get_valid_transitions();
        let has_unload = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Unload));
        let has_fail = transitions
            .iter()
            .any(|t| matches!(t, crate::plugin::PluginTransition::Fail { .. }));
        assert!(has_unload, "Quiescing should allow Unload");
        assert!(has_fail, "Quiescing should allow Fail");
    }
}
