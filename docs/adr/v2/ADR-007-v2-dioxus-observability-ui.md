# ADR 007 (v2): Visual Observability via Dioxus and SSE

## Status
Accepted

## Context
A durable workflow engine is only as good as its observability. If users cannot see what is happening, where a payload failed, or how long a step took, the engine is a black box.

We also want the UI to grow into a future drag-and-drop authoring surface. To maintain the "Single Binary" constraint, we cannot require a separate Node.js or React application.

## Decision
We will embed a Dioxus WASM UI directly into the Engine's Axum router.

### Architecture
1. **The Axum Router:** `vo-engine` serves the compiled Dioxus `.wasm` file and static assets on `GET /wtf/ui`.
2. **Authoritative Query APIs:** The Engine exposes timeline, history, instance, effect-journal, and workflow-version endpoints backed by durable state and operator projections.
3. **The Telemetry Stream:** `GET /api/v1/watch/:instance_id` remains an SSE endpoint, but it is a **best-effort live tail**, not the source of truth.
4. **The Reactive Canvas:** The Dioxus app renders the canonical `WorkflowSpec`, listens to SSE for live animation, and falls back to authoritative HTTP queries when the SSE stream lags or reconnects.
5. **Guarantee-Aware UX:** The UI visually distinguishes node classes such as `Pure`, `Managed Effect`, `Wait`, and `Unsafe`, and it surfaces whether a workflow qualifies for exact-once execution.

### Future Drag-and-Drop Authoring
The UI does not invent a second workflow format. It edits the same canonical `WorkflowSpec` used by the SDK and Engine. Publish-time validation rejects invalid exact workflows before activation.

## Consequences
- **Positive:** The Engine provides a world-class visual debugging and future no-code authoring experience out of the box.
- **Positive:** No JavaScript required. The entire stack (Engine, SDK, and UI) is 100% Rust.
- **Positive:** Operators can reason about guarantees directly in the UI instead of guessing from logs.
- **Negative:** The UI must handle SSE lag, reconnection, and query-based state reconciliation.
- **Negative:** WASM payloads have a larger initial download size compared to raw HTML/HTMX, though this is negligible on modern networks.
