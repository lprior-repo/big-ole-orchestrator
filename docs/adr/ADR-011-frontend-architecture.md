# ADR-011: Frontend Architecture (Oya Fork)

## Status

Superseded by ADR-018 (Dioxus Compiler Control Plane)

The Oya fork approach is abandoned. The frontend is rebuilt from scratch in Dioxus (Rust). This is not a downgrade — it is a fundamental architectural elevation: the Dioxus application is a *compiler*, not just a dashboard. It traverses user-designed workflow graphs and emits deterministic Rust source code (state enums, event enums, reducer match arms) that the engine executes natively. The real-time monitor still exists (live overlays on the same graph, event log timeline, time-travel scrubber) but is implemented via NATS KV watch proxied through an axum SSE endpoint, not HTTP polling. See ADR-018 for the full design.

## Context

wtf-engine needs a **visual workflow editor** for:

1. **Coders** - Visualize workflows, debug execution, view journal
2. **Non-coders** - Build workflows without writing code
3. **Both** - Same UI, different entry points

We considered:

| Approach | Pros | Cons |
|----------|------|------|
| **Fork Oya** | Full workflow editor, mature UI | Restate-specific, needs adaptation |
| **Build fresh** | Clean slate, less tech debt | Months of work |
| **Use Temporal UI** | Proven design | Not Rust, different patterns |

### Oya Frontend Analysis

Oya frontend provides:

- **Canvas-based workflow editor** with drag-and-drop
- **24 node types** (Restate-specific)
- **Execution visualization** with state badges
- **Inspector panel** for node configuration
- **History panel** for run records

## Decision

We will **fork Oya frontend** and adapt it for wtf-engine.

### Fork Strategy

1. **Copy** oya-frontend → wtf-frontend
2. **Replace** Restate client with wtf-api client
3. **Adapt** node types for wtf-engine (Step Functions parity)
4. **Extend** with journal viewer and execution history
5. **Keep** canvas, node editor, inspector infrastructure

### Module Structure

```
wtf-frontend/
├── src/
│   ├── ui/
│   │   ├── canvas.rs          # Main canvas component
│   │   ├── toolbar.rs         # Header with workflow name
│   │   ├── sidebar.rs         # Node palette
│   │   ├── inspector.rs       # Node configuration
│   │   ├── history_panel.rs   # Execution history
│   │   └── journal_viewer.rs  # Journal viewer (NEW)
│   │
│   ├── graph/
│   │   ├── workflow_node.rs   # Node types (adapt for Step Functions)
│   │   ├── execution_state.rs # State machine
│   │   ├── dag.rs            # petgraph integration (NEW)
│   │   └── validation.rs      # Workflow validation
│   │
│   ├── wtf_client/
│   │   ├── client.rs          # HTTP client for wtf-api
│   │   ├── types.rs          # API types
│   │   └── queries.rs        # Query builders
│   │
│   ├── codegen/
│   │   ├── generator.rs       # Graph → Rust code (NEW)
│   │   └── templates.rs       # Code templates
│   │
│   └── lib.rs
│
├── Cargo.toml
└── dist/                      # Built WASM
```

### Key Changes from Oya

| Component | Oya | wtf-engine |
|-----------|-----|------------|
| Client | Restate SDK | wtf-api HTTP |
| Node types | Restate-specific | Step Functions |
| State persistence | Restate state | wtf-storage |
| Journal | Restate journal | wtf journal |
| Execution | Local simulation | wtf-engine backend |

### Node Type Mapping

```rust
// Oya nodes → wtf-engine nodes
Oya: HttpHandler, KafkaHandler, CronTrigger, WorkflowSubmit
wtf: HttpTrigger, CronTrigger (simpler trigger model)

Oya: Run, ServiceCall, ObjectCall, WorkflowCall
wtf: Task (activity invocation)

Oya: GetState, SetState, ClearState
wtf: GetState, SetState (simplified)

Oya: Condition, Switch, Loop, Parallel, Compensate
wtf: Choice, Map, Parallel (Step Functions names)

Oya: Sleep, Timeout
wtf: Wait (max 24h), Timeout

Oya: DurablePromise, Awakeable, ResolvePromise, SignalHandler
wtf: WaitForSignal (simplified)
```

### Journal Viewer (New Component)

```rust
#[component]
pub fn JournalViewer(cx: Scope, invocation_id: String) -> Element {
    let journal = use_future(cx, (), |_| async {
        wtf_client::get_journal(&invocation_id).await
    });

    match journal.value() {
        Some(Ok(entries)) => rsx! {
            div { class: "journal-viewer" }
            for entry in entries {
                JournalEntryRow { entry: entry }
            }
        },
        Some(Err(e)) => rsx! { "Error: {e}" },
        None => rsx! { "Loading..." },
    }
}
```

### Frontend ↔ Backend Communication

```rust
// wtf-client/src/client.rs
pub struct WtfClient {
    base_url: String,
    http: reqwest::Client,
}

impl WtfClient {
    pub async fn get_journal(&self, invocation_id: &str) -> Result<Vec<JournalEntry>> {
        let url = format!("{}/api/v1/workflows/{}/journal", self.base_url, invocation_id);
        let response = self.http.get(&url).send().await?;
        let body: JournalResponse = response.json().await?;
        Ok(body.entries)
    }

    pub async fn start_workflow(&self, name: &str, input: serde_json::Value) -> Result<String> {
        let url = format!("{}/api/v1/workflows", self.base_url);
        let body = serde_json::json!({
            "workflow_name": name,
            "input": input,
        });
        let response = self.http.post(&url).json(&body).send().await?;
        let result: StartResponse = response.json().await?;
        Ok(result.invocation_id)
    }
}
```

## Consequences

### Positive

- **Months of work saved** - Full workflow editor
- **Proven UX** - Oya is used in production
- **Iterative adaptation** - Can adapt piecemeal

### Negative

- **Tech debt** - Inheriting Oya's patterns
- **Adaptation work** - Replacing Restate-specific code
- **Bundle size** - WASM + all UI components

### Future Considerations

- Consider rewriting in TypeScript for wider contributor base
- Consider adding collaborative editing (CRDT-based)
