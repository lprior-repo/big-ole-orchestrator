use super::errors::{
    PluginErrorCategory, PluginErrorContext, PluginErrorDetail, PluginHotLoadError,
};
use super::types::{PluginFailureContext, PluginState, PluginTransition};

pub fn apply_plugin_transition(
    state: PluginState,
    transition: PluginTransition,
) -> Result<PluginState, PluginHotLoadError> {
    match (&state, &transition) {
        (PluginState::Unloaded, PluginTransition::Register(_)) => Ok(PluginState::Registered),
        (PluginState::Failed(_), PluginTransition::Register(_)) => Ok(PluginState::Registered),
        (PluginState::Registered, PluginTransition::Load { .. }) => Ok(PluginState::Loading),
        (PluginState::Loading, PluginTransition::Activate) => Ok(PluginState::Active),
        (PluginState::Active, PluginTransition::Quiesce) => Ok(PluginState::Quiescing),
        (PluginState::Quiescing, PluginTransition::Unload) => Ok(PluginState::Unloaded),
        (PluginState::Active, PluginTransition::Reload { .. }) => Ok(PluginState::Active),
        (
            PluginState::Registered
            | PluginState::Loading
            | PluginState::Active
            | PluginState::Quiescing,
            PluginTransition::Fail { error },
        ) => {
            let failure_ctx = PluginFailureContext {
                error: error.clone(),
                timestamp_ms: 0,
            };
            Ok(PluginState::Failed(failure_ctx))
        }
        _ => Err(PluginHotLoadError::new(
            PluginErrorCategory::RegistrationFailure,
            PluginErrorDetail::PluginNotFound(make_unknown_plugin_id()),
            PluginErrorContext::DuringRegistration,
        )),
    }
}

fn make_unknown_plugin_id() -> super::types::PluginId {
    use super::types::{InstanceKey, PluginId, PluginName, PluginVersion};
    PluginId::new(
        PluginName::new("unknown").unwrap_or_else(|_| {
            #[allow(clippy::expect_used)]
            PluginName::new("x").expect("'x' is always a valid PluginName")
        }),
        PluginVersion::new(0, 0, 0),
        InstanceKey::new(),
    )
}
