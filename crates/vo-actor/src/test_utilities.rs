//! Test utilities for vo-actor.
//!
//! This module provides test helpers that are useful for both internal library
//! tests and external test crates.

use vo_types::InstanceId;

use crate::signal_messages::{LifecycleState, StateLookup};

#[derive(Debug, Clone)]
pub struct TestStateLookup;

impl TestStateLookup {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestStateLookup {
    fn default() -> Self {
        Self::new()
    }
}

impl StateLookup for TestStateLookup {
    fn derive_lifecycle_state(&self, instance_id: &InstanceId) -> LifecycleState {
        let id_str = instance_id.as_str();
        id_str
            .chars()
            .nth(22)
            .map_or(LifecycleState::Running, |c| match c {
                'C' => LifecycleState::Completed,
                'X' => LifecycleState::Cancelled,
                'F' => LifecycleState::Failed,
                'W' => LifecycleState::WaitingForSignal,
                _ => LifecycleState::Running,
            })
    }

    fn derive_error_type(&self, instance_id: &InstanceId) -> Option<&'static str> {
        let id_str = instance_id.as_str();
        id_str.chars().nth(20).and_then(|c| match c {
            'A' => Some("lock"),
            'S' => Some("storage"),
            'M' => Some("missing"),
            'N' => Some("nodenotfound"),
            'P' => Some("nopathtoterminal"),
            _ => None,
        })
    }
}
