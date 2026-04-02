# ADR 004 (v2): Code-as-Workflow (Rust SDK Definition)

## Status
Accepted

## Context
v1 assumed workflows would be defined as raw JSON documents adhering to the AWS Step Functions schema.

While JSON is excellent for serialization and UI rendering, it is "dead configuration" for developers. Writing complex DAGs in raw JSON is error-prone, lacks compile-time safety, and prevents dynamic graph generation.

For v2, we also need the future drag-and-drop UI and AI tooling to target the exact same semantics as the Rust SDK. Two competing workflow models would drift.

## Decision
We adopt the "Code-as-Workflow" paradigm. Workflows are authored in Rust using the `vo-sdk`, and the SDK compiles that authoring model into a canonical, versioned `WorkflowSpec` shared with the Engine and UI (ADR-031).

### The Fluent Builder
Developers use the SDK to define the graph programmatically with explicit node kinds:
```rust
let mut wf = vo_sdk::Workflow::new("checkout_flow");

let validate = wf.pure("validate", validate_cart);
let charge = wf.effect("charge", charge_customer);

wf.connect(&validate, &charge);
```

### The Shared Workflow Model
1. **Compilation:** The SDK builds a typed in-memory workflow representation.
2. **Canonicalization:** The SDK serializes that representation into the canonical `WorkflowSpec` JSON emitted during `--graph`.
3. **Registration:** The Engine validates, hashes, and stores the spec as a workflow version.
4. **UI and AI:** The Dioxus UI and AI tooling read and write the same canonical spec instead of inventing a second JSON DSL.
5. **Publication Rules:** Exact workflows cannot publish if the graph contains `Unsafe` nodes, unsupported managed-effect sinks, or missing dedupe requirements.

## Consequences
- **Positive:** Rust compiler catches broken edges, missing nodes, and type mismatches.
- **Positive:** The future drag-and-drop UI can become a real first-class authoring surface without semantic drift.
- **Positive:** Guarantee class, retry rules, and node capability are explicit in the workflow definition instead of being implied by task code.
- **Negative:** `WorkflowSpec` versioning and migration become a core compatibility surface.
- **Negative:** The UI still cannot hot-reload an active workflow definition without publishing a new version, which remains acceptable for a CI/CD-driven GitOps workflow.
