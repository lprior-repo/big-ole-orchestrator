//! `ProceduralActor` — Procedural paradigm actor state and event application (ADR-017).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

pub mod context;
pub mod state;

pub use self::context::WorkflowContext;
pub use self::state::{
    apply_event, Checkpoint, OperationId, ProceduralActorState, ProceduralApplyError,
    ProceduralApplyResult,
};

/// A procedural workflow implementation.
#[async_trait::async_trait]
pub trait WorkflowFn: std::fmt::Debug + Send + Sync + 'static {
    /// Execute the workflow logic using the provided context.
    async fn execute(&self, ctx: WorkflowContext) -> anyhow::Result<()>;
}

/// Runtime container for a procedural workflow and its current state.
pub struct ProceduralActorRuntime {
    pub state: ProceduralActorState,
    pub workflow_fn: Box<dyn WorkflowFn>,
}

impl ProceduralActorRuntime {
    /// Create a new runtime.
    #[must_use]
    pub fn new(state: ProceduralActorState, workflow_fn: Box<dyn WorkflowFn>) -> Self {
        Self { state, workflow_fn }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    #[tokio::test]
    async fn workflow_fn_trait_is_object_safe_and_boxable() {
        #[derive(Debug)]
        struct MyWorkflow;
        #[async_trait]
        impl WorkflowFn for MyWorkflow {
            async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let _: Box<dyn WorkflowFn> = Box::new(MyWorkflow);
    }

    #[test]
    fn procedural_actor_runtime_holds_state_and_fn() {
        #[derive(Debug)]
        struct MyWorkflow;
        #[async_trait]
        impl WorkflowFn for MyWorkflow {
            async fn execute(&self, _ctx: WorkflowContext) -> anyhow::Result<()> {
                Ok(())
            }
        }

        let state = ProceduralActorState::new();
        let runtime = ProceduralActorRuntime::new(state, Box::new(MyWorkflow));

        assert_eq!(runtime.state.operation_counter, 0);
    }
}
