# ADR 004 (v2): Code-as-Workflow (Rust SDK Definition)

## Status
Accepted

## Context
v1 assumed workflows would be defined as raw JSON documents adhering to the AWS Step Functions schema. 

While JSON is excellent for serialization and UI rendering, it is "dead configuration" for developers. Writing complex Directed Acyclic Graphs (DAGs) in raw JSON is error-prone, lacks compile-time safety, and prevents dynamic graph generation (e.g., loops).

## Decision
We adopt the "Code-as-Workflow" paradigm. Workflows are defined strictly in Rust code using the `vo-sdk`, rather than standalone JSON files.

### The Fluent Builder
Developers use the SDK in their `main.rs` to define the DAG programmatically:
```rust
let mut engine = vo_engine::Engine::new();

engine.register_workflow("checkout_flow", || {
    vo_sdk::Dag::new()
        .add_node("validate", BinaryNode::new("validate_cart"))
        .add_node("charge", BinaryNode::new("charge_stripe").retries(3))
        .connect("validate", "charge")
});
```

### The UI and AI Bridge
1. **The Compilation:** When the application compiles and runs, the SDK internally parses the Rust graph definition and generates the static DAG representation in memory.
2. **The Serialization:** The Engine automatically serializes this in-memory representation into the JSON format expected by the Dioxus UI. 
3. **The Dioxus UI:** The visual interface queries the Engine via an API to get the serialized JSON to render the glowing node graph. 
4. **The No-Code/AI Loop:** If a user builds a new graph in the UI, the UI/AI agent generates the equivalent `Dag::new()...` Rust code, overwrites the `main.rs` file, and triggers a recompilation. 

## Consequences
- **Positive:** Rust compiler catches broken edges, missing nodes, and type mismatches.
- **Positive:** Workflows can be generated dynamically using Rust control flow (e.g., `for rule in config { dag.add_node(...) }`).
- **Positive:** The Engine logic is drastically simplified as it executes native Rust structs instead of interpreting a massive JSON schema specification.
- **Negative:** The UI cannot "hot reload" an active workflow definition without recompiling the host application (acceptable for a CI/CD-driven GitOps workflow).