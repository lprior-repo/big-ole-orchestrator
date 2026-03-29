# ADR 007 (v2): Visual Observability via Dioxus and SSE

## Status
Accepted

## Context
A durable workflow engine is only as good as its observability. If users cannot see what is happening, where a payload failed, or how long a step took, the engine is a black box.

We need a visual, drag-and-drop style interface (similar to n8n) to monitor workflows. However, to maintain the "Single Binary" constraint, we cannot require users to deploy a separate Node.js or React application.

## Decision
We will port the existing `oya-frontend` codebase into a new crate (`vo-ui`) and embed it directly into the Engine's Axum router. The UI will be built in **Dioxus (WASM)**.

### Architecture
1. **The Axum Router:** `vo-engine` serves the compiled Dioxus `.wasm` file and static assets on `GET /wtf/ui`. 
2. **The Telemetry Stream:** The Engine exposes an endpoint `GET /api/v1/watch/:instance_id`. This endpoint uses Server-Sent Events (SSE). As the `DbWriterActor` flushes events to `fjall`, it pushes those exact same JSON events down the SSE connection.
3. **The Reactive Canvas:** The Dioxus WASM application runs in the user's browser. It reads the static DAG structure exported by the `vo-sdk` and draws the node graph. It listens to the SSE stream. Because Dioxus uses fine-grained Signals, when an `ActivityCompleted` event arrives via SSE, the Signal updates, and the specific SVG node on the canvas instantly glows green.

### Materialized Views for the Dashboard
Because the underlying storage (`fjall`) is a Key-Value store and cannot execute SQL queries, the Engine maintains an `instances` partition (`<status>:<timestamp>:<instance_id>`). 
When the user opens the UI Dashboard to view historical runs, the Axum API performs a fast prefix-scan on this partition and returns a paginated list of `InstanceSummary` JSON objects.

## Consequences
- **Positive:** The Engine provides a world-class, n8n-style visual debugging experience out of the box.
- **Positive:** No JavaScript required. The entire stack (Engine, SDK, and UI) is 100% Rust.
- **Positive:** Zero polling overhead. SSE streams provide millisecond-accurate visual updates to the graph.
- **Negative:** WASM payloads have a larger initial download size compared to raw HTML/HTMX, though this is negligible on modern networks.