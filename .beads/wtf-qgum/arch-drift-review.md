# Arch Drift Review — wtf-qgum

**STATUS: REFACTORED**

## File Under Review

`crates/wtf-worker/src/builtin.rs` — **421 lines** (over 300-line limit)

## Finding

The file contained 91 lines of production code and 330 lines of tests in a single monolithic `#[cfg(test)] mod tests` block.

## Refactoring Applied

Converted `builtin.rs` into a `builtin/` directory module and split by responsibility:

| File | Lines | Responsibility |
|------|-------|----------------|
| `builtin/mod.rs` | 99 | Production code (handlers, parser, registration) |
| `builtin/test_helpers.rs` | 18 | Shared `make_task()` constructor for tests |
| `builtin/tests_echo.rs` | 63 | Echo handler tests (happy path, edge cases, invariant) |
| `builtin/tests_sleep.rs` | 249 | Sleep handler + parse_sleep_ms tests (all paths) |

All files are **under 300 lines**.

## DDD Compliance

- ✅ **Parse, don't validate** — `parse_sleep_ms` is a pure function at the boundary
- ✅ **No primitive obsession** — `ActivityId`, `InstanceId`, `NamespaceId` are proper NewTypes (from `wtf_common`)
- ✅ **Single responsibility** — each file has one clear purpose
- ✅ **Constants for domain literals** — `SLEEP_PARSE_ERR`, `SLEPT_RESULT`
- ✅ **Zero `unwrap()`/`expect()`** — enforced via clippy lints

## Verification

- `cargo check -p wtf-worker` ✅
- `cargo test -p wtf-worker -- builtin` — 32/32 passed ✅
- No public API changes — `register_defaults`, `echo_handler`, `sleep_handler` unchanged
