use crate::master::registry::{WorkflowDefinition, WorkflowRegistry};
use crate::messages::{InstanceArguments, InstanceMsg, InstanceSeed};
use ractor::ActorRef;
use std::collections::HashMap;
use std::sync::Arc;
use wtf_common::{EventStore, InstanceId, StateStore, TaskQueue};

/// Configuration for the MasterOrchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of workflow instances this node may run concurrently.
    pub max_instances: usize,
    /// Unique identifier for this engine node.
    pub engine_node_id: String,
    /// Sled database handle for snapshot storage.
    pub snapshot_db: Option<sled::Db>,
    /// Abstract event store for writing events.
    pub event_store: Option<Arc<dyn EventStore>>,
    /// Abstract state store for heartbeats and metadata.
    pub state_store: Option<Arc<dyn StateStore>>,
    /// Abstract task queue for activity dispatch.
    pub task_queue: Option<Arc<dyn TaskQueue>>,
    /// Pre-seeded workflow definitions loaded from KV on startup.
    pub definitions: Vec<(String, WorkflowDefinition)>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_instances: 1000,
            engine_node_id: "engine-local".into(),
            snapshot_db: None,
            event_store: None,
            state_store: None,
            task_queue: None,
            definitions: Vec::new(),
        }
    }
}

/// In-memory state of the MasterOrchestrator.
#[derive(Debug)]
pub struct OrchestratorState {
    /// Registry of all currently active workflow instances.
    pub active: HashMap<InstanceId, ActorRef<InstanceMsg>>,
    /// Configuration (immutable after construction).
    pub config: OrchestratorConfig,
    /// Registry of available workflows.
    pub registry: WorkflowRegistry,
}

impl OrchestratorState {
    /// Create a new orchestrator state with the given config and pre-seeded definitions.
    #[must_use]
    pub fn new(config: OrchestratorConfig) -> Self {
        let mut registry = WorkflowRegistry::new();
        for (name, definition) in &config.definitions {
            registry.register_definition(name, definition.clone());
        }
        Self {
            active: HashMap::new(),
            config,
            registry,
        }
    }

    /// Return the number of currently active instances.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Returns `true` when `self.active.len() < self.config.max_instances`.
    ///
    /// This method is **pure** — it performs no state mutation.
    ///
    /// # Contract
    ///
    /// - Returns `true` iff `active.len() < max_instances` (capacity available)
    /// - Returns `false` iff `active.len() >= max_instances` (at limit)
    #[must_use]
    pub fn capacity_check(&self) -> bool {
        self.active.len() < self.config.max_instances
    }

    /// Register a newly spawned instance.
    pub fn register(&mut self, id: InstanceId, actor_ref: ActorRef<InstanceMsg>) {
        self.active.insert(id, actor_ref);
    }

    /// Deregister a stopped instance.
    pub fn deregister(&mut self, id: &InstanceId) {
        self.active.remove(id);
    }

    /// Look up an active instance by ID.
    #[must_use]
    pub fn get(&self, id: &InstanceId) -> Option<&ActorRef<InstanceMsg>> {
        self.active.get(id)
    }

