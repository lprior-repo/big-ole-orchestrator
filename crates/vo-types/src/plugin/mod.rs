//! Plugin hot-load system types (TDD Red — types not yet implemented).
//!
//! Architecture: Data (PluginId, PluginName, PluginVersion, PluginDescriptor, PluginInstance)
//!             → Calc (apply_plugin_transition, PluginState lifecycle)
//!             → Actions (hot-load events processed by engine layer).
//!
//! Contract: docs/contracts/plugin-hot-load-system.md
//! Test Plan: docs/test-plans/plugin-hot-load-system-test-plan.md

#![allow(dead_code, unused_imports)]

mod errors;
mod lifecycle;
mod types;

#[cfg(test)]
mod descriptor_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod fence_token_tests;
#[cfg(test)]
mod name_tests;
#[cfg(test)]
mod plugin_id_tests;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod transition_tests;
#[cfg(test)]
mod version_tests;

#[cfg(feature = "proptest")]
mod proptests;

#[cfg(kani)]
mod verification;

pub use errors::{
    IsolationBreachType, IsolationLevel, PluginErrorCategory, PluginErrorContext,
    PluginErrorDetail, PluginHotLoadError,
};
pub use lifecycle::apply_plugin_transition;
pub use types::{
    ArtifactRef, CapabilityId, HotLoadEvent, InstanceKey, PluginArtifact, PluginDescriptor,
    PluginFailureContext, PluginId, PluginInstance, PluginName, PluginState, PluginTransition,
    PluginVersion, PluginVersionConstraint, ResourceBudget, SchemaVersion, VersionRange,
};

pub const PLUGIN_NAME_MAX_LEN: usize = 64;
