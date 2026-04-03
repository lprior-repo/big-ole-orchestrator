## Summary

Implement a `DbWriterActor` in `vo-actor` that batches durable event writes to storage with bounded backpressure.

## Source ADRs

- `docs/adr/v2/ADR-002-v2-fjall-storage.md`
- `docs/adr/v2/ADR-015-v2-actor-invariants-backpressure.md`

## Scope

- Define `WriteEventMsg { envelope, reply }` for handing validated event writes to the writer actor.
- Buffer writes into batches.
- Flush when the batch reaches 100 items or when 10ms elapse.
- Commit each batch atomically.
- Expose mailbox depth so upstream code can reject work before overload.

## Constraints

- Mailbox must remain bounded at `10_000` items.
- Backpressure must trigger before the mailbox is exhausted.
- Sequence validation remains outside `DbWriterActor`.
- On commit failure, every sender in the batch receives an error.

## Relevant Files

- `crates/vo-actor/src/lib.rs`
- `crates/vo-storage/src/append.rs`
- `crates/vo-storage/src/lib.rs`

## Acceptance

- A batch of 100 writes flushes atomically.
- A partial batch flushes after 10ms.
- All senders receive `Ok(())` on successful commit.
- All senders receive an error if the batch commit fails.
- Overload handling rejects new work before mailbox exhaustion.
