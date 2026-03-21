# Contract: wtf-frontend (Phase 5 — Frontend)

## Epic Overview

**Bead ID:** wtf-qnol  
**Type:** Epic  
**Dependency:** Phase 4 (API must exist)  
**ADR:** ADR-018

Dioxus application that is both a workflow compiler and a live monitor. Adapted from Oya frontend at `/home/lewis/src/oya-frontend/`. Key adaptations: replace Restate HTTP client with wtf_client, replace Restate node types with FSM/DAG/Procedural node types, add code generator, add time-travel scrubber.

---

## 1. Preconditions

### External Dependencies
- [ ] Phase 4 API (`wtf-api`) must exist and be operational
- [ ] `wtf_client` crate must exist for HTTP communication

### Internal Dependencies
- [ ] `wtf-frontend` crate initialized in `crates/wtf-frontend/`
- [ ] Dioxus 0.7+ with router configured

### Assumptions
- FSM nodes: State machine transitions (entry, transition, state, final)
- DAG nodes: Directed acyclic graph for dataflow
- Procedural nodes: Sequential/scripted execution steps

---

## 2. Core Architecture

### 2.1 Client Layer (`wtf_client`)

```rust
// Replaces RestateClient from Oya
pub struct WtfClient {
    http_client: reqwest::Client,
    base_url: String,
    timeout_secs: u64,
}

impl WtfClient {
    pub async fn get_workflow(&self, id: &str) -> Result<Workflow, ClientError>;
    pub async fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, ClientError>;
    pub async fn get_execution(&self, workflow_id: &str, execution_id: &str) -> Result<ExecutionRecord, ClientError>;
    pub async fn list_executions(&self, workflow_id: &str) -> Result<Vec<ExecutionSummary>, ClientError>;
    pub async fn subscribe_events(&self, workflow_id: &str) -> Result<Stream<WftEvent>, ClientError>;
    pub async fn validate_workflow(&self, workflow: &Workflow) -> Result<ValidationResult, ClientError>;
    pub async fn compile_workflow(&self, workflow: &Workflow) -> Result<CompiledWorkflow, ClientError>;
}
```

### 2.2 Graph Core Types

```rust
// Replaces Restate-specific types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    // FSM nodes
    FsmEntry(FsmEntryConfig),
    FsmTransition(FsmTransitionConfig),
    FsmState(FsmStateConfig),
    FsmFinal(FsmFinalConfig),
    
    // DAG nodes
    DagTask(DagTaskConfig),
    DagSplit(DagSplitConfig),
    DagJoin(DagJoinConfig),
    
    // Procedural nodes
    ProceduralStep(ProceduralStepConfig),
    ProceduralScript(ProceduralScriptConfig),
}

pub struct Workflow {
    pub id: WorkflowId,
    pub name: String,
    pub nodes: Vec<Node>,
    pub connections: Vec<Connection>,
    pub viewport: Viewport,
}
```

---

## 3. Feature Specifications

### 3.1 Canvas & Node Editor
- [ ] Drag-drop node placement from palette
- [ ] Connection drawing between ports
- [ ] Pan/zoom viewport
- [ ] Multi-select with rubber band
- [ ] Node search/filter
- [ ] Minimap navigation

### 3.2 Inspector Panel
- [ ] Dynamic form based on node type
- [ ] Expression input with validation
- [ ] Port configuration
- [ ] Metadata editor

### 3.3 Execution History & Time-Travel
- [ ] List past executions with status
- [ ] Execution detail view with journal
- [ ] Time-travel scrubber: slider to replay execution steps
- [ ] Step-by-step forward/backward navigation
- [ ] State diff at each step

### 3.4 Toolbar & Sidebar
- [ ] Mode toggle: Design / Simulate / Monitor
- [ ] Workflow save/load/export
- [ ] Undo/redo stack
- [ ] Validation panel
- [ ] Node palette with categories

### 3.5 Code Generator
- [ ] Generate Rust code from graph
- [ ] Generate workflow definition JSON
- [ ] Syntax highlighted preview
- [ ] Copy to clipboard

### 3.6 Linter Integration
- [ ] Real-time validation as graph changes
- [ ] Issue highlighting on nodes
- [ ] Fix suggestions

---

## 4. UI Components (from Oya, adapted)

| Oya Component | WTF Adaptation |
|--------------|-----------------|
| `RestateClient` | `WtfClient` |
| `restate_sync/*` | `wtf_client/*` |
| `RestateInvocation` | `WtfExecution` |
| Journal viewer | Execution history + time-travel |
| Restate node types | FSM/DAG/Procedural types |
| `run_code` node | Procedural script node |
| - | Code generator (new) |
| - | Time-travel scrubber (new) |

---

## 5. Postconditions

### Functional
- [ ] App compiles with `cargo build -p wtf-frontend`
- [ ] Dev server runs with `cargo run -p wtf-frontend`
- [ ] Canvas renders with placeholder nodes
- [ ] Inspector shows node config form
- [ ] Time-travel scrubber functional

### Non-Functional
- [ ] Zero `unwrap()` in graph core
- [ ] Clippy clean (`cargo clippy -p wtf-frontend`)
- [ ] All types documented

---

## 6. Error Taxonomy

```rust
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Request timeout")]
    Timeout,
    #[error("HTTP {status}: {message}")]
    HttpError { status: u16, message: String },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("JSON parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Cyclic dependency detected: {0}")]
    CyclicDependency(String),
    #[error("Missing required port: {node}.{port}")]
    MissingPort { node: NodeId, port: PortName },
    #[error("Invalid expression: {0}")]
    InvalidExpression(String),
}
```

---

## 7. Child Beads

| Priority | Bead | Title | Description |
|----------|------|-------|-------------|
| 1 | child-1 | wtf_client HTTP API | Replace RestateClient with WTF API client |
| 1 | child-2 | wtf-frontend graph core | Graph types for FSM/DAG/Procedural |
| 1 | child-3 | Canvas & node editor | Visual canvas with drag-drop |
| 2 | child-4 | Inspector panel | Node configuration UI |
| 2 | child-5 | Execution history + time-travel | Time-travel scrubber |
| 2 | child-6 | Toolbar & sidebar | App chrome |
| 3 | child-7 | Code generator | Generate code from graph |
| 3 | child-8 | Linter integration | Validation |
