# ADR-DEEP: ADR-036 Causation Chain Test Findings

## Bead: ve-gcwv
**Title:** ADR-DEEP: ADR-036 causation chain test
**Type:** task
**Priority:** P0
**Assignee:** veloxide/polecats/dust

## Summary

Created comprehensive test suite verifying causation IDs propagate through the complete workflow lifecycle as specified by ADR-036 (Command Identity, Correlation, and Causation).

## Implementation

Created new test file: `crates/vo-types/tests/adr036_causation_chain_lifecycle_tests.rs`

### Tests Written (12 total):

1. **lifecycle_external_trigger_has_root_causation** - External triggers have root causation ID "external-root"

2. **lifecycle_workflow_start_links_to_trigger** - Workflow start causation links to trigger command

3. **lifecycle_step_chain_propagates_causation** - Each step's causation links to parent step (step-1 → step-2 → step-3 chain)

4. **lifecycle_timer_fired_links_to_wait_command** - Timer fired causation links to the wait command that set it

5. **lifecycle_signal_handler_links_to_signal** - Signal processing causation links to signal receipt

6. **lifecycle_retry_preserves_causation_to_original_command** - Retry causation maintains link to workflow parent, not failed command

7. **lifecycle_full_chain_traceable_from_completion_to_trigger** - Complete chain can be traced backwards from completion to trigger

8. **lifecycle_all_events_share_correlation_id** - All events in a workflow share the same correlation_id

9. **lifecycle_correlation_id_enables_event_filtering** - Events can be filtered by correlation_id to get all events in a business flow

10. **lifecycle_causation_chain_survives_event_serialization** - Causation chain survives JSON round-trip

11. **lifecycle_all_issuer_types_can_appear_in_causation_chain** - All issuer types (ApiClient, System, TimerLoop, RecoveryLoop, Operator) can appear in causation chain

12. **lifecycle_command_envelope_and_event_envelope_have_identical_causation_semantics** - CommandEnvelope and EventEnvelope have identical causation semantics

## ADR-036 Compliance

ADR-036 specifies:
- `causation_id` points to the immediate parent event/command that caused this command
- Every event emitted by the Engine records the command metadata that caused it
- The causation chain enables full traceability through business flows, retries, and compensations

The tests verify:
- Root causation for external triggers
- Parent-child causation links through workflow lifecycle
- Causation preservation across retries (links to parent, not failure)
- All issuer types can participate in causation chain
- Serialization/deserialization preserves causation
- Correlation ID groups all events in a business request

## Test Execution

```
cargo test --package vo-types --test adr036_causation_chain_lifecycle_tests
```

Result: **12 passed** (1 suite, ~0.03s)

## Notes

- Used IdempotencyKey format with dashes (e.g., "external-root") since colons are not allowed characters
- Tests simulate realistic workflow lifecycle scenarios: external triggers, workflow start, step execution, timer callbacks, signal handling, and retries
- All tests use proper ADR-036 semantics for causation chain propagation

## Files Changed

- **Added:** `crates/vo-types/tests/adr036_causation_chain_lifecycle_tests.rs` (622 lines)
