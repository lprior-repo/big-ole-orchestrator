# Contract Specification

## Context
- Feature: `wtf-frontend: Design Mode deploy flow — codegen + lint display + POST to API`
- Domain terms:
  - `WorkflowParadigm`: Enum with variants `Fsm`, `Dag`, `Procedural`
  - `WorkflowDefinition`: Serialized form containing paradigm, graph JSON, and generated code
  - `GeneratedCode`: Output from code_generator containing FSM/DAG/Procedural codegen beads
  - `LintError`: Validation error with code (WTF-Lxxx format) and message
  - `ValidationResult`: Contains issues (errors/warnings) from graph validation
  - `WtfClient`: HTTP client for posting definitions to the API
- Assumptions:
  - `design_mode.rs` module does not exist yet and must be created
  - `code_generator` module does not exist yet and must be created
  - `WtfClient::post_definition()` takes `WorkflowDefinition` and returns `Result<(), DeployError>`
  - `code_generator::generate()` takes `(workflow_graph: &Workflow, paradigm: WorkflowParadigm)` and returns `GeneratedCode`
  - Graph validation via `validate_workflow()` returns `ValidationResult`
- Open questions:
  - What is the exact format of `GeneratedCode`? (Assuming Rust source code as string for now)
  - Should `LintError` codes follow a specific format? (Using WTF-Lxxx as stated in description)

## Preconditions
- [P1] `deploy_handler()` may only be called when a workflow graph is present and non-empty
- [P2] `deploy_handler()` requires a valid `WorkflowParadigm` value (Fsm, Dag, or Procedural)
- [P3] `code_generator::generate()` requires a valid `WorkflowParadigm` that matches the graph structure
- [P4] `WtfClient::post_definition()` requires a non-null base URL configuration

## Postconditions
- [Q1] On successful deploy (HTTP 201), `deploy_handler()` returns `DeployResult::Success { generated_code: String }`
- [Q2] On HTTP 422 response, `deploy_handler()` returns `DeployResult::ValidationErrors { errors: Vec<LintError> }`
- [Q3] On network or HTTP error (non-201, non-422), `deploy_handler()` returns `DeployResult::Error { message: String }`
- [Q4] If `validate_workflow()` returns `ValidationResult` with errors, deploy is blocked and `DeployResult::ValidationErrors` is returned before any network call
- [Q5] `GeneratedCode` output is correctly serialized into `WorkflowDefinition` before POST

## Invariants
- [I1] Deploy button remains disabled in UI until all validation errors are resolved
- [I2] `WorkflowDefinition.paradigm` always matches the paradigm used for code generation
- [I3] `WorkflowDefinition.graph_json` is a valid JSON representation of the workflow graph

## Error Taxonomy
- `DeployError::ValidationFailed` - Graph validation found errors before deploy attempt
- `DeployError::LintErrors(Vec<LintError>)` - Server returned 422 with lint errors
- `DeployError::NetworkError(String)` - HTTP request failed (timeout, connection refused, etc.)
- `DeployError::SerializationError(String)` - Failed to serialize WorkflowDefinition to JSON
- `DeployError::CodegenError(String)` - Code generation failed for the given paradigm/graph

## Contract Signatures
```rust
// Deploy button handler
pub async fn deploy_handler(
    workflow: &Workflow,
    paradigm: WorkflowParadigm,
    client: &WtfClient,
) -> DeployResult;

// Validation check (blocking)
pub fn validate_before_deploy(workflow: &Workflow) -> ValidationResult;

// Code generation
pub fn generate_code(
    workflow: &Workflow,
    paradigm: WorkflowParadigm,
) -> Result<GeneratedCode, CodegenError>;

// POST to API
pub async fn post_definition(
    client: &WtfClient,
    definition: WorkflowDefinition,
) -> Result<DeploySuccess, DeployError>;
```

## Type Encoding
| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| P1: Non-empty workflow graph | Runtime-checked | `if workflow.nodes.is_empty()` guard returns `DeployError::ValidationFailed` |
| P2: Valid paradigm | Compile-time | `enum WorkflowParadigm { Fsm, Dag, Procedural }` - already exhaustive |
| P3: Paradigm matches graph | Runtime-checked | Separate validation functions per paradigm, returns `DeployError::CodegenError` on mismatch |
| P4: Valid base URL | Runtime-checked | `WtfClient::new()` takes `&str`, empty string returns error variant |

## Violation Examples (REQUIRED)
- VIOLATES P1: `deploy_handler(&empty_workflow, paradigm, client)` -- should produce `Err(DeployError::ValidationFailed("Workflow has no nodes"))`
- VIOLATES P2: N/A - paradigm is enum, cannot be invalid
- VIOLATES P4: `WtfClient::new("").post_definition(definition)` -- should produce `Err(DeployError::NetworkError("empty base URL"))`
- VIOLATES Q1: Server returns 500 instead of 201 -- should produce `Err(DeployError::NetworkError("HTTP 500"))`
- VIOLATES Q4: `validate_workflow()` returns errors but `deploy_handler()` proceeds to network call -- MUST NOT happen

## Ownership Contracts (Rust-specific)
- `deploy_handler(&workflow, paradigm, client)` -- borrows `workflow` immutably, borrows `client` immutably, no ownership transfer
- `post_definition(client, definition)` -- takes ownership of `definition`, client is borrowed
- `generate_code(workflow, paradigm)` -- borrows `workflow` immutably, paradigm is copied

## Non-goals
- [ ] Actually executing the generated code
- [ ] Persisting the generated code to disk
- [ ] Running the deployed workflow
