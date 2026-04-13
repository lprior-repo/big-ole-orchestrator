//! Declarative State Machine Compiler
//!
//! Provides a declarative TransitionTable with builder API, guard conditions,
//! side effects, and DOT visualization output.
//!
//! # Example
//!
//! ```rust
//! use vo_types::state::lifecycle::{LifecycleState, TransitionEvent};
//! use vo_types::state::compiler::{TransitionTable, Transition, Guard, SideEffect};
//!
//! let table = TransitionTable::builder()
//!     .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
//!         .to(LifecycleState::RunningDecision)
//!         .with_guard(Guard::Always)
//!         .with_side_effect(SideEffect::None)
//!         .build()
//!     .build();
//! ```

use crate::state::lifecycle::{LifecycleState, TransitionEvent};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompilerTransitionError {
    #[error("Cannot transition from terminal state")]
    TerminalStateTransition,
    #[error("Invalid transition for current state")]
    InvalidTransition,
    #[error("Guard condition rejected transition")]
    GuardRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardResult {
    Accepted,
    Rejected,
}

pub type GuardFn = Box<dyn Fn(LifecycleState, TransitionEvent) -> GuardResult + Send + Sync>;

pub enum Guard {
    Always,
    Never,
    If(fn(LifecycleState, TransitionEvent) -> bool),
    Fn { f: GuardFn },
}

impl Clone for Guard {
    fn clone(&self) -> Self {
        match self {
            Guard::Always => Guard::Always,
            Guard::Never => Guard::Never,
            Guard::If(predicate) => Guard::If(*predicate),
            Guard::Fn { f: _ } => Guard::Fn {
                f: Box::new(|_, _| GuardResult::Rejected),
            },
        }
    }
}

impl Guard {
    pub fn check(&self, state: LifecycleState, event: TransitionEvent) -> GuardResult {
        match self {
            Guard::Always => GuardResult::Accepted,
            Guard::Never => GuardResult::Rejected,
            Guard::If(predicate) => {
                if predicate(state, event) {
                    GuardResult::Accepted
                } else {
                    GuardResult::Rejected
                }
            }
            Guard::Fn { f } => f(state, event),
        }
    }
}

impl Default for Guard {
    fn default() -> Self {
        Guard::Always
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideEffectResult {
    Executed,
    Skipped,
}

pub type SideEffectFn =
    Box<dyn Fn(LifecycleState, TransitionEvent, LifecycleState) -> SideEffectResult + Send + Sync>;

pub enum SideEffect {
    None,
    Log { message: String },
    Fn { f: SideEffectFn },
}

impl Clone for SideEffect {
    fn clone(&self) -> Self {
        match self {
            SideEffect::None => SideEffect::None,
            SideEffect::Log { message } => SideEffect::Log {
                message: message.clone(),
            },
            SideEffect::Fn { f: _ } => SideEffect::Fn {
                f: Box::new(|_, _, _| SideEffectResult::Skipped),
            },
        }
    }
}

impl SideEffect {
    pub fn execute(
        &self,
        from: LifecycleState,
        event: TransitionEvent,
        to: LifecycleState,
    ) -> SideEffectResult {
        match self {
            SideEffect::None => SideEffectResult::Skipped,
            SideEffect::Log { message } => {
                eprintln!(
                    "Transition side effect: {} -> {:?} via {:?}: {}",
                    format!("{:?}", from),
                    to,
                    event,
                    message
                );
                SideEffectResult::Executed
            }
            SideEffect::Fn { f } => f(from, event, to),
        }
    }
}

impl Default for SideEffect {
    fn default() -> Self {
        SideEffect::None
    }
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
            && !(current == LifecycleState::Failed && event == TransitionEvent::InstanceResumed)
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
                rules: HashMap::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_table_builder_basic() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .build()
            .build();

        let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
        assert_eq!(result, Ok(LifecycleState::RunningDecision));
    }

    #[test]
    fn test_transition_table_invalid_transition() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .build()
            .build();

        let result = table.apply(LifecycleState::Pending, TransitionEvent::StepScheduled);
        assert_eq!(result, Err(CompilerTransitionError::InvalidTransition));
    }

    #[test]
    fn test_terminal_state_rejection() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .build()
            .build();

        let result = table.apply(LifecycleState::Completed, TransitionEvent::AssignToNode);
        assert_eq!(
            result,
            Err(CompilerTransitionError::TerminalStateTransition)
        );
    }

    #[test]
    fn test_guard_always_accepts() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .with_guard(Guard::Always)
            .build()
            .build();

        let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
        assert_eq!(result, Ok(LifecycleState::RunningDecision));
    }

    #[test]
    fn test_guard_never_rejects() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .with_guard(Guard::Never)
            .build()
            .build();

        let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
        assert_eq!(result, Err(CompilerTransitionError::GuardRejected));
    }

    #[test]
    fn test_guard_predicate() {
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .with_guard(Guard::If(|_, _| true))
            .build()
            .build();

        let result = table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode);
        assert_eq!(result, Ok(LifecycleState::RunningDecision));
    }

    #[test]
    fn test_side_effect_execution() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static EXECUTED: AtomicBool = AtomicBool::new(false);
        let table = TransitionTable::builder()
            .add_transition(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .to(LifecycleState::RunningDecision)
            .with_side_effect(SideEffect::Fn {
                f: Box::new(|_, _, _| {
                    EXECUTED.store(true, Ordering::SeqCst);
                    SideEffectResult::Executed
                }),
            })
            .build()
            .build();

        table
            .apply(LifecycleState::Pending, TransitionEvent::AssignToNode)
            .unwrap();
        assert!(EXECUTED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_lifecycle_table_complete() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::Pending, TransitionEvent::AssignToNode),
            Ok(LifecycleState::RunningDecision)
        );
        assert_eq!(
            table.apply(
                LifecycleState::RunningDecision,
                TransitionEvent::StepScheduled
            ),
            Ok(LifecycleState::StepScheduled)
        );
        assert_eq!(
            table.apply(LifecycleState::StepScheduled, TransitionEvent::ExecuteStep),
            Ok(LifecycleState::StepExecuting)
        );
        assert_eq!(
            table.apply(LifecycleState::StepExecuting, TransitionEvent::CompleteStep),
            Ok(LifecycleState::Completed)
        );
    }

    #[test]
    fn test_lifecycle_table_cancel() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::Pending, TransitionEvent::Cancel),
            Ok(LifecycleState::Cancelled)
        );
    }

    #[test]
    fn test_lifecycle_table_fail() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::RunningDecision, TransitionEvent::Fail),
            Ok(LifecycleState::Failed)
        );
    }

    #[test]
    fn test_lifecycle_table_recovery() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::Failed, TransitionEvent::InstanceResumed),
            Ok(LifecycleState::RunningDecision)
        );
    }

    #[test]
    fn test_lifecycle_table_timer_path() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer),
            Ok(LifecycleState::WaitingForTimer)
        );
        assert_eq!(
            table.apply(LifecycleState::WaitingForTimer, TransitionEvent::TimerFired),
            Ok(LifecycleState::StepExecuting)
        );
    }

    #[test]
    fn test_lifecycle_table_timer_expired() {
        let table = create_lifecycle_table();

        assert_eq!(
            table.apply(LifecycleState::StepExecuting, TransitionEvent::WaitForTimer),
            Ok(LifecycleState::WaitingForTimer)
        );
        assert_eq!(
            table.apply(
                LifecycleState::WaitingForTimer,
                TransitionEvent::TimerExpired
            ),
            Ok(LifecycleState::Failed)
        );
    }

    #[test]
    fn test_dot_visualization() {
        let table = create_lifecycle_table();
        let dot = table.to_dot();

        assert!(dot.contains("digraph LifecycleStateMachine"));
        assert!(dot.contains("Pending"));
        assert!(dot.contains("RunningDecision"));
    }

    #[test]
    fn test_get_transitions_from() {
        let table = create_lifecycle_table();
        let transitions = table.get_transitions_from(LifecycleState::Pending);

        assert_eq!(transitions.len(), 2);
    }

    #[test]
    fn test_is_terminal_state() {
        let table = create_lifecycle_table();

        assert!(table.is_terminal_state(&LifecycleState::Completed));
        assert!(table.is_terminal_state(&LifecycleState::Failed));
        assert!(table.is_terminal_state(&LifecycleState::Cancelled));
        assert!(!table.is_terminal_state(&LifecycleState::Pending));
    }
}
