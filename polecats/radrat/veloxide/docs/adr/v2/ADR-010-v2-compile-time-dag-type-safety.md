# ADR 010 (v2): Compile-Time DAG Type Safety

## Status
Accepted

## Context
Traditional orchestrators (Temporal, Restate, Airflow) rely on dynamic typing or runtime reflection to move data between workflow steps. If Step A outputs an `Order` object, but Step B expects a `Receipt` object, the system only discovers this mismatch at runtime when Step B crashes.

Because `vo-engine` compiles an entire workflow (all steps and DAG topology) into a single Rust binary (ADR-009), we have a unique opportunity to leverage the `rustc` compiler to validate the entire workflow graph before it ever runs.

## Decision
We will enforce cross-node type safety at compile time using a heavily constrained generic `NodeHandle<I, O>` pattern in the `vo-sdk`.

### Implementation Strategy
When a developer registers a task function, the SDK wraps it in a typed handle:
```rust
// `validate_cart` takes `Order` and returns `ValidatedOrder`
let validate = dag.add_node("validate", validate_cart);
// validate is of type: NodeHandle<Order, ValidatedOrder>

let charge = dag.add_node("charge", charge_stripe);
// charge is of type: NodeHandle<ValidatedOrder, Receipt>
```

The DAG builder's `connect` method uses a generic type constraint `T` to unify the output of the source node with the input of the target node:
```rust
impl Dag {
    pub fn connect<T>(&mut self, from: &NodeHandle<impl Any, T>, to: &NodeHandle<T, impl Any>) {
        // ... implementation
    }
}
```

### Usage
```rust
dag.connect(&validate, &charge); // Compiles perfectly

// dag.connect(&charge, &validate);
// ERROR: expected `Receipt`, found `Order`
```

### Fan-In Workaround
For `fan_in` scenarios (multiple nodes feeding into one node), enforcing compile-time type safety across arbitrary struct fields requires complex macro generation that degrades the developer experience. For v1, `fan_in` will rely on runtime `serde` validation. The engine will supply a JSON object keyed by parent node names, and the child node will use standard `#[derive(Deserialize)]` to extract the fields it needs.

## Consequences
- **Positive:** Unprecedented pipeline safety. The rust compiler becomes a strict workflow linter. 
- **Positive:** AI Code-Gen loop is flawless. If the AI hallucinates a bad connection, the code will fail to compile, and the AI can read the compiler error to fix it autonomously.
- **Negative:** Slightly more verbose graph definitions for the developer (they must capture the handle returned by `add_node` instead of just passing strings).