# Martin Fowler Test Plan: wtf-frontend graph core (wtf-d7kw)

## Happy Path Tests

### test_fsm_entry_node_creation
Given: A valid workflow with no nodes
When: Creating an FsmEntry node and adding it to workflow
Then: Node is added with correct NodeType::FsmEntry variant

### test_fsm_state_node_creation
Given: A valid workflow with an FsmEntry node
When: Creating an FsmState node and adding it to workflow
Then: Node is added with correct NodeType::FsmState variant

### test_fsm_transition_connection
Given: Workflow with FsmEntry and FsmState nodes
When: Adding a connection Entry→State
Then: Connection is created successfully

### test_dag_task_node_creation
Given: A valid empty workflow
When: Creating a DagTask node and adding it
Then: Node is added with correct NodeType::DagTask variant

### test_dag_split_fans_out_to_multiple_tasks
Given: Workflow with DagSplit node and 3 DagTask nodes
When: Adding connections Split→Task1, Split→Task2, Split→Task3
Then: All connections created, fan-out pattern validated

### test_dag_join_collects_from_multiple_tasks
Given: Workflow with 3 DagTask nodes and one DagJoin node
When: Adding connections Task1→Join, Task2→Join, Task3→Join
Then: All connections created, fan-in pattern validated

### test_procedural_step_sequence
Given: A workflow with multiple ProceduralStep nodes
When: Connecting them sequentially Step1→Step2→Step3
Then: Linear execution path established

### test_workflow_serialization_roundtrip
Given: A workflow with mixed node types (FSM, DAG, Procedural)
When: Serializing to JSON and deserializing back
Then: All node types, connections, and metadata preserved

## Error Path Tests

### test_returns_error_when_fsm_transition_from_final_to_entry
Given: Workflow with FsmFinal node
When: Attempting to create transition Final→Entry
Then: Returns `Err(Error::InvalidStateTransition)`

### test_returns_error_when_cycle_detected_in_dag
Given: Workflow with TaskA→TaskB→TaskC
When: Adding connection C→A (creates cycle)
Then: Returns `Err(Error::CycleDetected)`

### test_returns_error_when_node_name_is_empty
Given: Valid workflow
When: Creating node with empty name ""
Then: Returns `Err(Error::EmptyNodeName)`

