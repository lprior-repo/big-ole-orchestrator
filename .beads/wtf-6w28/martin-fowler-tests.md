# Martin Fowler Test Plan

## Context
Bead: `wtf-6w28` - Design Mode deploy flow
Feature: Deploy button handler with validation, codegen, and API POST

## Happy Path Tests

### test_deploy_succeeds_with_valid_fsm_workflow
Given: A valid FSM workflow with entry point, states, and transitions, paradigm = Fsm
When: `deploy_handler()` is called
Then: Returns `DeployResult::Success { generated_code }` containing valid Rust FSM code

### test_deploy_succeeds_with_valid_dag_workflow
Given: A valid DAG workflow with entry, tasks, and fan-in/fan-out, paradigm = Dag
When: `deploy_handler()` is called
Then: Returns `DeployResult::Success { generated_code }` containing valid Rust DAG code

### test_deploy_succeeds_with_valid_procedural_workflow
Given: A valid Procedural workflow with steps, paradigm = Procedural
When: `deploy_handler()` is called
Then: Returns `DeployResult::Success { generated_code }` containing valid Rust procedural code

### test_generated_code_contains_correct_paradigm_marker
Given: A valid workflow with paradigm = Dag
When: `deploy_handler()` is called and returns success
Then: The `generated_code` contains DAG-specific constructs (e.g., `DagActor`)

## Error Path Tests

### test_deploy_fails_when_workflow_has_no_nodes
Given: An empty workflow with zero nodes
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` with message "Workflow has no nodes"

### test_deploy_fails_when_workflow_has_no_entry_point
Given: A workflow with nodes but no entry point (HTTP Handler, Kafka Handler, etc.)
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` containing entry point error

### test_deploy_fails_on_unreachable_nodes
Given: A workflow with unreachable nodes
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` with unreachable node warnings (deployment blocked)

### test_deploy_handles_422_response_with_lint_errors
Given: Server returns HTTP 422 with `{"errors": [{"code": "WTF-L001", "message": "..."}]}`
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors { errors: Vec<LintError> }` containing parsed WTF-L001

### test_deploy_handles_network_error
Given: Network connection refused or timeout
When: `deploy_handler()` is called
Then: Returns `DeployResult::Error { message: "Network error description" }`

### test_deploy_handles_http_500_error
Given: Server returns HTTP 500 Internal Server Error
When: `deploy_handler()` is called
Then: Returns `DeployResult::Error { message: "HTTP 500" }`

### test_deploy_handles_serialization_error
Given: WorkflowDefinition cannot be serialized to JSON
When: `post_definition()` is called
Then: Returns `DeployResult::Error { message: "Serialization failed" }`

## Edge Case Tests

### test_deploy_with_single_node_workflow
Given: A workflow with only one node (entry point only)
When: `deploy_handler()` is called
Then: Returns `DeployResult::Success` (valid minimal workflow)

### test_deploy_with_dag_having_unconnected_nodes
Given: A DAG workflow with unconnected nodes
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` with connectivity warnings

### test_deploy_with_cyclic_dag
Given: A DAG workflow that contains a cycle
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` with cycle detection error

### test_deploy_with_missing_required_node_config
Given: An HTTP Handler node without a path configured
When: `deploy_handler()` is called
Then: Returns `DeployResult::ValidationErrors` with "HTTP Handler requires a path" error

## Contract Verification Tests

### test_precondition_p1_empty_workflow_blocked
Given: Empty workflow
When: `deploy_handler()` is called
Then: Does NOT call `code_generator::generate()`
Then: Does NOT call `WtfClient::post_definition()`

### test_precondition_p4_empty_base_url_blocked
Given: WtfClient configured with empty base URL
When: `post_definition()` is called
Then: Returns `Err(DeployError::NetworkError("empty base URL"))`

### test_postcondition_q1_success_response_parsed
Given: Server returns HTTP 201 with `{"generated_code": "..."}`
When: `post_definition()` is called
Then: Returns `Ok(DeploySuccess { generated_code: "..." })`

### test_postcondition_q4_validation_blocks_deploy
Given: `validate_workflow()` returns errors
When: `deploy_handler()` is called
Then: Returns early with `DeployResult::ValidationErrors`
Then: Does NOT make any HTTP request

## Contract Violation Tests

### test_violates_p1_empty_workflow_returns_error
Given: `workflow.nodes.is_empty() == true`
When: `deploy_handler(&empty_workflow, Fsm, client)` is called
Then: Returns `Err(DeployError::ValidationFailed("Workflow has no nodes"))`
NOT: Panics, unwraps, or proceeds to network call

### test_violates_q4_validation_must_block_network_call
Given: `validate_workflow()` returns `ValidationResult { issues: [ValidationIssue { severity: Error, ... }] }`
When: `deploy_handler()` is executing
Then: `WtfClient::post_definition()` is NEVER called
Then: Returns `DeployResult::ValidationErrors { ... }`

## Given-When-Then Scenarios

### Scenario 1: Successful FSM Deploy
Given: A workflow with HTTP Handler entry, two FSM states, and FSM final state
And: All states have valid transitions configured
And: `paradigm = WorkflowParadigm::Fsm`
When: User clicks the Deploy button
Then: Validation passes with no errors
Then: Code generator produces FSM Rust code
Then: POST to `/definitions` returns 201
Then: UI shows success toast "Deployed successfully!"
Then: Generated code panel opens with syntax-highlighted Rust

### Scenario 2: Blocked Deploy Due to Validation Errors
Given: A workflow with unreachable nodes
And: User clicks the Deploy button
When: `deploy_handler()` is called
Then: Validation returns errors for unreachable nodes
Then: Deploy is blocked
Then: UI shows lint panel with WTF-Lxxx codes
Then: Offending nodes are highlighted in red on canvas
Then: Deploy button remains disabled

### Scenario 3: Server Rejects Deploy with Lint Errors
Given: User clicks Deploy button
And: Local validation passes
When: Server returns 422 with lint errors
Then: `deploy_handler()` parses `LintError` array
Then: UI shows lint panel with WTF-Lxxx codes
Then: Offending nodes are highlighted in red on canvas
Then: Deploy button remains disabled until errors are fixed

### Scenario 4: Network Failure During Deploy
Given: User clicks Deploy button
And: Local validation passes
When: Network request times out
Then: `deploy_handler()` returns `DeployResult::Error { message: "Request timed out" }`
Then: UI shows error toast with the message
Then: User can retry the deploy
