# Martin Fowler Tests: wtf-frontend (Phase 5 — Frontend)

## Epic: Phase 5 — Frontend

**Bead ID:** wtf-qnol  
**Feature:** Dioxus workflow compiler and live monitor with canvas, node editor, inspector, execution history, code generator, and time-travel scrubber.

---

## Test Scenarios

### Theme 1: Client Connection

#### Scenario: Connect to WTF API
- **Given** the wtf-frontend application is running
- **And** the WTF API server is available at the configured endpoint
- **When** the user opens the application
- **Then** the client establishes connection to `/api/v1/workflows`
- **And** the connection status indicator shows "Connected"
- **And** any existing workflows are loaded into the sidebar

#### Scenario: Handle API unavailability
- **Given** the WTF API server is not running
- **When** the user opens the application
- **Then** the client shows "Disconnected" status
- **And** a banner displays "Cannot connect to API"
- **And** the user can still interact with local workflow data

#### Scenario: Request timeout recovery
- **Given** the client has sent a request to the API
- **When** the request times out after 10 seconds
- **Then** the client displays a timeout error
- **And** the user can retry the operation

---

### Theme 2: Workflow Canvas

#### Scenario: Create new workflow
- **Given** the user is in Design mode
- **When** the user clicks "New Workflow"
- **Then** a new empty canvas appears
- **And** the viewport is centered at origin
- **And** the workflow name is "Untitled Workflow"

#### Scenario: Add node from palette
- **Given** the user has an empty canvas
- **When** the user drags a "DagTask" node from the palette
- **And** drops it on the canvas
- **Then** a new node appears at the drop position
- **And** the node is selected
- **And** the inspector shows the node configuration

#### Scenario: Connect two nodes
- **Given** there are two unconnected nodes on the canvas
- **When** the user drags from the output port of node A
- **And** drops on the input port of node B
- **Then** a directed edge is drawn from A to B
- **And** the connection is saved to the workflow

#### Scenario: Delete node with connections
- **Given** node A is connected to node B
- **When** the user selects node A and presses Delete
- **Then** node A is removed
- **And** all connections to/from node A are removed
- **And** node B remains unchanged

#### Scenario: Pan and zoom viewport
- **Given** there are many nodes on the canvas
- **When** the user scrolls the mouse wheel
- **Then** the canvas zooms in/out
- **When** the user middle-clicks and drags
- **Then** the canvas pans in the drag direction

#### Scenario: Multi-select with rubber band
- **Given** there are multiple nodes on the canvas
- **When** the user clicks and drags a rubber band selection
- **Then** all nodes within the rubber band are selected
- **And** the selection can be moved as a group

---

### Theme 3: Node Configuration

#### Scenario: Configure FSM entry node
- **Given** there is an FsmEntry node on the canvas
- **When** the user selects the node
- **Then** the inspector shows:
  - Name field (prefilled with "entry")
  - Initial state dropdown
- **When** the user sets the initial state to "waiting"
- **And** saves the configuration
- **Then** the node's config is updated
- **And** the node label displays the configured name

#### Scenario: Configure DAG task node
- **Given** there is a DagTask node on the canvas
- **When** the user selects the node
- **Then** the inspector shows:
  - Name field
  - Command/expression input
  - Timeout field
  - Retry policy
- **When** the user enters a shell command
- **And** sets timeout to 30 seconds
- **Then** the node is configured correctly

#### Scenario: Expression validation
- **Given** the inspector has an expression input
- **When** the user types an invalid expression
- **Then** a red underline appears
- **And** a tooltip shows "Invalid expression syntax"
- **When** the user hovers over the expression
- **Then** a suggestion to fix appears

---

### Theme 4: Execution History & Time-Travel

#### Scenario: View execution history
- **Given** a workflow has been executed before
- **When** the user clicks on the History panel
- **Then** a list of past executions appears
- **And** each entry shows: timestamp, duration, status (success/failure)
- **When** the user clicks on an execution
- **Then** the execution detail view opens
- **And** the canvas highlights nodes that were executed

#### Scenario: Time-travel to specific step
- **Given** the user is viewing an execution record
- **When** the user drags the time-travel scrubber to step 5
- **Then** the canvas shows the workflow state at step 5
- **And** the node that executed at step 5 is highlighted
- **And** the output of that step is displayed in the inspector

