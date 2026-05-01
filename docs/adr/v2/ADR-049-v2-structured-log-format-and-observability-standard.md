# ADR 049 (v2): Structured Log Format and Observability Standard

## Status
Accepted

## Context

Veloxide uses the `tracing` crate (v0.1) as its sole logging and instrumentation library. The codebase contains approximately 50 `#[tracing::instrument]` annotations across `vo-api`, `vo-ipc`, `vo-executor`, `vo-actor`, and `vo-core`, and hundreds of `tracing::info!`/`warn!`/`error!` calls using structured fields (e.g., `instance_id`, `method`, `status`, `duration_ms`).

However, three critical gaps exist:

1. **No subscriber initialization in the main engine.** Only the standalone `fleet-feed` utility calls `tracing_subscriber::fmt().with_env_filter().init()`. The main engine binary (`vo-cli`) never initializes a subscriber, making all tracing calls in `vo-actor`, `vo-api`, `vo-worker`, `vo-executor`, `vo-core`, and `vo-ipc` no-ops at runtime.

2. **No structured JSON output.** All subscriber configuration (where it exists) uses the default text formatter. No `tracing_subscriber::fmt().json()` or equivalent is configured anywhere. The structured fields in trace macros are discarded by the text formatter.

3. **No OpenTelemetry/OTLP integration.** The `vo-common/src/telemetry/` module defines stub types (`TelemetryExporter`, `OtlpEndpoint`, `TelemetryConfig`, `TelemetryTracer`) that claim to provide "unified metrics, tracing, and log correlation with OTLP export" but contain no actual export logic. No `opentelemetry`, `opentelemetry-otlp`, or `tracing-opentelemetry` crate dependency exists.

The `metrics` crate (v0.24) is also used in `vo-storage` for application-level metrics, but it has no configured exporter — metrics are recorded but never exposed.

Without a structured log format standard, logs from the engine are unparseable by log aggregators (Grafana Loki, Elasticsearch, Datadog), trace context from workflow execution cannot be correlated across services, and the existing instrumentation investment (structured fields, instrument annotations) is wasted.

## Decision

We establish a structured logging and observability standard with three layers.

### 1. Log Format: Structured JSON via tracing-subscriber

The engine SHALL emit all logs as newline-delimited JSON (one JSON object per line). This is achieved via `tracing_subscriber::fmt().json()`.

**Every log line SHALL contain these fields:**

| Field | Source | Example |
|-------|--------|---------|
| `timestamp` | `tracing_subscriber::fmt::time::UtcTime` | `"2026-04-30T14:30:00.123Z"` |
| `level` | Tracing level | `"info"`, `"warn"`, `"error"` |
| `target` | Crate module path | `"vo_actor::spawn_supervisor"` |
| `message` | Log message | `"workflow completed"` |
| `span` | Current span name | `execute_node` |
| `trace_id` | W3C trace context (16 hex chars) | `"4bf92f3577b34da6"` |
| `span_id` | W3C span context (8 hex chars) | `"00f067aa0ba902b7"` |
| `instance_id` | Workflow instance (when present) | `"inst_abc123"` |

Additional structured fields from individual trace macros (e.g., `method`, `status`, `duration_ms`) are included as top-level JSON keys.

### 2. Subscriber Initialization: Centralized in vo-core

A new `vo-core::tracing` module provides the subscriber builder. The main engine binary (`vo-cli`) and the API server (`vo-api`) both call this during startup.

```rust
// vo-core/src/tracing.rs
pub fn init_subscriber(filter: &str) -> Result<(), BoxError> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::new(filter))
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_target(true)
        .with_current_span(true)
        .with_span_list(false)
        .init();
    Ok(())
}
```

**Default filter:** `RUST_LOG` environment variable, falling back to `"info"`.

**Override:** The CLI flag `--log-level` overrides the filter for the engine process. The API server reads from config or env.

### 3. Trace Context Propagation

Workflow execution generates a root trace context when a workflow is instantiated. This context propagates through:

1. **Actor state machine** — the root span covers the workflow actor's lifetime.
2. **Subprocess execution** — the `trace_id` and `span_id` are injected into the child process's environment as `VO_TRACE_ID` and `VO_SPAN_ID`. Child binaries using the `vo-sdk` automatically pick these up.
3. **FD3/FD4 IPC** — trace context is included in the IPC envelope header, not in the payload JSON.
4. **API requests** — the `tower-http` `TraceLayer` extracts or generates a trace context per request. The custom logging middleware (`vo-api/src/middleware/logging.rs`) already emits `request_id` (ULID) — this is mapped to `trace_id` when no external trace context is present.

### 4. OTLP Export (Future Phase)

The stub types in `vo-common/src/telemetry/` are retained but marked with `#[deprecated]` documentation pointing to the new standard. Actual OTLP export requires adding:

- `opentelemetry` + `opentelemetry-otlp` + `tracing-opentelemetry` as optional dependencies.
- A `TelemetryLayer` configured behind a `telemetry` feature flag.

This is explicitly out of scope for this ADR and tracked as a follow-up bead. The JSON log format is designed to be forward-compatible with OTLP ingestion: log aggregators that accept JSON can parse and index the structured fields directly.

### 5. What This Does NOT Change

- **Existing trace macros** — all `#[tracing::instrument]` annotations and `tracing::info!`/`warn!`/`error!` calls remain unchanged. The structured fields they emit are already correct; the subscriber just formats them as JSON.
- **`metrics` crate usage** — `vo-storage`'s use of the `metrics` facade is orthogonal to logging and is not affected.
- **Child process stdout** — the engine still reads output from FD4, not stdout. Child process logging is the child's responsibility.
- **`fleet-feed`** — the standalone utility retains its own subscriber init (text format, env-filter) since it is not part of the engine.

## Consequences

### Positive
- **Machine-parseable logs**: Every log line is valid JSON, enabling ingestion by Loki, Elasticsearch, Datadog, or any JSON-aware aggregator without custom parsers.
- **Trace correlation**: `trace_id` and `span_id` allow reconstructing the full execution path of a workflow across actor boundaries and subprocess invocations.
- **No code changes to instrumentation**: The existing 50+ `#[tracing::instrument]` annotations and structured field usage work as-is with the JSON formatter.
- **Operator-friendly**: `jq` can filter and transform logs locally. `jq 'select(.level == "error")'` surfaces errors.
- **Forward-compatible**: The JSON format maps cleanly to OTLP log records when OTLP export is implemented.

### Negative
- **Verbose output in terminal**: JSON logs are harder to read in a terminal during local development. The `--log-format text` flag (or `RUST_LOG_FORMAT=text`) is provided to fall back to human-readable output.
- **Larger log volume**: JSON keys add overhead versus compact text. The structured fields were already being emitted; only the framing changes.

### Migration
- Add `vo-core::tracing` module with subscriber builder.
- Call `init_subscriber()` in `vo-cli` main and `vo-api` server startup.
- Add `--log-level` and `--log-format` CLI flags to `vo-cli`.
- Mark `vo-common/src/telemetry/export.rs` stubs as deprecated.
- No changes to any trace macro calls or instrumentation annotations.
