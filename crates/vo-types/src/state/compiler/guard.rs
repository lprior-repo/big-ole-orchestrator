use crate::state::lifecycle::{LifecycleState, TransitionEvent};

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
    Fn {
        f: GuardFn,
    },
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
