## Contract: Template Rendering Engine

### 1. Purpose

Defines the contract for rendering workflow node templates in the veloxide Dioxus UI. This contract establishes the types, invariants, and error taxonomy for the template rendering subsystem that powers the prototype palette, command palette, and node visualization.

### 2. Source ADRs

- `docs/adr/v2/ADR-004-v2-code-as-workflow-sdk.md` (code-as-workflow paradigm)
- `docs/adr/v2/ADR-031-v2-canonical-workflow-spec-sdk-ui.md` (canonical WorkflowSpec)
- `docs/adr/v2/ADR-007-v2-dioxus-observability-ui.md` (Dioxus UI conventions)

### 3. Template Types

#### 3.1 NodeTemplateId

The canonical identifier for all workflow node templates.

```
NodeTemplateId {
  HttpHandler,       // HTTP or gRPC request handler
  KafkaHandler,      // Kafka event consumer
  CronTrigger,       // Scheduled periodic trigger
  WorkflowSubmit,    // Start another workflow instance
  Run,               // Durable persisted side effect step
  ServiceCall,       // Request-response service invocation
  ObjectCall,        // Virtual object handler invocation
  SendMessage,       // Fire-and-forget one-way call
  GetState,          // Read persisted state
  SetState,          // Write persisted state
  Condition,         // Conditional branching (If/Else)
  Parallel,          // Concurrent branch execution
  Timer,             // Durable pause/wait
  Timeout,           // Step deadline guard
}
```

#### 3.2 SketchNode

A node instantiated from a template for prototyping.

```
SketchNode {
  node_type: NodeTemplateId,
  label: String,
}
```

#### 3.3 PaletteEntry

A renderable entry in the prototype palette.

```
PaletteEntry {
  node_type: NodeTemplateId,
  icon: &'static str,
}
```

#### 3.4 CommandTemplate

A filterable command template for the command palette.

```
CommandTemplate {
  node_type: NodeTemplateId,
  label: String,
  hint: String,
}
```

### 4. Template Metadata

#### 4.1 TemplateDescriptor

Each `NodeTemplateId` has immutable metadata:

```
TemplateDescriptor {
  id: NodeTemplateId,
  as_str: &'static str,    // URL-safe identifier (e.g., "http-handler")
  label: &'static str,     // Human-readable name (e.g., "HTTP Handler")
  hint: &'static str,      // One-line description
}
```

#### 4.2 Template Category

Templates are grouped by execution semantics:

```
enum TemplateCategory {
  Ingress,      // HttpHandler, KafkaHandler, CronTrigger
  Execution,    // Run, ServiceCall, ObjectCall, SendMessage
  State,        // GetState, SetState
  Control,      // Condition, Parallel, Timer, Timeout
  Workflow,     // WorkflowSubmit
}
```

### 5. Invariants (INV-*)

- **INV-001**: `NodeTemplateId::all()` returns exactly 14 templates
- **INV-002**: Each `NodeTemplateId` variant maps to a unique `as_str` string
- **INV-003**: `NodeTemplateId::from_str` is the inverse of `as_str` for all valid IDs
- **INV-004**: Template `label()` and `hint()` are non-empty for all variants
- **INV-005**: `SketchNode` label defaults to `node_type.label()`
- **INV-006**: `generate_skeleton` produces valid YAML with sequential `step-N` IDs
- **INV-007**: `generate_skeleton` emits `depends_on` only for nodes after the first
- **INV-008**: Palette entries render without panics for all `NodeTemplateId` variants
- **INV-009**: `filtered_templates` is case-insensitive on query matching
- **INV-010**: `filtered_templates` returns all templates when query is empty

### 6. Error Taxonomy

```rust
enum TemplateError {
    ParseError {
        input: String,
        expected: &'static str,
    },
    ValidationError {
        template_id: NodeTemplateId,
        violation: ValidationViolation,
    },
    RenderError {
        template_id: NodeTemplateId,
        context: RenderContext,
    },
    SerializationError {
        reason: SerializationReason,
    },
}

enum ValidationViolation {
    MissingRequiredField(String),
    InvalidTemplateCombination(Vec<NodeTemplateId>),
    CircularDependency,
}

enum RenderContext {
    Palette,
    CommandPalette,
    Canvas,
    Inspector,
}

enum SerializationReason {
    YamlEncodeError(String),
    JsonEncodeError(String),
    EmptySketch,
}
```

### 7. Template Filtering Protocol

#### 7.1 Filter Query Grammar

```
query := term*
term  := word | word word*   // whitespace-separated tokens
word  := LETTER+             // case-insensitive match against label OR hint OR id
```

#### 7.2 Filter Algorithm

1. Tokenize query into lowercase terms
2. For each template, score by match count across `label`, `hint`, and `as_str`
3. Return templates with score > 0, sorted by descending score
4. Empty query returns all templates

### 8. Skeleton Generation Protocol

#### 8.1 Input

`Vec<SketchNode>` in palette order

#### 8.2 Output Format

```yaml
name: "prototype-workflow"
steps:
  - id: step-1
    type: {node_type.as_str}
    config: {}
  - id: step-2
    type: {node_type.as_str}
    depends_on: [step-1]
    config: {}
```

#### 8.3 Constraints

- First node has no `depends_on`
- Each subsequent node depends on all prior nodes
- `config` is always `{}` in prototype mode

### 9. Constraints

- Template metadata is compile-time constant; no runtime fetching
- Skeleton generation is pure; no side effects
- Palette rendering must handle all 14 template types
- Filter queries must complete in O(n) where n = template count
- Empty sketch produces header-only YAML with no step entries

### 10. Relevant Files

- `crates/vo-frontend/src/ui/domain_types.rs` (NodeTemplateId, HandleKind)
- `crates/vo-frontend/src/ui/prototype_palette.rs` (PaletteEntry, generate_skeleton)
- `crates/vo-frontend/src/ui/command_palette.rs` (CommandTemplate, filtered_templates)
- `crates/vo-frontend/src/ui/panel_types.rs` (InvocationStatus, HttpMethod, PayloadShape)
- `crates/vo-frontend/src/ui/edges/rendering.rs` (SVG defs and parallel group rendering)

### 11. Acceptance Criteria

- NodeTemplateId enum has exactly 14 variants covering all workflow node types
- Template metadata (as_str, label, hint) is exhaustive for all variants
- filtered_templates is case-insensitive and matches on label, hint, and id
- generate_skeleton produces syntactically valid YAML for any non-empty sketch
- generate_skeleton produces header-only output for empty sketch
- All template rendering paths return Result or Option, never panic
- Error taxonomy covers parse, validation, render, and serialization failures
- Contract document references only existing files and ADRs