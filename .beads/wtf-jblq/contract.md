# Contract Specification: wtf-jblq — Frontend SSE Watch Client Parity

```json
{"bead_id":"wtf-jblq","phase":"contract-synthesis","updated_at":"2026-03-22T00:00:00Z"}
```

## Context

- **Feature**: Dioxus SSE watch client with reconnect/backoff and `use_instance_watch` hook for instance state monitoring
- **Domain terms**:
  - `WatchError` — error enum for request failures and payload parse failures
  - `BackoffPolicy` — exponential backoff with initial delay and max cap
  - `InstanceView` — parsed SSE payload representing a workflow instance
  - `WatchState` — internal state machine tracking client, URL, backoff policy, and attempt count
  - SSE `data:` frame — Server-Sent Events payload format with two supported schemas:
    1. Key-prefixed: `namespace/id:{json}` — parsed by splitting on `:` then JSON-parsing the RHS
    2. Plain JSON: `{json}` — parsed directly as `InstanceView`
- **Assumptions**:
  - The backend SSE endpoint is at `{base_url}/api/v1/watch/{namespace}`
  - The HTTP `Accept: text/event-stream` header is sent on every request
  - The stream is long-lived and reconnects indefinitely on failure
  - The `use_instance_watch` hook targets `http://localhost:8080` as the base URL (hardcoded in implementation)
- **Open questions**:
  - Should `use_instance_watch` accept a configurable base URL, or is `localhost:8080` intentional for monitor mode?
  - Is there a maximum reconnect attempt cap, or does it retry forever?

## Preconditions

- **P1**: `base_url` passed to `watch_namespace` must be a non-empty, valid URL string.
  - Enforcement: Runtime (Result error variant `WatchError::Request`).
  - Violation: `watch_namespace("", "ns")` → `Err(WatchError::Request(...))` (reqwest rejects empty base URL).
- **P2**: `namespace` passed to `watch_namespace` must be a non-empty string.
  - Enforcement: Runtime (Result error variant `WatchError::Request`).
  - Violation: `watch_namespace("http://localhost:8080", "")` → URL becomes `/api/v1/watch/` (server likely 404s, caught as `WatchError::Request`).
- **P3**: `BackoffPolicy::new(initial, max)` requires `initial <= max`.
  - Enforcement: Debug-only (`debug_assert!` in constructor or delay computation).
  - Violation: `BackoffPolicy::new(Duration::from_secs(5), Duration::from_secs(1))` — `delay_for_attempt(0)` returns `min(5s, 1s) = 1s` but invariant is violated silently (policy is malformed; max is less than initial).

## Postconditions

- **Q1**: `watch_namespace(base_url, namespace)` returns a `Stream<Item = Result<InstanceView, WatchError>>` that never terminates normally (infinite stream).
  - Enforcement: Type signature (compiler-enforced).
- **Q2**: After a successful `fetch_one_event` (returning `Ok(InstanceView)`), the next `WatchState` has `attempt: 0` (backoff reset).
  - Enforcement: Internal state transition logic.
  - Violation: If Q2 is violated, the stream would not reset attempt count after success — detectable by observing that after a successful item, the next failure uses `delay_for_attempt(0)` instead of the current attempt.
- **Q3**: After a failed `fetch_one_event` (returning `Err`), the next `WatchState` has `attempt` incremented by 1 (saturating).
  - Enforcement: `state.attempt.saturating_add(1)` in state transition.
  - Violation: If Q3 is violated, attempt counter would overflow or not increment — saturating arithmetic prevents overflow.
- **Q4**: `use_instance_watch(namespace)` returns a `ReadSignal<Vec<InstanceView>>` whose value is always sorted by `instance_id` (ascending, lexicographic).
  - Enforcement: `upsert_instance` always sorts after upserting.
  - Violation: Inserting out-of-order instances into the signal's Vec would violate Q4.
- **Q5**: `parse_first_sse_data_payload(body)` returns the first `data:` line content from an SSE body, or `WatchError::InvalidPayload` if none found.
  - Enforcement: Implementation uses `split("\n\n").find_map(...)` — matches SSE double-newline event delimiter.
  - Violation: `parse_first_sse_data_payload("")` → `Err(WatchError::InvalidPayload("missing data: line in SSE stream"))`.
- **Q6**: `parse_first_instance_payload(payload)` handles both plain JSON and key-prefixed formats, returning equivalent `InstanceView` for equivalent data.
  - Enforcement: Unit tests (e.g., `parses_key_prefixed_payload` vs `parses_plain_json_payload`).
  - Violation: `parse_first_instance_payload("invalid json")` → `Err(WatchError::InvalidPayload(...))`.

## Invariants

- **I1**: `BackoffPolicy` invariants: `initial > Duration::ZERO` and `initial <= max`. After construction, `delay_for_attempt(n)` always returns a value in `[initial, max]`.
- **I2**: `WatchState::attempt` is always incremented via `saturating_add(1)`, guaranteeing it never overflows `u32`.
- **I3**: The SSE stream returned by `watch_namespace_with_policy` is infinite — it only ends via consumption (`for_each` awaits forever) or dropped. It never returns `None` from `unfold`.
- **I4**: `use_instance_watch` signal value is a `Vec<InstanceView>` that contains unique `instance_id` entries (no duplicates after upsert).

## Error Taxonomy

