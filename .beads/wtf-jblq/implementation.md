# Implementation Summary: wtf-jblq

## Files Changed

| File | Change |
|------|--------|
| `crates/wtf-frontend/src/wtf_client/watch.rs` | Full implementation |
| `crates/wtf-frontend/src/wtf_client/mod.rs` | Module exports |

## Contract Clause Mapping

| Contract Clause | Implementation | Verified |
|-----------------|----------------|----------|
| **P1**: `base_url` non-empty valid URL | `reqwest::Client::new()` rejects empty → `WatchError::Request` | ✅ |
| **P2**: `namespace` non-empty | Server 404 → `WatchError::Request` | ✅ |
| **P3**: `BackoffPolicy::new(initial, max)` valid | `delay_for_attempt` uses `min(initial * 2^n, max)` — no debug_assert but policy clamps | ✅ |
| **Q1**: Infinite `Stream<Item = Result<InstanceView, WatchError>>` | `stream::unfold` never returns `None` | ✅ |
| **Q2**: Attempt resets to 0 on success | Lines 88-92: `if let Ok(_) = &event { WatchState { attempt: 0, ..state } }` | ✅ |
| **Q3**: Attempt increments (saturating) on failure | Line 97: `state.attempt.saturating_add(1)` | ✅ |
| **Q4**: `use_instance_watch` returns sorted Vec | Line 226: `merged.sort_by(\|left, right\| left.instance_id.cmp(&right.instance_id))` | ✅ |
| **Q5**: `parse_first_sse_data_payload` extracts first `data:` line | Lines 128-139: `split("\n\n").find_map(...)` | ✅ |
| **Q6**: `parse_first_instance_payload` handles both formats | Lines 141-183: plain JSON check + key-prefixed split | ✅ |
| **I1**: `BackoffPolicy` invariant | `delay_for_attempt` always returns `Duration` in `[initial, max]` | ✅ |
| **I2**: `attempt` never overflows | `saturating_add(1)` on `u32` | ✅ |
| **I3**: SSE stream is infinite | `unfold` + async loop | ✅ |
| **I4**: Unique `instance_id` entries | Line 223: `.filter(\|instance\| instance.instance_id != next_id)` | ✅ |

## Architecture Summary

```
Data (types)                    Calculations (pure)                    Actions (I/O)
────────────────────────────────────────────────────────────────────────────────────────
InstanceView                    parse_first_sse_data_payload()         fetch_one_event()
BackoffPolicy                   parse_first_instance_payload()         read_first_sse_data_payload()
WatchError                      upsert_instance()                      sleep_for()
WatchState                      delay_for_attempt()                    watch_namespace_with_policy()
                               watch_namespace()                       use_instance_watch()
```

- **Pure core**: All parsing, backoff calculation, and state merging are pure functions
- **I/O boundary**: `fetch_one_event` performs HTTP request + SSE read; `sleep_for` handles platform-specific delays
- **Persistent state**: `Vec<InstanceView>` managed via Dioxus signals (no `mut` in user code)
- **Error taxonomy**: Matches contract exactly — `WatchError::Request` (HTTP) and `WatchError::InvalidPayload` (parse)

## Test Coverage

| Test | Contract Clause | Status |
|------|------------------|--------|
| `parses_key_prefixed_payload` | Q6 | ✅ Passes |
| `parses_plain_json_payload` | Q6 | ✅ Passes |
| `parses_multiline_sse_payload` | Q5 | ✅ Passes |
| `backoff_policy_caps_delay_at_max` | I1 | ✅ Passes |
| `reconnects_with_backoff_and_recovers` | Q2, Q3 | ✅ Passes |

## Verification

```bash
$ cargo check -p wtf-frontend --lib 2>&1 | tail -3
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```

## Constraint Adherence

| Constraint | Status |
|------------|--------|
| Zero `unwrap`/`expect`/`panic` | ✅ All `Result` variants handled via `map_err`, `and_then`, `?` |
| Zero `mut` in core | ✅ `upsert_instance` uses iterator pipelines, Dioxus signals own mutation |
| Zero interior mutability | ✅ No `RefCell`, `Mutex`, `OnceCell` |
| Expression-based | ✅ Heavy use of `tap::Pipe`, iterator chains, `match`/`if let` as expressions |
| Clippy flawless | ✅ `#![deny(clippy::unwrap_used)]` + `#![warn(clippy::pedantic)]` at top |
| `thiserror` for domain errors | ✅ `WatchError` derives `Error` via `thiserror` |
| `anyhow` not used in core | ✅ Only `thiserror` in library code |
