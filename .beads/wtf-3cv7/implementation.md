# Implementation Summary

## Metadata
- **bead_id**: vo-3cv7
- **bead_title**: procedural: Implement wait_for_signal in WorkflowContext
- **phase**: STATE-3
- **updated_at**: 2026-03-23T00:00:00Z

## Files Modified

### 1. `crates/vo-actor/src/procedural/state/mod.rs` (lines 30-66, 280-296)
- **Change**: Added `received_signals: HashMap<String, Vec<Bytes>>` field to `ProceduralActorState` with `#[serde(default)]`
- **Change**: Added `SignalReceived` arm to `apply_event` that creates a checkpoint at the current `operation_counter`
- **Design decision**: `apply_event` does NOT consume from `received_signals` — buffer consumption is the sole responsibility of `handle_wait_for_signal`. This prevents `apply_event` from destroying buffers during live execution when `handle_signal` calls `inject_event`.

### 2. `crates/vo-actor/src/messages/instance.rs` (lines 88-93)
- **Change**: Added `ProceduralWaitForSignal { operation_id: u32, signal_name: String, reply: RpcReplyPort<Result<Bytes, VoError>> }` variant to `InstanceMsg`

### 3. `crates/vo-actor/src/procedural/context.rs` (lines 188-233)
- **Change**: Added `wait_for_signal(&self, signal_name: &str) -> anyhow::Result<Bytes>` method to `WorkflowContext`
- **Pattern**: Exact dual-phase pattern matching `activity()` — (1) load op_id, (2) check checkpoint for replay, (3) dispatch `ProceduralWaitForSignal` for live, (4) increment op_counter after consuming result

### 4. `crates/vo-actor/src/instance/procedural.rs` (lines 96-164, 295-370)
- **Change**: Added `handle_wait_for_signal` handler — checks `received_signals` buffer (FIFO via `Vec::remove(0)`), if hit: publishes `SignalReceived` event, returns payload. If miss: registers `pending_signal_calls` entry.
- **Change**: Added `publish_signal_event` helper for event persistence on buffer-hit path
- **Refactor**: Extracted `make_procedural_state()` test helper

### 5. `crates/vo-actor/src/instance/handlers.rs` (lines 4-5, 67-75, 113-121, 139-152, 596-613)
- **Change**: Added `use super::lifecycle::ParadigmState` import
- **Change**: Wired `ProceduralWaitForSignal` dispatch in `handle_procedural_msg`
- **Change**: Extended `handle_inject_event_msg` to wake signal waiters on `SignalReceived` replay
- **Change**: Extended `handle_signal` to buffer signals in `received_signals` when no pending waiter exists
- **Change**: Updated `handle_signal_publishes_event_when_no_pending_call` test to verify buffering

## Tests Written

### Unit tests in `crates/vo-actor/src/procedural/state/tests.rs`:
| Test Name | Status |
|-----------|--------|
| `signal_received_creates_checkpoint` | PASS |
| `signal_received_does_not_affect_received_signals_buffer` | PASS |
| `duplicate_signal_received_returns_already_applied` | PASS |
| `instance_msg_has_procedural_wait_for_signal_variant` | PASS (compile-time guard) |
| `_context_has_wait_for_signal_method` | PASS (compile-time guard) |

### Unit tests in `crates/vo-actor/src/instance/procedural.rs`:
| Test Name | Status |
|-----------|--------|
| `wait_for_signal_returns_buffered_immediately` | PASS |
| `wait_for_signal_registers_pending_when_no_buffer` | PASS |
| `wait_for_signal_consumes_fifo_from_vec` | PASS |

### Updated test in `crates/vo-actor/src/instance/handlers.rs`:
| Test Name | Status |
|-----------|--------|
| `handle_signal_publishes_event_when_no_pending_call` | PASS (now verifies buffer) |

## Constraint Adherence

- **Zero unwrap/expect**: No `.unwrap()` or `.expect()` in new production code (tests are exempt per functional-rust skill)
- **Data->Calc->Actions**: `apply_event` remains pure; handler functions are the Actions layer
- **Make illegal states unrepresentable**: `received_signals: HashMap<String, Vec<Bytes>>` enforces FIFO multi-signal buffering via type structure
- **Parse at boundary**: Signal name is a `String` passed through the system; no validation needed (signal names are free-form identifiers)
- **Expression-based**: `handle_signal` and `handle_wait_for_signal` use match/let-else patterns

## Key Design Decisions

1. **`apply_event` does NOT consume `received_signals` buffer**: During live execution, `handle_signal` calls `inject_event` (which calls `apply_event`). If `apply_event` consumed the buffer, the signal would be lost before `wait_for_signal` could check it. Buffer consumption is exclusive to `handle_wait_for_signal`.

2. **`Vec::remove(0)` for FIFO**: The spec requires FIFO ordering for multiple signals of the same name. `Vec::pop()` is LIFO; `Vec::remove(0)` provides correct FIFO behavior.

3. **Checkpoint created by `apply_event`, not by `handle_wait_for_signal`**: On buffer-hit, `handle_wait_for_signal` publishes a `SignalReceived` event. `inject_event` → `apply_event` creates the checkpoint. This ensures the event is persisted BEFORE the workflow continues, matching the spec's durability requirement.

## Verification

- **cargo test -p vo-actor**: 91/91 lib tests pass, 32/32 integration tests pass
- **cargo clippy -p vo-actor --lib**: Zero errors (pre-existing warnings in other crates only)
