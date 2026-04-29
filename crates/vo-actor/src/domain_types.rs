/// Namespace identifier for workflow isolation.
pub type NamespaceId = String;

/// Workflow execution paradigm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowParadigm {
    Fsm,
    Dag,
    Procedural,
}

/// View of an instance's execution phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstancePhaseView {
    Replay,
    Live,
}
