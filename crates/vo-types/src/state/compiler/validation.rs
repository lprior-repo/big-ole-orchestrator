//! State machine behavior: guard conditions, side effects, and invariant checks.

use crate::state::lifecycle::{LifecycleState, TransitionEvent};

use super::definition::TransitionTable;

// --- Guard conditions ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardResult {
    Accepted,
    Rejected,
}

pub type GuardFn = Box<dyn Fn(LifecycleState, TransitionEvent) -> GuardResult + Send + Sync>;

#[derive(Default)]
pub enum Guard {
    #[default]
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

// --- Side effects ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideEffectResult {
    Executed,
    Skipped,
}

pub type SideEffectFn =
    Box<dyn Fn(LifecycleState, TransitionEvent, LifecycleState) -> SideEffectResult + Send + Sync>;

#[derive(Default)]
pub enum SideEffect {
    #[default]
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
                eprintln!("Transition side effect: {from:?} -> {to:?} via {event:?}: {message}");
                SideEffectResult::Executed
            }
            SideEffect::Fn { f } => f(from, event, to),
        }
    }
}

// --- TransitionTable visualization ---

impl TransitionTable {
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

// --- Invariant checks ---

/// Returns `true` if the state machine allows recovery from the given state.
///
/// The only non-terminal recovery is `Failed -> RunningDecision` via
/// `InstanceResumed`.
pub fn allows_recovery(table: &TransitionTable, state: LifecycleState) -> bool {
    if !table.is_terminal_state(&state) {
        return false;
    }
    state == LifecycleState::Failed
}

/// Returns `true` if a transition from `from` with `event` is valid
/// considering terminal state invariants.
pub fn is_valid_transition(
    table: &TransitionTable,
    from: LifecycleState,
    event: TransitionEvent,
) -> bool {
    if table.is_terminal_state(&from)
        && !(from == LifecycleState::Failed
            && event == TransitionEvent::InstanceResumed)
    {
        return false;
    }
    table.get_rule(from, event).is_some()
}
