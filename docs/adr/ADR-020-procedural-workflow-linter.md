# ADR-020: Procedural Workflow Static Linter

## Status

Accepted

## Context

The procedural execution paradigm (ADR-017) requires workflow functions to call `ctx.*` methods in a deterministic, consistent order on every replay. The operation ID for each `ctx.activity(...)` call is derived from `instance_id + monotonic counter`. If the function calls `ctx.*` methods in a different order on re-execution (due to branching on external data, non-deterministic utilities, or developer error), the counter-derived operation IDs will be wrong, and the checkpoint map will return incorrect results.

This is the same class of bug that Temporal users hit when they accidentally use non-deterministic APIs inside workflow functions. In Temporal's case it surfaces as a runtime error during replay. In wtf-engine we want to detect it **at compile time** before deployment.

The Dioxus compiler (ADR-018) generates workflow code that is deterministic by construction. But developers may also write procedural workflows by hand. The linter must catch violations in hand-written code.

## Decision

A static analysis linter runs as part of the engine's workflow ingestion pipeline and as a standalone CLI command. It analyzes the Rust source of procedural workflow functions and flags patterns that would cause non-deterministic `ctx.*` call ordering.

### Linter Integration Points

1. **Dioxus Deploy button** — linter runs on generated code before upload (should always pass; provides a safety net)
2. **Engine ingestion API** — `POST /api/v1/definitions/<type>` runs linter on uploaded source; returns `422 Unprocessable Entity` with lint errors if violations found
3. **`wtf lint <file>`** — standalone CLI command for local development feedback

### Detected Patterns

#### Pattern 1: Non-deterministic time

```rust
// FORBIDDEN in workflow function body
let now = std::time::SystemTime::now();
let now = chrono::Utc::now();
let now = tokio::time::Instant::now();

// REQUIRED
let now = ctx.now(); // returns logged timestamp; consistent on replay
```

#### Pattern 2: Non-deterministic randomness

```rust
// FORBIDDEN
let id = uuid::Uuid::new_v4();
let n = rand::random::<u64>();

// REQUIRED
let n = ctx.random_u64(); // returns logged value; consistent on replay
```

#### Pattern 3: Direct async I/O

```rust
// FORBIDDEN — direct network call inside workflow function
let resp = reqwest::get("https://api.example.com").await?;
let row = sqlx::query("SELECT ...").fetch_one(&pool).await?;

// REQUIRED
let resp = ctx.activity("fetch_data", input).await?;
```

#### Pattern 4: ctx calls inside closures with non-deterministic dispatch order

```rust
// FORBIDDEN — dispatch order depends on HashMap iteration order
let mut tasks = HashMap::new();
for item in items {
    tasks.insert(item.id, async { ctx.activity("process", item).await });
}
// joining these executes ctx.activity in HashMap iteration order (non-deterministic)

// REQUIRED — use ctx.parallel() which records dispatch order in the log
let results = ctx.parallel(items.iter().map(|item|
    ctx.activity("process", item)
)).await?;
```

#### Pattern 5: tokio::spawn inside workflow function

```rust
// FORBIDDEN — spawned tasks may call ctx.* in non-deterministic order
tokio::spawn(async { ctx.activity("background_task", input).await });

// REQUIRED — use ctx.activity directly; it is already async
ctx.activity("background_task", input).await?;
```

### Lint Output Format

```
error[WTF-L001]: non-deterministic time call in workflow function
  --> src/checkout_worker.rs:42:15
   |
42 |     let now = chrono::Utc::now();
   |               ^^^^^^^^^^^^^^^^^^ use `ctx.now()` instead
   |
   = note: direct time calls produce different values on replay

error[WTF-L003]: direct async I/O in workflow function
  --> src/checkout_worker.rs:67:18
   |
67 |     let resp = reqwest::get(url).await?;
   |                ^^^^^^^^^^^^^^^^^^^^^^^^ wrap in ctx.activity(...)
   |
   = note: results of network calls must be logged to be replayed correctly
```

### Implementation Approach

The linter is implemented as a Rust crate (`wtf-linter`) using `syn` to parse the workflow source and walk the AST. It does not require the full Rust compiler — it uses `syn` + `quote` for parsing and analysis only.

Lint rules are defined as visitors over the `syn` AST. Each rule pattern matches a specific AST shape and emits a diagnostic. The linter is conservative: it may have false negatives (miss some violations) but must not have false positives (must not flag correct code).

### Lint Rule Registry

| Code | Name | Severity |
|------|------|----------|
| WTF-L001 | non-deterministic-time | Error |
| WTF-L002 | non-deterministic-random | Error |
| WTF-L003 | direct-async-io | Error |
| WTF-L004 | ctx-in-closure | Warning |
| WTF-L005 | tokio-spawn-in-workflow | Error |
| WTF-L006 | std-thread-spawn-in-workflow | Error |

## Consequences

### Positive

- Determinism violations are caught before deployment, not during production replay
- Developer feedback loop is fast (seconds, not "deploy and crash on first replay")
- The linter documents the constraints explicitly as named rules with actionable messages

### Negative

- syn-based analysis can miss dynamic patterns (e.g., violations hidden inside macro expansions)
- Linter must be kept in sync as new forbidden patterns are discovered
- Does not catch all violations — determinism requires developer discipline beyond what static analysis can guarantee

### Mitigations

- Integration tests that forcefully crash and replay procedural workflows catch violations the linter misses
- Violations that reach production surface as `ReplayDivergence` errors in the engine, which are caught early in load testing
