# Implementation: DEFECT-06 + DEFECT-05 Fixes

## Summary

Applied targeted fixes for two defects in the `get_workflow` handler path:
- **DEFECT-06 (CRITICAL):** Instance actor timeout now returns 503 SERVICE_UNAVAILABLE instead of being silently collapsed into 404 NOT_FOUND.
- **DEFECT-05 (HIGH):** `map_actor_error` now discriminates all ractor error variants with distinct error codes and descriptive messages instead of collapsing everything into a generic "actor failed" 500.

## Changed Files

| File | Change |
|------|--------|
| `crates/vo-actor/src/messages/errors.rs` | Added `GetStatusError` enum with `Timeout` variant |
| `crates/vo-actor/src/messages/orchestrator.rs` | Updated `GetStatus` reply type from `RpcReplyPort<Option<...>>` to `RpcReplyPort<Result<Option<...>, GetStatusError>>` |
| `crates/vo-actor/src/master/handlers/status.rs` | Changed return type to `Result<Option<InstanceStatusSnapshot>, GetStatusError>`; timeout maps to `Err(Timeout)`, missing actor maps to `Ok(None)` |
| `crates/vo-actor/src/master/handlers/list.rs` | Updated caller to match on `Result` instead of `Option`; skips timed-out instances explicitly |
| `crates/vo-actor/src/lib.rs` | Exported `GetStatusError` from crate root |
| `crates/vo-api/src/handlers/workflow.rs` | Updated `map_status_result` to handle `GetStatusError::Timeout` → 503; updated `get_instance_paradigm` to unwrap double-Result; improved `map_actor_error` with 5 distinct error variants |

## Constraint Adherence

- **Zero `unwrap`/`expect`**: All new code uses `match`, `if let`, or `map_err`. No panicking in non-test code.
- **No mutability**: All changes are pure pattern matching — no `mut` introduced.
- **Expression-based**: All functions remain expression-based with early-return patterns.
- **<25 line functions**: `status.rs` handle_get_status is 21 lines; `map_actor_error` is 24 lines; `map_status_result` is 16 lines.
- **Backward compatibility**: The `OrchestratorMsg::GetStatus` reply type changed, but the orchestrator dispatch in `master/mod.rs` required zero changes — `reply.send()` forwards the `Result` transparently. The `list_workflows` handler is unaffected (it calls `ListActive`, not `GetStatus`).

## Semantic Correctness (DEFECT-06)

Before: `handle_get_status` returned `Option<InstanceStatusSnapshot>`. Both "not in registry" and "actor timed out" produced `None` → HTTP 404.

After: Three distinct outcomes:
1. `Ok(Some(snapshot))` — instance exists and responded → HTTP 200
2. `Ok(None)` — instance not in registry → HTTP 404
3. `Err(GetStatusError::Timeout)` — instance exists but actor unresponsive → HTTP 503

## Error Discrimination (DEFECT-05)

Before: `map_actor_error` had two arms — `Timeout` → 503, everything else → generic 500 "actor failed".

After: Five distinct error arms covering all ractor 0.15.12 variants:
- `CallResult::Timeout` → 503 `actor_timeout`
- `CallResult::SenderError` → 503 `sender_error`
- `MessagingErr::ChannelClosed` → 503 `channel_closed`
- `MessagingErr::InvalidActorType` → 500 `invalid_actor_type`
- `MessagingErr::SendErr` → 503 `send_error`

This benefits all handlers that use `map_actor_error` (`start_workflow`, `terminate_workflow`, `list_workflows`, `get_workflow`).

## Test Results

- `vo-actor`: 68/68 unit tests pass ✅
- `vo-api`: 33/33 unit tests pass ✅
- `cargo check --workspace`: clean ✅
- `journal_test`: 7 failures — pre-existing known issue (AGENTS.md item 1), not caused by these changes
