# Kani Justification: wtf-m60g

- **bead_id**: wtf-m60g
- **phase**: STATE-5.7
- **updated_at**: 2026-03-23T19:20:00Z

## Critical State Machines
None. `publish_instance_started` is a guard-and-publish function:
1. Check event_log.is_empty() (crash recovery guard)
2. Check event_store is Some (prerequisite guard)
3. Publish single event via trait method
4. No state mutation in this function (publishing is a side effect)

## Why Kani Adds Nothing
1. The function has exactly two early-return guards and one fallible operation
2. No loops, no branches beyond the two guards
3. The crash recovery guard (`event_log.is_empty()`) is a single boolean check
4. No shared mutable state — reads `&InstanceArguments` and `&[WorkflowEvent]` (immutable borrows)
5. The EventStore trait dispatch is well-tested by the RecordingEventStore mock

## What Tests Already Guarantee
- `publish_instance_started_skips_on_nonempty_log` — crash recovery guard
- `publish_instance_started_errors_on_missing_store` — prerequisite guard
- `publish_instance_started_publishes_event_on_fresh` — happy path

## Conclusion
No state machine to model. Two guards and one I/O call. Kani would not discover anything the tests and borrow checker don't already cover.
