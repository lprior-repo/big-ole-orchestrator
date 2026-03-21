# ADR-018: Dioxus as Compiler Control Plane

## Status

Accepted. Supersedes ADR-011.

## Context

The prior frontend architecture (ADR-011) was an adapted fork of Oya, a React-based workflow editor. This approach treated the frontend as a dashboard with a visual editor bolted on.

The v3 architecture makes a more fundamental claim: **the frontend is a compiler**. The Dioxus application is the control plane. When a user clicks "Deploy," the application traverses its internal graph model and emits deterministic Rust source code that the engine executes. The visual canvas is the source of the workflow program, not a representation of a separately-maintained JSON definition.

This changes the relationship between the frontend and the backend fundamentally. The backend executes what the frontend compiles. They share a type system (Rust). Type errors are compiler errors, not runtime errors.

The Oya fork would have required continuous translation between Restate-specific patterns and wtf-engine patterns. A ground-up Dioxus implementation is the correct architectural choice given the Rust-native end-to-end constraint.

## Decision

The wtf-engine frontend is a **Dioxus application** (Rust, compiles to native desktop and WASM) with three modes: **Design**, **Simulate**, and **Monitor**. It is the sole mechanism for defining new workflow types.

### Compilation Target

```toml
[dependencies]
dioxus = { version = "0.7", features = ["web", "desktop", "router"] }
```

The same source compiles to:
- Native desktop binary (primary production target — full performance, offline capable)
- WASM web app (browser access, served by the engine's axum server at `/ui`)

### Application Modes

#### Design Mode

A vector-graphics canvas built with Dioxus signals and custom SVG rendering.

- **FSM canvas:** State nodes, transition edges, effect annotation panels
- **DAG canvas:** Activity nodes, dependency edges, parallelism visualization
- **Procedural canvas:** Code scaffold editor with `ctx.*` call annotations

Validation runs live:
- Unreachable states highlighted in amber
- Missing terminal states flagged
- DAG cycles detected immediately (petgraph `is_cyclic_directed`)
- Missing `ctx.*` determinism annotations flagged

#### Simulate Mode

Runs workflow logic locally inside the Dioxus application. No engine connection required.

- For FSM: click available transitions to advance state; event log panel shows the `WorkflowEvent` values that would be written to JetStream
- For DAG: mark nodes complete manually; dependency evaluation shown live
- For Procedural: step through `ctx.*` calls; checkpoint map shown as it builds

This gives developers an accurate preview of the durable record structure before deploying.

#### Monitor Mode

Connects to a running engine via WebSocket (`ws://<host>/api/v1/watch/<namespace>`).

- Live state overlays on the same graph used in Design mode
- Event log timeline: real JetStream events, in sequence order, with timestamps
- **Time-travel scrubber:** drag to any sequence number and see the exact state the instance was in at that moment (engine replays to that seq on demand)
- Pending activity duration: how long each in-flight activity has been waiting
- Diff view: current state vs. last known good state

### Code Generation Pipeline

When the developer clicks **Deploy** in Design mode:

1. **Graph traversal** — Dioxus internal graph model → AST
2. **Validation pass** — all invariants checked; deployment blocked on errors
3. **Rust code generation** — five artifact types per workflow:
   - `<workflow>_types.rs` — state enum, event enum, effect declarations (regenerated on every deploy)
   - `<workflow>_machine.rs` — reducer match arms, transition table (regenerated)
   - `<workflow>_worker.rs` — activity function signatures with `todo!` bodies (generated once, never overwritten)
   - `<workflow>_bin.rs` — binary entrypoint (regenerated)
   - Cargo.toml additions (regenerated)
4. **Push to engine** — generated Rust source posted to `/api/v1/definitions/<type>`
5. **Engine compilation** — engine compiles via `rustc` subprocess or loads pre-compiled WASM module

### Generated Code Shape

The generated code is deterministic, exhaustive, and passes `clippy` with default lints.

For an FSM with states `[Pending, Authorized, Charged, Fulfilled]` and events `[Authorize, Charge, Fulfill, Cancel]`:

```rust
// <workflow>_types.rs — GENERATED, do not edit

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Pending,
    Authorized,
    Charged,
    Fulfilled,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Authorize { payment_method: String },
    Charge { amount_cents: u64 },
    Fulfill { tracking_id: String },
    Cancel { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    CallAuthorizationService { payment_method: String },
    CallChargeService { amount_cents: u64 },
    CallFulfillmentService { tracking_id: String },
    SendCancellationEmail { reason: String },
}

// <workflow>_machine.rs — GENERATED, do not edit

pub fn transition(state: &State, event: &Event) -> Option<(State, Vec<Effect>)> {
    match (state, event) {
        (State::Pending, Event::Authorize { payment_method }) =>
            Some((State::Authorized, vec![Effect::CallAuthorizationService {
                payment_method: payment_method.clone()
            }])),
        (State::Authorized, Event::Charge { amount_cents }) =>
            Some((State::Charged, vec![Effect::CallChargeService {
                amount_cents: *amount_cents
            }])),
        (State::Charged, Event::Fulfill { tracking_id }) =>
            Some((State::Fulfilled, vec![Effect::CallFulfillmentService {
                tracking_id: tracking_id.clone()
            }])),
        (_, Event::Cancel { reason }) =>
            Some((State::Cancelled, vec![Effect::SendCancellationEmail {
                reason: reason.clone()
            }])),
        _ => None, // No transition for this (state, event) pair
    }
}
```

### Frontend ↔ Backend Communication

```
Design Mode:  POST /api/v1/definitions/<type>   (upload generated code)
Monitor Mode: WS   /api/v1/watch/<namespace>    (NATS KV watch proxy)
              GET  /api/v1/instances/<id>/events (JetStream log for time-travel)
              POST /api/v1/instances/<id>/replay-to/<seq> (snapshot to seq for scrubber)
```

## Consequences

### Positive

- Type-safe end-to-end: compiler errors catch workflow bugs before deployment
- No Restate-specific tech debt to carry
- The visual canvas is the canonical definition — no DSL/JSON to maintain separately
- Time-travel debugging comes for free from the event log (ADR-013)
- Dioxus compiles to both desktop and web from one codebase

### Negative

- Ground-up frontend build (no Oya shortcut)
- Dioxus 0.7 is still maturing; some patterns may shift
- WASM bundle size for the web target requires optimization

### Mitigations

- Desktop target ships first (faster, full performance, no WASM constraints)
- Web target is a secondary build target using the same Dioxus codebase
- Dioxus API surface is stabilizing rapidly with 0.7
