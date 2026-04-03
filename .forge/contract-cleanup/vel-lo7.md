## Summary

Add lease-record and fence-token domain types in `vo-types` for execution ownership and fencing.

## Source ADRs

- `docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md`

## Scope

- Define the durable lease record type.
- Define the fence token type used to identify the current execution owner.
- Keep the change confined to `vo-types` domain types.

## Constraints

- Lease and fencing values must support ownership checks and stale-writer rejection.
- Do not spread implementation into unrelated crates.
- Use current workspace paths only.

## Relevant Files

- `crates/vo-types/src/types.rs`
- `crates/vo-types/src/state.rs`

## Acceptance

- Lease and fence token types compile in `vo-types`.
- Tests prove the types support current-owner identification and fencing semantics.
- Error-path tests cover invalid or incomplete lease metadata.
