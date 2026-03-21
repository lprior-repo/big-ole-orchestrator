# Contract: Phase 4 — API Layer (wtf-5gtk)

bead_id: wtf-5gtk
bead_title: epic: Phase 4 — API Layer (wtf-api)
phase: 4
updated_at: 2026-03-21T04:12:32Z

---

## Epic Overview

The API Layer provides an HTTP API for the wtf-engine based on axum. It exposes workflow management endpoints, real-time dashboard updates via SSE, and workflow definition validation via the integrated linter.

## Scope

### Already Implemented (inherited from previous phases)

1. **Core HTTP Server** (`wtf-api` crate)
   - `app.rs`: Axum router assembly with API routes, health/metrics endpoints
   - `handlers.rs`: HTTP handlers for workflow CRUD operations
   - `routes.rs`: Route definitions
   - `types.rs`: API request/response types

2. **Workflow Management Endpoints** (DONE)
   - `POST /api/v1/workflows` — start workflow instance
   - `GET /api/v1/workflows` — list active instances
   - `GET /api/v1/workflows/:id` — get instance status
   - `DELETE /api/v1/workflows/:id` — terminate instance
   - `POST /api/v1/workflows/:id/signals` — send signal to instance

3. **Health/Metrics** (DONE)
   - `GET /health` — liveness probe
   - `GET /metrics` — Prometheus metrics stub

### Already Spin-off to Child Beads

1. **wtf-wdxg** — SSE watch endpoint for real-time Dioxus dashboard updates
   - `GET /api/v1/watch/:namespace` — NATS KV watch proxy
   - `GET /api/v1/watch` — watch all namespaces

2. **wtf-k0ck** — Time-travel replay endpoint
   - `GET /api/v1/workflows/:id/events` — JetStream log stream
   - `GET /api/v1/instances/:id/replay-to/:seq` — replay to sequence

### Remaining Work (to be spin-off as child bead)

3. **Workflow Definition Ingestion with Linter** (NOT YET STARTED)
   - `POST /api/v1/workflows/validate` — accept workflow Rust source code, run linter, return diagnostics
   - Integrates with `wtf-linter` crate (rules L001-L006)
   - ADR-020 defines the linting architecture

---

## Contract for Remaining Work: Workflow Definition Ingestion

### Endpoint

`POST /api/v1/workflows/validate`

### Request

```json
{
  "workflow_source": "<rust source code as string>"
}
```

### Response (200 OK)

```json
{
  "valid": true|false,
  "diagnostics": [
    {
      "code": "WTF-L001",
      "severity": "error|warning",
      "message": "Human readable message",
      "suggestion": "Optional fix suggestion",
      "span": [start_byte, end_byte]
    }
  ]
}
```

### Response (400 Bad Request)

```json
{
  "error": "parse_error",
  "message": "Failed to parse source code"
}
```

### Error Mapping

- Parse error → 400 with `"parse_error"`
- Linter diagnostics → 200 with `valid: false` if any errors

---

## Dependencies

- Phase 1 (wtf-actor) — OrchestratorMsg actor
- Phase 2 (wtf-storage) — Event log and snapshots
- `wtf-linter` crate — Already exists with rule stubs

---

## Out of Scope

- Workflow execution (handled by wtf-actor)
- Frontend dashboard UI (handled by wtf-frontend)
- Persistence layer (handled by wtf-storage)
