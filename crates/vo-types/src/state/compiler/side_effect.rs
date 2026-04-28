use crate::state::lifecycle::{LifecycleState, TransitionEvent};

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
    Log {
        message: String,
    },
    Fn {
        f: SideEffectFn,
    },
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
