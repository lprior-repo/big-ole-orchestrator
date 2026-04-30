# ADR-052: Structured Log Format and Observability Standard

**Status**: Accepted  
**Created**: 2026-04-30  
**Supersedes**: None  
**Related**: ADR-007 (v2) Visual Observability via Dioxus and SSE, ADR-013 (v2) System Resilience

## Context

The Veloxide engine uses `tracing` instrumentation throughout all crates (`tracing::info!`, `tracing::debug!`, `#[tracing::instrument]`), but no tracing subscriber is initialized. This means:

1. All trace events are **silently dropped** — no logs appear in stdout, stderr, or any file
2. No structured (JSON) log format exists — log aggregators cannot parse output
3. No trace context propagation — `trace_id` and `span_id` are never emitted alongside log entries
4. No OTLP integration — metrics and traces cannot be exported to external backends (Grafana, Jaeger, etc.)

This creates an observability black box: operators cannot debug issues, correlate events, or monitor system health through logs.

## Decision

The system SHALL emit structured logs in JSON format with trace context propagation, using `tracing-subscriber` with JSON formatting. OTLP export SHALL be configured but can be disabled via environment variable for local development.

### Log Format

Every log line SHALL be a valid JSON object with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string (RFC 3339) | ISO 8601 timestamp with timezone |
| `level` | string | One of: `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR` |
| `target` | string | Rust module path that generated the log |
| `message` | string | The log message |
| `span_id` | string (hex) | Current tracing span ID (16 hex chars) |
| `trace_id` | string (hex) | Current tracing trace ID (32 hex chars) |
| `service.name` | string | `"veloxide-engine"` or `"veloxide-cli"` |

Additional fields SHALL be emitted for structured fields added via `tracing` key-value pairs (e.g., `error`, `sequence`, `workflow_id`).

### Architecture

```
┌─────────────────────────────────────────────────┐
│  Application Code (all crates)                  │
│  tracing::info!(workflow_id = "x", "started")   │
└──────────────────┬──────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────┐
│  tracing-subscriber (JSON format)               │
│  - JSON serialization with trace context        │
│  - EnvFilter for log level control              │
│  - RFC 3339 timestamps                          │
│  - Service name tagging                         │
└──────────────────┬──────────────────────────────┘
                   │
          ┌────────┴────────┐
          ▼                 ▼
   stdout (JSON)    OTLP Exporter (optional)
                   (via VELOXIDE_OTLP_ENDPOINT)
```

### Initialization

Tracing SHALL be initialized once at application startup in the CLI entry point (`vo-cli/src/main.rs`):

```rust
fn init_tracing() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(false)
        .json();

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
```

The function SHALL be called at the very start of `main()`, before any other initialization.

### Log Level Control

Log levels SHALL be controlled via the `RUST_LOG` environment variable, following the `tracing-subscriber` convention:

| Variable | Purpose | Default |
|----------|---------|---------|
| `RUST_LOG` | Log level filter | `info` |
| `VELOXIDE_OTLP_ENDPOINT` | OTLP endpoint URL | Not set (OTLP disabled) |
| `VELOXIDE_LOG_FORMAT` | Log format: `json` or `pretty` | `json` |

### OTLP Integration

OTLP export SHALL be wired up via the `VELOXIDE_OTLP_ENDPOINT` environment variable:

- When `VELOXIDE_OTLP_ENDPOINT` is **not set**: OTLP export is disabled, only console output
- When `VELOXIDE_OTLP_ENDPOINT` **is set**: OTLP exporter is added alongside console output

This keeps local development simple (no external dependencies) while enabling production observability.

## Consequences

### Positive

- **Queryable logs**: Every log line is valid JSON, parseable by log aggregators (Fluent Bit, Vector, Grafana Loki)
- **Trace correlation**: `trace_id` and `span_id` in log lines enable cross-service event correlation
- **Configurable verbosity**: `RUST_LOG` environment variable controls log levels without code changes
- **Production-ready**: OTLP export enables integration with Grafana, Jaeger, Honeycomb, etc.
- **No silent drops**: All `tracing` events now reach a subscriber

### Negative

- **Larger log output**: JSON is more verbose than plain text
- **Performance overhead**: JSON serialization adds CPU cost (acceptable for observability)
- **New dependencies**: `tracing-subscriber` with `json` and `time` features
- **OTLP requires external service**: When `VELOXIDE_OTLP_ENDPOINT` is set, the exporter will retry connections even if no backend is available

### Mitigations

- `VELOXIDE_LOG_FORMAT=pretty` provides human-readable output for local debugging
- OTLP exporter uses exponential backoff for connection failures
- Log levels can be set to `warn` or `error` in production to reduce volume

## Implementation Notes

### Files Modified

- `Cargo.toml` (workspace): Add `json` and `time` features to `tracing-subscriber` dependency
- `crates/vo-cli/src/main.rs`: Call `init_tracing()` at start of `main()`
- `crates/fleet-feed/src/main.rs`: Call `init_tracing()` at start of `main()` (upgrade from default fmt)

### Validation

- `moon run :test` passes after changes
- `RUST_LOG=debug cargo run -- serve ...` produces JSON log lines on stdout
- Each JSON line contains `span_id` and `trace_id` fields when inside a `#[tracing::instrument]` span

## Open Questions

1. Should trace context be propagated through IPC (vo-ipc crate) for cross-process correlation?
2. Should we add `span_events` to JSON output to capture intra-span events?
3. Should OTLP export use gRPC or HTTP as the default codec?
4. Should structured logging be enabled by default in tests, or kept minimal for test output clarity?

## References

- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber)
- [OpenTelemetry Protocol specification](https://opentelemetry.io/docs/specs/otlp/)
- ADR-007 (v2): Visual Observability via Dioxus and SSE
- `tracing-subscriber` features: `json`, `env-filter`, `time`
