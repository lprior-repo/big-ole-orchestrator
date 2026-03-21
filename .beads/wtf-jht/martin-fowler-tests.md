bead_id: wtf-jht
bead_title: "bead: Implement list_workflows handler"
phase: "STATE 2"
updated_at: "2026-03-20T23:45:00Z"

# Martin Fowler Test Plan

## Happy Path Tests

### test_returns_empty_list_when_no_workflows_running
Given: Orchestrator has no running workflow instances
When: list_workflows handler is called
Then: Returns 200 OK with ListWorkflowsResponse { workflows: [] }

### test_returns_workflows_list_when_workflows_running
Given: Orchestrator has 2 running workflow instances (wf1, wf2)
When: list_workflows handler is called
Then: Returns 200 OK with ListWorkflowsResponse containing 2 WorkflowInfo entries

## Error Path Tests

### test_returns_500_when_orchestrator_unavailable
Given: ActorRef<OrchestratorMsg> points to dead actor
When: list_workflows handler is called
Then: Returns 500 Internal Server Error

### test_returns_500_when_orchestrator_times_out
Given: Orchestrator is alive but doesn't respond to ListWorkflows message
When: list_workflows handler is called with timeout
Then: Returns 500 Internal Server Error

## Edge Case Tests

### test_returns_empty_list_with_correct_response_structure
Given: Orchestrator returns empty Vec
When: list_workflows handler is called
Then: Returns 200 OK with ListWorkflowsResponse { workflows: [] }
And: Response Content-Type is application/json

### test_workflow_info_structure_is_complete
Given: Orchestrator has one running workflow with invocation_id "inv1", name "test", status "running", started_at "2024-01-01T00:00:00Z"
When: list_workflows handler is called
Then: Returns 200 OK with WorkflowInfo { invocation_id: "inv1", workflow_name: "test", status: "running", started_at: "2024-01-01T00:00:00Z" }

## Contract Verification Tests

### test_precondition_master_actor_available
Given: Router is configured with Extension(master) layer
When: list_workflows is invoked
Then: master ActorRef is extractable from Extension

### test_postcondition_correct_response_type
Given: Valid OrchestratorMsg::ListWorkflows response
When: handler processes response
Then: Returns Json<ListWorkflowsResponse>

### test_invariant_read_only_operation
Given: Orchestrator has running workflows
When: list_workflows handler is called multiple times
Then: Orchestrator state remains unchanged (no side effects)

## Given-When-Then Scenarios

### Scenario 1: List workflows when orchestrator is healthy
Given: The master ActorRef is available and responsive
And: Orchestrator has running workflows: [{invocation_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "checkout", status: "running", started_at: "2024-01-15T10:30:00Z"}]
When: GET /api/v1/workflows is called
Then: 
- HTTP status is 200 OK
- Content-Type is application/json
- Response body is {"workflows": [{...}]}

### Scenario 2: List workflows when orchestrator is unreachable
Given: The master ActorRef is dead or unresponsive
When: GET /api/v1/workflows is called
Then:
- HTTP status is 500 Internal Server Error
- No response body (or generic error)
