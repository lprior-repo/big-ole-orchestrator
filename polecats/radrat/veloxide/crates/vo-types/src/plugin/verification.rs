#![cfg(kani)]

use crate::plugin::types::{PluginState, PluginTransition};
use crate::plugin::*;
use crate::FenceToken;

#[kani::proof]
fn fence_token_monotonicity() {
    let value: u64 = kani::any();
    kani::assume(value > 0 && value < u64::MAX);
    let token = FenceToken::new(value).unwrap();
    let next = token.next().unwrap();
    assert!(next > token);
}

#[kani::proof]
fn plugin_version_compatibility_is_reflexive() {
    let major: u32 = kani::any();
    let minor: u32 = kani::any();
    let patch: u32 = kani::any();
    let v = PluginVersion::new(major, minor, patch);
    assert!(v.is_compatible_with(&v));
}

#[kani::proof]
fn plugin_version_compatibility_is_symmetric() {
    let m1: u32 = kani::any();
    let n1: u32 = kani::any();
    let p1: u32 = kani::any();
    let m2: u32 = kani::any();
    let n2: u32 = kani::any();
    let p2: u32 = kani::any();
    let v1 = PluginVersion::new(m1, n1, p1);
    let v2 = PluginVersion::new(m2, n2, p2);
    assert_eq!(v1.is_compatible_with(&v2), v2.is_compatible_with(&v1));
}

#[kani::proof]
fn plugin_state_transition_is_total() {
    let state_idx: u8 = kani::any();
    let event_idx: u8 = kani::any();
    kani::assume(state_idx < 6);
    kani::assume(event_idx < 7);

    let state = match state_idx {
        0 => PluginState::Registered,
        1 => PluginState::Loading,
        2 => PluginState::Active,
        3 => PluginState::Quiescing,
        4 => PluginState::Unloaded,
        5 => PluginState::Failed(PluginFailureContext {
            error: PluginHotLoadError::new(
                PluginErrorCategory::LoadFailure,
                PluginErrorDetail::PluginNotFound(PluginId::new(
                    PluginName::new("k").unwrap(),
                    PluginVersion::new(1, 0, 0),
                    InstanceKey::new(),
                )),
                PluginErrorContext::DuringLoad,
            ),
            timestamp_ms: 1,
        }),
        _ => unreachable!(),
    };

    let _result = apply_plugin_transition(state, dummy_transition(event_idx));
}

fn dummy_transition(idx: u8) -> PluginTransition {
    let desc = PluginDescriptor {
        id: PluginId::new(
            PluginName::new("k").unwrap(),
            PluginVersion::new(1, 0, 0),
            InstanceKey::new(),
        ),
        schema_version: SchemaVersion(1),
        capabilities: vec![],
        dependencies: vec![],
        resource_requirements: ResourceBudget {
            memory_bytes: 1,
            cpu_units: 1,
            max_instances: 1,
        },
        isolation_level: IsolationLevel::SharedRuntime,
    };
    let err = PluginHotLoadError::new(
        PluginErrorCategory::LoadFailure,
        PluginErrorDetail::PluginNotFound(PluginId::new(
            PluginName::new("k").unwrap(),
            PluginVersion::new(1, 0, 0),
            InstanceKey::new(),
        )),
        PluginErrorContext::DuringLoad,
    );
    match idx {
        0 => PluginTransition::Register(desc),
        1 => PluginTransition::Load {
            expected_version: PluginVersion::new(1, 0, 0),
        },
        2 => PluginTransition::Activate,
        3 => PluginTransition::Quiesce,
        4 => PluginTransition::Unload,
        5 => PluginTransition::Reload {
            new_descriptor: desc,
        },
        6 => PluginTransition::Fail { error: err },
        _ => unreachable!(),
    }
}