#### Scenario: Step-by-step playback
- **Given** the user is viewing an execution record
- **When** the user clicks the "Play" button
- **Then** the execution replays step by step
- **With** a 1-second delay between steps
- **When** the user clicks "Pause"
- **Then** the playback stops at the current step

#### Scenario: Navigate forward/backward
- **Given** the user is viewing an execution at step 3
- **When** the user clicks the "Next" button
- **Then** the view advances to step 4
- **When** the user clicks the "Previous" button
- **Then** the view goes back to step 3

#### Scenario: State diff between steps
- **Given** the user is viewing an execution at step 5
- **When** the user opens the "State Diff" panel
- **Then** it shows the changes between step 4 and step 5
- **And** added keys are highlighted in green
- **And** removed keys are highlighted in red

---

### Theme 5: Code Generator

#### Scenario: Generate Rust code from graph
- **Given** the user has designed a workflow on the canvas
- **When** the user clicks "Generate Code"
- **Then** a modal opens with the generated Rust code
- **And** the code includes the workflow struct and node implementations
- **When** the user clicks "Copy"
- **Then** the code is copied to the clipboard
- **And** a toast notification confirms "Copied to clipboard"

#### Scenario: Generate workflow JSON
- **Given** the user has designed a workflow
- **When** the user selects "Export JSON"
- **Then** a JSON representation of the workflow is generated
- **And** a download dialog opens

---

### Theme 6: Validation & Linter

#### Scenario: Detect cyclic dependency in DAG
- **Given** the user has created a DAG with nodes A → B → C
- **When** the user connects node C back to node A
- **Then** a validation error appears: "Cyclic dependency detected"
- **And** the problematic edge is highlighted in red
- **And** the edge is not saved

#### Scenario: Detect missing required port
- **Given** there is a node that requires an input port "trigger"
- **When** the node has no incoming connection
- **Then** a warning appears: "Missing required port: node.trigger"
- **And** the node shows a warning indicator

#### Scenario: Real-time validation
- **Given** the user is editing a node configuration
- **When** the configuration becomes invalid
- **Then** the validation runs automatically
- **And** issues appear immediately in the Validation panel
- **And** affected nodes are highlighted

---

### Theme 7: Toolbar & Sidebar

#### Scenario: Mode toggle
- **Given** the user is in Design mode
- **When** the user toggles to "Monitor" mode
- **Then** the canvas becomes read-only
- **And** live execution data is displayed
- **When** the user toggles to "Simulate" mode
- **Then** the canvas is editable
- **And** a simulated execution can be run

#### Scenario: Undo/Redo
- **Given** the user has made changes to the canvas
- **When** the user presses Ctrl+Z
- **Then** the last change is undone
- **When** the user presses Ctrl+Shift+Z
- **Then** the change is redone

#### Scenario: Search nodes
- **Given** the sidebar has a search box
- **When** the user types "DagTask"
- **Then** the node palette filters to show only matching nodes

---

### Theme 8: Error Handling

#### Scenario: Display connection error
- **Given** the client fails to connect to the API
- **When** the error occurs
- **Then** an error toast is displayed with the message
- **And** the error is logged to the console

#### Scenario: Handle invalid workflow response
- **Given** the API returns an invalid workflow JSON
- **When** the client parses the response
- **Then** a parse error is logged
- **And** the user sees "Failed to load workflow"

#### Scenario: Graceful degradation
- **Given** the API is unavailable
- **When** the user tries to save the workflow
- **Then** the workflow is saved locally
- **And** a sync indicator shows "Saved locally"
- **And** when the API becomes available, the workflow is synced

---

## Test Data Fixtures

### Workflow JSON
```json
{
  "id": "wf-123",
  "name": "Test Workflow",
  "nodes": [
    {
      "id": "node-1",
      "type": "fsm_entry",
      "name": "start",
      "config": { "initial_state": "idle" }
    }
  ],
  "connections": []
}
```

### Execution Record
```json
{
  "execution_id": "exec-456",
  "workflow_id": "wf-123",
  "started_at": "2026-03-21T10:00:00Z",
  "completed_at": "2026-03-21T10:05:00Z",
  "status": "success",
  "steps": [
    { "step": 1, "node_id": "node-1", "output": { "entered": true } }
  ]
}
```