    /// Build `InstanceArguments` from infrastructure config + per-instance seed.
    ///
    /// This is the single source of truth for wiring config and registry fields
    /// into the argument struct, used by both fresh-spawn and crash-recovery paths.
    #[must_use]
    pub fn build_instance_args(&self, seed: InstanceSeed) -> InstanceArguments {
        InstanceArguments {
            namespace: seed.namespace,
            instance_id: seed.instance_id,
            workflow_type: seed.workflow_type.clone(),
            paradigm: seed.paradigm,
            input: seed.input,
            engine_node_id: self.config.engine_node_id.clone(),
            event_store: self.config.event_store.clone(),
            state_store: self.config.state_store.clone(),
            task_queue: self.config.task_queue.clone(),
            snapshot_db: self.config.snapshot_db.clone(),
            procedural_workflow: self.registry.get_procedural(&seed.workflow_type),
            workflow_definition: self.registry.get_definition(&seed.workflow_type),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ractor::Actor as _;

    fn test_config() -> OrchestratorConfig {
        OrchestratorConfig {
            max_instances: 10,
            engine_node_id: "node-test".into(),
            snapshot_db: None,
            event_store: None,
            state_store: None,
            task_queue: None,
            definitions: Vec::new(),
        }
    }

    #[test]
    fn new_state_is_empty() {
        let state = OrchestratorState::new(test_config());
        assert_eq!(state.active_count(), 0);
    }

    #[test]
    fn has_capacity_when_empty() {
        let state = OrchestratorState::new(test_config());
        assert!(state.capacity_check());
    }

    #[test]
    fn has_capacity_false_when_at_limit() {
        let config = OrchestratorConfig {
            max_instances: 0,
            engine_node_id: "node".into(),
            snapshot_db: None,
            event_store: None,
            state_store: None,
            task_queue: None,
            definitions: Vec::new(),
        };
        let state = OrchestratorState::new(config);
        assert!(!state.capacity_check());
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let state = OrchestratorState::new(test_config());
        let id = InstanceId::new("unknown");
        assert!(state.get(&id).is_none());
    }

    #[test]
    fn deregister_removes_entry() {
        let mut state = OrchestratorState::new(test_config());
        let id = InstanceId::new("not-there");
        state.deregister(&id); // should not panic
        assert_eq!(state.active_count(), 0);
    }

    /// Minimal actor that discards all messages — used to obtain a valid `ActorRef<InstanceMsg>` for tests.
    struct NullActor;

    #[async_trait::async_trait]
    impl ractor::Actor for NullActor {
        type Msg = InstanceMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _: ractor::ActorRef<Self::Msg>,
            _: Self::Arguments,
        ) -> Result<(), ractor::ActorProcessingErr> {
            Ok(())
        }
    }

    fn single_instance_config() -> OrchestratorConfig {
        OrchestratorConfig {
            max_instances: 1,
            engine_node_id: "node".into(),
            snapshot_db: None,
            event_store: None,
            state_store: None,
            task_queue: None,
            definitions: Vec::new(),
        }
    }

    #[tokio::test]
    async fn has_capacity_false_when_exactly_one_at_max_one() {
        let mut state = OrchestratorState::new(single_instance_config());
        let id = InstanceId::new("only-instance");
        let (actor_ref, _handle) = NullActor::spawn(None, NullActor, ())
            .await
            .expect("null actor spawned");
        state.register(id, actor_ref);
        assert!(!state.capacity_check());
    }

    #[test]
    fn orchestrator_config_default_max_instances() {
        let cfg = OrchestratorConfig::default();
        assert_eq!(cfg.max_instances, 1000);
    }

    #[test]
    fn orchestrator_config_default_node_id() {
        let cfg = OrchestratorConfig::default();
        assert_eq!(cfg.engine_node_id, "engine-local");
    }

    #[test]
    fn new_state_with_pre_seeded_definitions_populates_registry() {
        let def = WorkflowDefinition {
            paradigm: wtf_common::WorkflowParadigm::Fsm,
            graph_raw: r#"{"states":[],"transitions":[]}"#.to_owned(),
            description: Some("test def".to_owned()),
        };
        let config = OrchestratorConfig {
            definitions: vec![("payments/checkout".to_owned(), def.clone())],
            ..OrchestratorConfig::default()
        };
        let state = OrchestratorState::new(config);
        let looked_up = state.registry.get_definition("payments/checkout");
        assert_eq!(looked_up, Some(def));
    }

    #[test]
    fn new_state_with_multiple_definitions() {
        let def_a = WorkflowDefinition {
            paradigm: wtf_common::WorkflowParadigm::Dag,
            graph_raw: r#"{"nodes":[],"edges":[]}"#.to_owned(),
            description: None,
        };
        let def_b = WorkflowDefinition {
            paradigm: wtf_common::WorkflowParadigm::Fsm,
            graph_raw: r#"{"states":["a","b"],"transitions":[]}"#.to_owned(),
            description: Some("B workflow".to_owned()),
        };
        let config = OrchestratorConfig {
            definitions: vec![
                ("ns1/wf-a".to_owned(), def_a),
                ("ns2/wf-b".to_owned(), def_b),
            ],
            ..OrchestratorConfig::default()
        };
        let state = OrchestratorState::new(config);
        assert!(state.registry.get_definition("ns1/wf-a").is_some());
        assert!(state.registry.get_definition("ns2/wf-b").is_some());
        assert!(state.registry.get_definition("nonexistent").is_none());
    }

    #[test]
    fn new_state_with_empty_definitions_has_empty_registry() {
        let state = OrchestratorState::new(OrchestratorConfig::default());
        assert!(state.registry.get_definition("anything").is_none());
    }
}
