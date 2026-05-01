# ADR 060 (v2): Metrics Cardinality Explosion Prevention

## Status
Accepted

## Context
The Engine uses the `metrics` crate (v0.24) for observability. While the current implementation is relatively clean, the `metrics` crate records all label combinations as separate time series in the backend (Prometheus, Datadog, OpenTelemetry, etc.). 

If unbounded label values leak into metric keys, cardinality explodes: a single metric with a user-controlled label like `workflow_id`, `user_id`, or `tenant_id` can create millions of unique time series in hours, crashing the metrics backend and causing OOM in the recording sidecar.

### Current State Audit (April 2026)

All 21 metric usages across 4 crates were audited:

| Crate | Metric | Dynamic Labels | Cardinality Risk |
|-------|--------|---------------|-----------------|
| `vo-storage` | `vo_storage.write_rejected_total` | `"class"` (3 values), `"reason"` (2 values) | **LOW** — 6 max combinations |
| `vo-storage` | `vo_storage.queue_depth` | `"class"` (3 values) | **LOW** — 3 max combinations |
| `vo-scheduler` | `vo_scheduler.jobs_scheduled_total` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.jobs_completed_total` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.jobs_failed_total` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.jobs_cancelled_total` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.jobs_retried_total` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.queue_depth` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.job_execution_duration_seconds` | None | **NONE** |
| `vo-scheduler` | `vo_scheduler.job_retry_delay_seconds` | None | **NONE** |
| `vo-core` | `vo_config_hot_reload.reloads_total` | None | **NONE** |
| `vo-core` | `vo_config_hot_reload.reload_errors_total` | None | **NONE** |
| `vo-core` | `vo_config_hot_reload.reload_duration_ms` | None | **NONE** |
| `vo-actor` | `vo.db_writer.committed` | None | **NONE** |
| `vo-actor` | `vo.db_writer.mailbox_depth` | None | **NONE** |

**Finding:** The current codebase is clean. No unbounded label values exist. However, no policy or enforcement mechanism prevents future developers from adding high-cardinality labels.

### How Cardinality Explosion Happens

The `metrics` crate uses label tuples to identify unique time series. A metric call like:

```rust
metrics::counter!("vo.workflow.status", "workflow_id" => wf_id, "status" => status_str).increment(1);
```

creates a new time series for every unique `workflow_id` + `status_str` combination. If `workflow_id` is a ULID (effectively random), this creates one series per workflow instance — millions in a busy deployment.

The `metrics` crate buffers observations in a `sweep` interval. Each unique label tuple occupies memory in the recorder. Prometheus-backed recorders write these to disk. The explosion is not just memory — it destroys query performance on the metrics backend.

## Decision

### 1. The Bounded Labels Policy

**All metric label values MUST come from a bounded enum or a hard-coded set of known strings.** No dynamic strings (IDs, names, user input, ULIDs, UUIDs) are permitted as metric label values.

**Allowed label value sources:**
- Rust `enum` with `#[derive(Display)]` or `match`-to-string conversion
- Hard-coded string literals
- Config constants (e.g., `WriteClass` → `"critical_control_plane" | "operator_projection" | "bulk_blob"`)

**Forbidden label value sources:**
- User IDs, workflow IDs, instance IDs, tenant IDs
- ULIDs, UUIDs, or hash values
- Free-form strings (path components, error messages, request URLs)
- Any value derived from external input without bounded enumeration

### 2. The Metrics Registry Gate

Every new metric must pass a review gate before being merged. The gate checks:

1. **No dynamic labels** — all label values are from bounded sets
2. **Label count ≤ 3** — each metric has at most 3 label dimensions (name + up to 3 labels)
3. **Purpose documented** — the `describe_*!()` call (when recorder supports it) explains what the metric measures and why
4. **No per-request/per-identity metrics** — metrics aggregate, they do not track individual entities

### 3. Static Label Enforcement via Wrapper Module

A project-level `vo_metrics` module (or crate-level `metrics` wrapper) provides label-safe constructors:

```rust
// Instead of:
metrics::counter!("vo.foo", "label" => dynamic_string).increment(1);

// Use:
vo_metrics::counter!("vo.foo").label(StaticLabel::FooVariant::Bar).increment(1);
```

This enforces at the type system level that label values are from a bounded enum, not free strings. The wrapper module also provides:

- `StaticLabel<T: Into<&'static str>>` — only allows `'static` string literals or enum-derived values
- Compile-time label count enforcement via const generics or builder pattern
- A `describe_metric!()` macro that documents metrics for the review gate

### 4. Monitoring and Alerting

The metrics backend (when Prometheus or compatible) exposes `prometheus_target_metadata_cache` or equivalent. Alerts trigger on:

- **Time series count per metric > 100** — warns of unexpected label growth
- **Time series count per metric > 1000** — critical, indicates active cardinality leak
- **Rate of new time series creation > 10/min** — detects live injection of unbounded labels

### 5. Runtime Cardinality Guard

A `metrics::SetRecorderError` handler monitors for recorder-level cardinality warnings. When the underlying recorder (e.g., Prometheus `Registry`) reports label tuple limits:

1. The affected metric is automatically switched to a "drop" mode (observations recorded but not exported)
2. A `tracing::error!` is emitted with the metric name and offending label values
3. An alert fires to the on-call engineer

## Consequences

- **Positive:** Mathematically bounded metric cardinality — the worst case is the product of all bounded label enum variants across all metrics (currently < 100 total time series)
- **Positive:** Metrics backends remain performant regardless of deployment scale
- **Positive:** Type-safe label enforcement catches cardinality violations at compile time
- **Negative:** Slightly more verbose metric definition code (mitigated by `vo_metrics` wrapper macros)
- **Negative:** Review gate adds ~5 minutes per new metric (acceptable cost vs. OOM risk)
- **Negative:** If a new bounded dimension is needed, the enum must be extended (requires code change, not just config) — this is intentional to prevent accidental cardinality growth

### Implementation Notes

- The `vo_metrics` wrapper module should be created in `crates/vo-core/src/metrics/`
- Existing metrics in `vo-storage`, `vo-scheduler`, `vo-core`, and `vo-actor` should be migrated to the wrapper
- The `metrics` crate v0.24 supports `describe_counter!()`, `describe_gauge!()`, `describe_histogram!()` for documentation when a `prometheus` recorder is used
- The `prometheus` crate's `Registry` has no hard cardinality limit, but the `promhttp` exporter and downstream consumers (Prometheus server) do
