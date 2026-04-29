use crate::state::lifecycle::{LifecycleState, TransitionEvent};

use super::guard::Guard;
use super::side_effect::SideEffect;

pub struct TransitionRule {
    pub from: LifecycleState,
    pub event: TransitionEvent,
    pub to: LifecycleState,
    pub guard: Guard,
    pub side_effect: SideEffect,
    pub description: Option<String>,
}

impl Clone for TransitionRule {
    fn clone(&self) -> Self {
        Self {
            from: self.from,
            event: self.event,
            to: self.to,
            guard: self.guard.clone(),
            side_effect: self.side_effect.clone(),
            description: self.description.clone(),
        }
    }
}

impl std::fmt::Debug for TransitionRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransitionRule")
            .field("from", &self.from)
            .field("event", &self.event)
            .field("to", &self.to)
            .field("guard", &"Guard")
            .field("side_effect", &"SideEffect")
            .field("description", &self.description)
            .finish()
    }
}

impl TransitionRule {
    pub fn new(from: LifecycleState, event: TransitionEvent, to: LifecycleState) -> Self {
        Self {
            from,
            event,
            to,
            guard: Guard::Always,
            side_effect: SideEffect::None,
            description: None,
        }
    }
}
