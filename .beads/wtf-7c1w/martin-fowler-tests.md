# Martin Fowler Test Plan

## Feature: Harden serve run-loop runtime correctness

### Scenario: Graceful shutdown drains all tasks in dependency order

**Given**: A running `wtf serve` process with API server and timer loop active  
**When**: SIGTERM or SIGINT is received  
**Then**: 
- Shutdown signal is broadcast to all subsystems
- API server task is awaited to completion
- Timer loop task is awaited to completion  
- Orchestrator stop is invoked
- `drain_runtime` returns `Ok(())` only if all components succeeded

---

## Happy Path Tests

### test_drain_runtime_returns_ok_when_all_tasks_succeed
- **Given**: Valid `shutdown_tx`, two tasks that complete with `Ok(())`, and a `stop_master` closure
- **When**: `drain_runtime(shutdown_tx, api_task, timer_task, stop_master)` is called
- **Then**: Returns `Ok(())`, `stop_master` was invoked, both tasks are drained

### test_shutdown_signal_broadcasts_to_all_receivers
- **Given**: A `watch` channel with multiple receivers
- **When**: `shutdown_tx.send(true)` is called
- **Then**: All receivers observe the `true` value

### test_stop_master_called_after_task_await
- **Given**: Tasks that complete successfully
- **When**: `drain_runtime` completes
- **Then**: `stop_master` closure is invoked after both `await` statements

---

## Error Path Tests

### test_drain_runtime_returns_err_when_api_task_fails
- **Given**: `api_task` resolves to `Err(EApi)`, `timer_task` succeeds
- **When**: `drain_runtime` is called
- **Then**: Returns `Err(anyhow::Error)` with context `"api server failed"`

### test_drain_runtime_returns_err_when_timer_task_fails
- **Given**: `api_task` succeeds, `timer_task` resolves to `Err(ETimer)`
- **When**: `drain_runtime` is called
- **Then**: Returns `Err(anyhow::Error)` with context `"timer loop failed"`

### test_drain_runtime_returns_err_when_api_task_join_panics
- **Given**: `api_task` is a JoinHandle that panics when awaited
- **When**: `drain_runtime` awaits it
- **Then**: Returns `Err(anyhow::Error)` with context `"api task join failed"`

### test_drain_runtime_returns_err_when_timer_task_join_panics
- **Given**: `timer_task` is a JoinHandle that panics when awaited
- **When**: `drain_runtime` awaits it
- **Then**: Returns `Err(anyhow::Error)` with context `"timer task join failed"`

### test_drain_runtime_returns_err_when_shutdown_send_channel_closed
- **Given**: `shutdown_tx` is dropped before `drain_runtime` calls `send(true)`
- **When**: `drain_runtime` is called
- **Then**: Returns `Err` from `shutdown_tx.send(true)`

---

## Edge Case Tests

### test_drain_runtime_handles_tasks_completing_in_any_order
- **Given**: `api_task` and `timer_task` with indeterminate completion order
- **When**: `drain_runtime` awaits both
- **Then**: Both are awaited correctly regardless of which completes first

### test_drain_runtime_with_identical_error_types
- **Given**: Both tasks fail with the same error type
- **When**: `drain_runtime` is called
- **Then**: First error in source order is propagated via `anyhow::Context`

### test_drain_runtime_does_not_panic_on_double_sender_drop
- **Given**: `shutdown_tx` is dropped after `send` succeeds
- **When**: `drain_runtime` completes
- **Then**: No panic; resources cleaned up correctly

---

## Contract Verification Tests

### test_precondition_p1_shutdown_tx_send_works_when_valid
- **Given**: A valid `watch::Sender<bool>` that is not closed
- **When**: `send(true)` is called
- **Then**: Returns `Ok(())` and receivers observe `true`

### test_precondition_p3_fn_once_called_exactly_once
- **Given**: A `FnOnce` closure
- **When**: The closure is called via `drain_runtime`
- **Then**: It is invoked exactly once; attempting to call again would not compile

### test_postcondition_q1_signal_broadcast
- **Given**: `shutdown_tx` and multiple cloned `shutdown_rx` receivers
- **When**: `drain_runtime` calls `shutdown_tx.send(true)`
- **Then**: All receivers see `true` before `drain_runtime` returns

### test_postcondition_q5_ok_only_when_all_tasks_ok
- **Given**: `api_task` returns `Ok(())`, `timer_task` returns `Ok(())`
- **When**: `drain_runtime` returns
- **Then**: Result is `Ok(())`
- **And**: If either task returns `Err`, final result is `Err`

### test_invariant_i2_all_tasks_terminated_after_drain
- **Given**: A running `drain_runtime` call
- **When**: It returns `Ok(())`
- **Then**: Both `JoinHandle`s are fully consumed (awaited to completion)

### test_invariant_i3_channel_dropped_after_receivers_complete
- **Given**: `shutdown_tx` sender and receivers
- **When**: `drain_runtime` returns
- **Then**: `shutdown_tx` is dropped (sender destroyed), receivers may still hold a copy

---

## Contract Violation Tests

### test_violates_p1_send_on_closed_channel_returns_err
- **Given**: `shutdown_tx` that has been dropped
- **When**: `shutdown_tx.send(true)` is called
- **Then**: Returns `Err(SendError)` — NOT a panic

### test_violates_p2_already_completed_join_handle
- **Given**: A `JoinHandle` resolved to `Ok(())` immediately via `spawn`
- **When**: `drain_runtime` awaits it
- **Then**: Completes normally — caller responsibility not to pass zombie handles

### test_violates_q2_api_task_error_propagated
- **Given**: `api_task` resolves to `Err(io::Error)`  
- **When**: `drain_runtime` awaits it
- **Then**: Returns `Err(anyhow::Error)` containing `"api server failed"` context

### test_violates_q5_any_task_failure_causes_err_return
- **Given**: `timer_task` returns `Err(etimer)` while `api_task` returns `Ok(())`
- **When**: `drain_runtime` completes
- **Then**: Returns `Err` — `Ok(())` is impossible if any task fails

---

## End-to-End Scenario Test

### test_full_graceful_shutdown_sequence
- **Given**: A `wtf serve` process with orchestrator, API, and timer running
- **When**: SIGTERM is sent to the process
- **And**: `wait_for_shutdown_signal()` completes
- **And**: `drain_runtime` is invoked
- **Then**:
  1. `shutdown_tx.send(true)` broadcasts to API and timer
  2. Both tasks observe shutdown and complete
  3. `master.stop(None)` is called
  4. `drain_runtime` returns `Ok(())`
  5. Process exits cleanly with code 0