### test_returns_error_when_connection_references_nonexistent_node
Given: Workflow with one node (id: node1)
When: Adding connection from node1 to node2 (doesn't exist)
Then: Returns `Err(Error::NodeNotFound)`

### test_returns_error_when_port_name_invalid
Given: Workflow with a node that has ports ["input", "output"]
When: Adding connection with port "invalid_port"
Then: Returns `Err(Error::InvalidPortName)`

### test_returns_error_when_time_travel_cursor_out_of_bounds
Given: ExecutionState with 5 history entries (indices 0-4), cursor at 4
When: Calling jump_to_cursor(cursor: 10)
Then: Returns `Err(Error::TimeTravelBounds)`

### test_returns_error_when_duplicate_node_id_added
Given: Workflow with node having id: abc
When: Adding another node with same id: abc
Then: Returns `Err(Error::DuplicateNodeId)`

## Edge Case Tests

### test_handles_single_node_workflow
Given: Workflow with only one FsmEntry node (no connections)
When: Validating workflow
Then: Returns Ok (single entry node is valid)

### test_handles_dag_with_single_task
Given: Workflow with DagSplit connected to single DagTask
When: Validating DAG structure
Then: Returns Ok

### test_handles_procedural_script_empty_body
Given: ProceduralScript node with no child steps
When: Adding to workflow
Then: Node is created, validation passes

### test_handles_fsm_with_multiple_states
Given: FsmEntry → State1 → State2 → State3 → FsmFinal
When: Validating state machine
Then: All transitions valid, returns Ok

### test_handles_mixed_fsm_and_dag_in_single_workflow
Given: FSM for workflow orchestration, DAG for parallel task execution
When: Creating complex workflow
Then: Both patterns coexist, validation passes

## Contract Verification Tests

### test_precondition_node_type_exhaustiveness
Given: NodeType enum with 9 variants
When: Matching on all variants without wildcard
Then: Code compiles only if all variants handled (compile-time check)

### test_precondition_valid_state_transitions
Given: FSM states Entry, State, Transition, Final
When: Calling fsm_transition with invalid pair
Then: Returns appropriate error (not panic)

### test_precondition_dag_acyclicity
Given: DAG nodes and connections
When: Adding connection that would create cycle
Then: Returns Err(Error::CycleDetected)

### test_postcondition_node_creation_preserves_identity
Given: New node with specific id and name
When: Adding to workflow
Then: node.id and node.name preserved exactly

### test_postcondition_connection_graph_integrity
Given: Empty workflow
When: Adding connections
Then: All connections appear in workflow.connections

### test_invariant_all_connections_reference_valid_nodes
Given: Workflow with nodes [A, B, C] and connections [A→B, B→C]
When: Checking invariant
Then: All source/target node IDs found in nodes list

### test_invariant_fsm_entry_has_no_incoming
Given: FsmEntry node in workflow
When: Checking incoming connections
Then: Entry has zero incoming connections

### test_invariant_fsm_final_has_no_outgoing
Given: FsmFinal node in workflow
When: Checking outgoing connections
Then: Final has zero outgoing connections

## Contract Violation Tests

### test_fsm_transition_violation_returns_invalid_state_transition
Given: FsmFinal node exists in workflow
When: Attempting transition from Final to Entry
Then: Returns `Err(Error::InvalidStateTransition)`, not panic

### test_cycle_violation_returns_cycle_detected
Given: DAG with TaskA → TaskB → TaskC
When: Adding connection TaskC → TaskA
Then: Returns `Err(Error::CycleDetected)`, not panic

### test_empty_name_violation_returns_empty_node_name
Given: Attempting to create Node { name: "" }
When: Calling add_node with empty name
Then: Returns `Err(Error::EmptyNodeName)`, not panic

### test_nonexistent_node_violation_returns_node_not_found
Given: Workflow with node1 only
When: Connection from node1 → node2 (doesn't exist)
Then: Returns `Err(Error::NodeNotFound)`, not panic

### test_invalid_port_violation_returns_invalid_port_name
Given: Node with ports ["in", "out"]
When: Connection uses port "nonexistent"
Then: Returns `Err(Error::InvalidPortName)`, not panic

### test_time_travel_bounds_violation_returns_time_travel_bounds
Given: ExecutionState with history size 5
When: jump_to_cursor(100) (beyond bounds)
Then: Returns `Err(Error::TimeTravelBounds)`, not panic

### test_duplicate_id_violation_returns_duplicate_node_id
Given: Node with id: abc exists
When: add_node with another node id: abc
Then: Returns `Err(Error::DuplicateNodeId)`, not panic

## Given-When-Then Scenarios

### Scenario 1: Building a Complete FSM Workflow
Given: Empty workflow canvas
When: User adds FsmEntry → FsmState (x2) → FsmTransition → FsmFinal and connects them
Then:
- All 5 nodes appear in workflow.nodes
- 4 connections created (Entry→State1, State1→State2, State2→Transition, Transition→Final)
- FSM invariant validation passes (Entry has no incoming, Final has no outgoing)

### Scenario 2: Building a Parallel DAG Task
Given: Empty workflow
When: User creates DagSplit → [DagTask A, DagTask B, DagTask C] → DagJoin
Then:
- Split node has 3 outgoing connections
- Join node has 3 incoming connections
- DAG acyclicity validation passes
- No cycles detected

### Scenario 3: Time-Travel Execution Playback
Given: Workflow that has executed 10 steps with history
When: User scrubs cursor to position 5
Then:
- ExecutionState reflects state at step 5
- No data loss on cursor movement
- Future history (steps 6-10) preserved for redo

### Scenario 4: Mixed Workflow with FSM Orchestration and DAG Execution
Given: Complex workflow design
When: User creates:
  - FSM for top-level: Entry → State(Execute Tasks) → Final
  - DAG for parallel: State contains DagSplit → [Task A, Task B] → DagJoin
Then:
- Both FSM and DAG structures valid
- FSM invariants hold (single entry/exit)
- DAG invariants hold (no cycles, proper fan-in/fan-out)
- Complete workflow is connected graph
