//! Transition table compilation and builder API.
//!
//! TransitionTable, TransitionTableBuilder, TransitionBuilder, and
//! create_lifecycle_table (the default lifecycle state machine).

use crate::state::lifecycle::{LifecycleState, TransitionEvent};

use super::definition::{CompilerTransitionError, Guard, GuardResult, SideEffect, TransitionRule};

pub struct TransitionTable {
    pub(crate) rules: std::collections::HashMap<(LifecycleState, TransitionEvent), TransitionRule>,
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
        if super::validation::is_terminal_transition(current, event) {
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

    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph LifecycleStateMachine {\n");
        dot.push_str("    rankdir=LR;\n");
        dot.push_str("    node [shape=ellipse];\n\n");

        for state in &self.terminal_states {
            dot.push_str(&format!(
                "    {:?} [style=filled, fillcolor=gray];\n",
                state
            ));
        }

        for rule in self.rules.values() {
            let guard_label = match &rule.guard {
                Guard::Always => String::new(),
                Guard::Never => "[never]".to_string(),
                Guard::If(_) => "[if]".to_string(),
                Guard::Fn { .. } => "[fn]".to_string(),
            };
            let effect_label = match &rule.side_effect {
                SideEffect::None => String::new(),
                SideEffect::Log { message } => format!("\\n{}", message),
                SideEffect::Fn { .. } => "\\n[side-effect]".to_string(),
            };
            let label = format!("{:?}{}", rule.event, guard_label);
            dot.push_str(&format!(
                "    {:?} -> {:?} [label=\"{}{}\"];\n",
                rule.from, rule.to, label, effect_label
            ));
        }

        dot.push_str("}\n");
        dot
    }
}

pub struct TransitionTableBuilder {
    pub(crate) table: TransitionTable,
}

impl TransitionTableBuilder {
    pub fn new() -> Self {
        Self {
            table: TransitionTable {
                rules: std::collections::HashMap::new(),
                terminal_states: vec![
                    LifecycleState::Completed,
                    LifecycleState::Failed,
                    LifecycleState::Cancelled,
                ],
            },
        }
    }

    pub fn add_transition(self, from: LifecycleState, event: TransitionEvent) -> TransitionBuilder {
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

pub fn create_lifecycle_table() -> TransitionTable {
    TransitionTable::builder()
        .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
        .to(LifecycleState::RunningDecision)
        .with_description("Assign pending bead to node")
        .build()
        .add_transition(LifecycleState::Pending, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel pending bead")
        .build()
        .add_transition(
            LifecycleState::RunningDecision,
            TransitionEvent::StepScheduled,
        )
        .to(LifecycleState::StepScheduled)
        .with_description("Schedule step for execution")
        .build()
        .add_transition(LifecycleState::RunningDecision, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel during decision")
        .build()
        .add_transition(LifecycleState::RunningDecision, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail during decision")
        .build()
        .add_transition(LifecycleState::StepScheduled, TransitionEvent::ExecuteStep)
        .to(LifecycleState::StepExecuting)
        .with_description("Begin step execution")
        .build()
        .add_transition(LifecycleState::StepScheduled, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel scheduled step")
        .build()
        .add_transition(LifecycleState::StepScheduled, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail scheduled step")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer)
        .to(LifecycleState::WaitingForTimer)
        .with_description("Wait for timer")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::CompleteStep)
        .to(LifecycleState::Completed)
        .with_description("Complete step successfully")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel executing step")
        .build()
        .add_transition(LifecycleState::StepExecuting, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail executing step")
        .build()
        .add_transition(
            LifecycleState::StepExecuting,
            TransitionEvent::PrepareEffect,
        )
        .to(LifecycleState::PreparingEffect)
        .with_description("Begin preparing managed effect")
        .build()
        .add_transition(
            LifecycleState::PreparingEffect,
            TransitionEvent::EffectPrepared,
        )
        .to(LifecycleState::StepExecuting)
        .with_description("Effect prepared, resume execution")
        .build()
        .add_transition(LifecycleState::PreparingEffect, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel while preparing effect")
        .build()
        .add_transition(LifecycleState::PreparingEffect, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail while preparing effect")
        .build()
        .add_transition(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired)
        .to(LifecycleState::StepExecuting)
        .with_description("Timer fired, resume execution")
        .build()
        .add_transition(
            LifecycleState::WaitingForTimer,
            TransitionEvent::TimerExpired,
        )
        .to(LifecycleState::Failed)
        .with_description("Timer expired")
        .build()
        .add_transition(LifecycleState::WaitingForTimer, TransitionEvent::Cancel)
        .to(LifecycleState::Cancelled)
        .with_description("Cancel while waiting for timer")
        .build()
        .add_transition(LifecycleState::WaitingForTimer, TransitionEvent::Fail)
        .to(LifecycleState::Failed)
        .with_description("Fail while waiting for timer")
        .build()
        .add_transition(LifecycleState::Failed, TransitionEvent::InstanceResumed)
        .to(LifecycleState::RunningDecision)
        .with_description("Resume failed instance")
        .with_guard(Guard::Always)
        .build()
        .build()
}
