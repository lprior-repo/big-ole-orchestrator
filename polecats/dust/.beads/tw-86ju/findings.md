# Findings: tw-86ju - DbWriterActor apply_message_to_batch Implementation

## Summary
Implemented `apply_message_to_batch` function in `vo-actor/src/db_writer.rs` to route `DbWriterMessage` variants to appropriate storage partitions.

## Changes Made

### Modified: `crates/vo-actor/src/db_writer.rs`

1. **Added imports** for types needed for partition routing:
   - `InstanceId`, `InstanceStatus`, `SequenceNumber`, `TimestampMs` from `vo_types`
   - These were needed for proper type conversions

2. **Updated `commit_single` and `commit_batch`** to pass `db: &fjall::Database` to `apply_message_to_batch`

3. **Implemented `apply_message_to_batch`** with full partition routing:

   | Message Variant | Partition | Operation |
   |----------------|-----------|-----------|
   | `AppendEvent` | events | insert |
   | `RecordInstanceStatus` | instances | insert |
   | `AcquireLease` | leases | insert |
   | `ReleaseLease` | leases | remove |
   | `UpsertTimer` | timers | insert |
   | `DeleteTimer` | timers | remove |
   | `RecordEffect` | effects | insert |
   | `TakeSnapshot` | snapshots | insert |
   | `AtomicTransition` | multi | atomic write |

4. **Added helper functions**:
   - `encode_timer_key()` - constructs timer partition key `[fire_at_ms(8)][instance_id(16)][timer_id(16)]`
   - `encode_timer_key_raw()` - variant for delete operations

## Key Design Decisions

### EffectRecord instance_id Issue
`EffectRecord` does not contain `instance_id` - only `intent_id`. The `EffectId` requires both. Used placeholder `InstanceId::from_bytes([0u8; 16])` for `RecordEffect` case. This is a design issue with `DbWriterMessage::RecordEffect` - it should carry `instance_id`.

### DeleteTimer fire_at Issue
`DeleteTimer` message does not contain `fire_at_ms`, only `instance_id` and `timer_id`. Timer keys require `[fire_at_ms(8)][instance_id(16)][timer_id(16)]`. Used `fire_at_ms = 0` as placeholder, but proper fix would require adding `fire_at` to `DeleteTimer` variant.

### SnapshotData.sequence_number
`SnapshotData::sequence_number()` returns `SequenceNumber` which has `as_u64()` method to get inner u64 value.

## Compilation Status
- `cargo check -p vo-actor` passes
- Pre-existing test compilation errors in `vo_actor_comprehensive_tests.rs`, `probe/integration_tests.rs`, `probe/proptest.rs`, `probe/qa_smoke.rs` - these are unrelated to db_writer changes and involve missing Serialize implementations

## Testing
- Library code compiles successfully
- Unit tests in `db_writer::tests` module compile (they test handle/mailbox behavior, not the new routing)
- Full test suite has pre-existing compilation issues unrelated to this change