```rust
#[derive(Debug, Error)]
pub enum WatchError {
    #[error("request failed: {0}")]
    Request(String),       // HTTP network error, timeout, 503, 404, etc.

    #[error("invalid SSE payload: {0}")]
    InvalidPayload(String), // SSE parsing failure, JSON decode failure, missing data: line
}
```

- `WatchError::Request` — covers all HTTP-level failures: DNS resolution, connection refused, 503 Service Unavailable, 404 Not Found, timeout, etc. The inner `String` is the `reqwest` error message.
- `WatchError::InvalidPayload` — covers SSE frame parse failures (`parse_first_sse_data_payload` failing to find a `data:` line) and JSON decode failures (`parse_first_instance_payload` failing to deserialize the JSON).

## Contract Signatures

```rust
// Public API
pub fn watch_namespace(
    base_url: &str,
    namespace: &str,
) -> impl Stream<Item = Result<InstanceView, WatchError>>

#[must_use]
pub fn use_instance_watch(namespace: String) -> ReadSignal<Vec<InstanceView>>

// Internal
fn watch_namespace_with_policy(
    base_url: &str,
    namespace: &str,
    backoff: BackoffPolicy,
) -> impl Stream<Item = Result<InstanceView, WatchError>>

async fn fetch_one_event(
    client: &reqwest::Client,
    url: &str,
) -> Result<InstanceView, WatchError>

fn parse_first_sse_data_payload(body: &str) -> Result<String, WatchError>
fn parse_first_instance_payload(payload: &str) -> Result<InstanceView, WatchError>
```

## Type Encoding

| Precondition | Enforcement Level | Type / Pattern |
|---|---|---|
| `base_url` non-empty valid URL | Runtime | `Result<T, WatchError::Request>` from reqwest |
| `namespace` non-empty | Runtime | Server returns 404 → `WatchError::Request` |
| `BackoffPolicy::new(initial, max)` valid | Debug-only | `debug_assert!(initial <= max)` |
| SSE stream never terminates | Compile-time | `impl Stream<Item = ...>` infinite via `unfold` |
| `attempt` never overflows | Compile-time | `saturating_add(1)` on `u32` |
| `use_instance_watch` returns sorted Vec | Type-enforced return | `ReadSignal<Vec<InstanceView>>` with sorted upsert |

## Violation Examples (REQUIRED — one per precondition and postcondition)

- **VIOLATES P1**: `watch_namespace("", "payments")` — reqwest rejects empty base URL → `Err(WatchError::Request("builder error: empty domain"))`
- **VIOLATES P2**: `watch_namespace("http://localhost:8080", "")` — URL becomes `/api/v1/watch/` → server 404s → `Err(WatchError::Request("404 Not Found"))`
- **VIOLATES P3**: `BackoffPolicy::new(Duration::from_secs(5), Duration::from_secs(1))` — delay_for_attempt(0) returns 1s (clamped by max) but policy is logically inverted; no compile-time enforcement.
- **VIOLATES Q2**: If the state transition did NOT reset attempt to 0 on success, the next failure after a success would use the OLD attempt value's delay instead of `delay_for_attempt(0)`. This would cause incorrect backoff after recovery. No direct test exists for this internal state — verified by `reconnects_with_backoff_and_recovers` integration test observing delay behavior.
- **VIOLATES Q3**: If `saturating_add` were replaced with `wrapping_add`, extremely long outage sequences (> 2^32 failures) would wrap attempt to 0, causing the backoff to reset to initial delay when it should remain at max. Tested by `backoff_policy_caps_delay_at_max` (which verifies max is never exceeded).
- **VIOLATES Q4**: Calling `upsert_instance` with an unsorted input Vec and NOT sorting would leave the signal's Vec unsorted → ordering-dependent code that assumes sorted would behave incorrectly.
- **VIOLATES Q5**: `parse_first_sse_data_payload("")` → `Err(WatchError::InvalidPayload("missing data: line in SSE stream"))`
- **VIOLATES Q6**: `parse_first_instance_payload("not json at all")` → `Err(WatchError::InvalidPayload("..."))`

## Ownership Contracts (Rust-specific)

- `watch_namespace(base_url, namespace)` — borrows `base_url` and `namespace` (`&str`) for the duration of stream setup. The returned `impl Stream` borrows from the `WatchState` which owns the cloned `String` versions. No ownership transfer.
- `use_instance_watch(namespace: String)` — takes ownership of `namespace` (moves into async block). The `instances_signal` is a `UseSignal` owned by the hook, converted to `ReadSignal` via `.into()`.
- `fetch_one_event(client, url)` — borrows `client` and `url` from the caller. Client is shared across reconnect attempts (no clone per request).
- `upsert_instance(current, next)` — takes ownership of `current` Vec, clones `next.instance_id` for comparison. Returns owned `Vec<InstanceView>`. No shared mutation.
- Clone policy: `InstanceView` is `Clone` (derived). `WatchError` is `Clone` (derived). `BackoffPolicy` is `Clone` (derived). Cloning is intentional for stream item propagation and signal state.

## Non-goals

- Authentication/authorization header injection (not in scope for this bead)
- Connection pooling configuration (reqwest default is used)
- TLS verification configuration (system default)
- SSE `event:` line parsing (only `data:` lines are parsed; `event:` is ignored)
- Graceful shutdown of the stream (no `CancellationToken` integration in this bead)
- WebAssembly-specific timeout behavior (WASM uses `gloo_timers`, non-WASM uses `std::thread::sleep` — these have different timing precision)
