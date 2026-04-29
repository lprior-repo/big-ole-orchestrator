//! State machine type definitions.
//!
//! Defines `TransitionTable`, `TransitionRule`, and associated builders.

use std::collections::HashMap;

use crate::state::lifecycle::{LifecycleState, TransitionEvent};

use super::validation::{Guard, GuardResult, SideEffect};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompilerTransitionError {
    #[error("Cannot transition from terminal state")]
    TerminalStateTransition,
    #[error("Invalid transition for current state")]
    InvalidTransition,
    #[error("Guard condition rejected transition")]
    GuardRejected,
}

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

#[derive(Debug, Clone)]
pub struct TransitionTable {
    pub(crate) rules: HashMap<(LifecycleState, TransitionEvent), TransitionRule>,
    pub(crate) terminal_states: Vec<LifecycleState>,
}

impl TransitionTable {
    pub fn builder() -> TransitionTableBuilder {
        TransitionTableBuilder::new()
    }

    pub fn apply(
        &self,
        current: LifecycleState,
        event: TransitionEvent,
    ) -> Result<LifecycleState, CompilerTransitionError> {
        if self.is_terminal_state(&current)
            && !(current == LifecycleState::Failed
                && event == TransitionEvent::InstanceResumed)
        {
            return Err(CompilerTransitionError::TerminalStateTransition);
        }

        let key = (current, event);
        match self.rules.get(&key) {
            Some(rule) => match rule.guard.check(current, event) {
                GuardResult::Accepted => {
                    rule.side_effect.execute(current, event, rule.to);
                    Ok(rule.to)
                }
                GuardResult::Rejected => Err(CompilerTransitionError::GuardRejected),
            },
            None => Err(CompilerTransitionError::InvalidTransition),
        }
    }

    pub fn get_rule(
        &self,
        from: LifecycleState,
        event: TransitionEvent,
    ) -> Option<&TransitionRule> {
        self.rules.get(&(from, event))
    }

    pub fn get_transitions_from(&self, state: LifecycleState) -> Vec<&TransitionRule> {
        self.rules
            .values()
            .filter(|rule| rule.from == state)
            .collect()
    }

    pub fn is_terminal_state(&self, state: &LifecycleState) -> bool {
        self.terminal_states.contains(state)
    }
}

pub struct TransitionTableBuilder {
    pub(crate) table: TransitionTable,
}

impl TransitionTableBuilder {
    pub fn new() -> Self {
        Self {
            table: TransitionTable {
                rules: HashMap::new(),
                terminal_states: vec![
                    LifecycleState::Completed,
                    LifecycleState::Failed,
                    LifecycleState::Cancelled,
                ],
            },
        }
    }

    pub fn add_transition(
        self,
        from: LifecycleState,
        event: TransitionEvent,
    ) -> TransitionBuilder {
        TransitionBuilder::new(self.table, from, event)
    }

    pub fn terminal_state(mut self, state: LifecycleState) -> Self {
        if !self.table.terminal_states.contains(&state) {
            self.table.terminal_states.push(state);
        }
        self
    }

    pub fn build(self) -> TransitionTable {
        self.table
    }
}

impl Default for TransitionTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TransitionBuilder {
    table: TransitionTable,
    from: LifecycleState,
    event: TransitionEvent,
    to: LifecycleState,
    guard: Guard,
    side_effect: SideEffect,
    description: Option<String>,
}

impl TransitionBuilder {
    fn new(table: TransitionTable, from: LifecycleState, event: TransitionEvent) -> Self {
        Self {
            table,
            from,
            event,
            to: LifecycleState::Pending,
            guard: Guard::Always,
            side_effect: SideEffect::None,
            description: None,
        }
    }

    pub fn to(mut self, state: LifecycleState) -> Self {
        self.to = state;
        self
    }

    pub fn with_guard(mut self, guard: Guard) -> Self {
        self.guard = guard;
        self
    }

    pub fn with_side_effect(mut self, effect: SideEffect) -> Self {
        self.side_effect = effect;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn build(mut self) -> TransitionTableBuilder {
        let rule = TransitionRule {
            from: self.from,
            event: self.event,
            to: self.to,
            guard: self.guard,
            side_effect: self.side_effect,
            description: self.description,
        };
        self.table.rules.insert((self.from, self.event), rule);
        TransitionTableBuilder { table: self.table }
    }
}
