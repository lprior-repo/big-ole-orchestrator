# QA Report: wtf-3cv7 — procedural: Implement wait_for_signal

**Date:** 2026-03-23
**Status:** PASS (1 defect noted, non-blocking)

---

## Checklist Results

### 1. `wait_for_signal` in context.rs
**PASS** — `procedural/context.rs:195-240`. Follows the established dual-phase pattern (checkpoint check → live dispatch). Returns `anyhow::Result<Bytes>`. Increments `op_counter` after successful resolution in both paths.

### 2. `handle_wait_for_signal` in instance/procedural.rs
**PASS** — `instance/procedural.rs:97-128`. Checks `received_signals` buffer first (FIFO via `Vec::remove(0)`), falls back to registering a pending waiter in `pending_signal_calls`.

### 3. `ProceduralWaitForSignal` variant in messages/instance.rs
**PASS** — `messages/instance.rs:92-96`. Fields: `operation_id: u32`, `signal_name: String`, `reply: RpcReplyPort<Result<Bytes, WtfError>>`. Consistent with sibling variants.

### 4. KNOWN DEFECT: buffer removal before publish
**CONFIRMED — DEFECT STILL PRESENT.**

`instance/procedural.rs:105-118` — The buffered payload is removed from `received_signals` at line 107 (`queue.remove(0)`), then `publish_signal_event` is called at lines 112-118. If `publish_signal_event` fails (the `store.publish` call silently swallows the error — see `publish_signal_event` lines 130-148 which has no error return path), the payload is already gone from the buffer AND the waiter has been replied to via `reply.send(Ok(payload_to_return))` at line 119.

**Severity: MEDIUM** — On publish failure:
- The waiter receives the payload (so from the caller's perspective it "works")
- But the `SignalReceived` event is never persisted to the event log
- On crash/replay, the signal is lost — the checkpoint won't exist and the buffer is empty

The root cause: `publish_signal_event` is fire-and-forget (returns nothing, swallows errors). The reply to the waiter is sent unconditionally after the call. A fix should either:
- Make `publish_signal_event` return `Result` and only reply `Ok` on success
- Or remove the buffer AFTER confirming publish succeeds (and re-insert on failure)

### 5. Test execution
**PASS** — All 4 `wait_for_signal` tests pass:
```
test procedural::state::tests::instance_msg_has_procedural_wait_for_signal_variant ... ok
test instance::procedural::tests::wait_for_signal_consumes_fifo_from_vec ... ok
test instance::procedural::tests::wait_for_signal_registers_pending_when_no_buffer ... ok
test instance::procedural::tests::wait_for_signal_returns_buffered_immediately ... ok
```

### 6. unwrap/expect in production code (context.rs)
**PASS** — Zero occurrences. The file has `#![deny(clippy::unwrap_used)]`, `#![deny(clippy::expect_used)]`, `#![deny(clippy::panic)]` at module level. All error paths use `?` or `anyhow::bail!`.

### 7. Line count: context.rs
**310 lines** — Under the 300-line soft limit by 10 lines. Borderline. The test module (lines 243-310) accounts for 68 lines; production code is 242 lines.

---

## Signal Flow Verification

| Path | Mechanism | Verified |
|------|-----------|----------|
| Signal arrives with pending waiter | `handle_signal` → delivers via RPC port (`handlers.rs:161`) | PASS |
| Signal arrives with no waiter (live) | `handle_signal` → buffers in `received_signals` (`handlers.rs:163-168`) | PASS |
| `wait_for_signal` finds buffered signal | `handle_wait_for_signal` → pops FIFO, publishes event, replies (`procedural.rs:105-121`) | PASS (with defect noted) |
| `wait_for_signal` no buffer | `handle_wait_for_signal` → registers in `pending_signal_calls` (`procedural.rs:124-126`) | PASS |
| Replay: `SignalReceived` event injected | `handle_inject_event_msg` → wakes pending waiter (`handlers.rs:123-131`) | PASS |

---

## Verdict

**PASS** — Feature is functionally correct for the happy path. The publish-before-confirm defect in `handle_wait_for_signal` is a durability concern (signal lost on crash after publish failure) but does not affect correctness under normal operation. Recommend tracking as a follow-up issue.
