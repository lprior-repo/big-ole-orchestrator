## Summary

Add explicit schema-version fields to durable record types in `vo-types` so replay and upcasting can branch on known versions instead of inferring payload shape.

## Source ADRs

- `docs/adr/v2/ADR-035-v2-event-schema-evolution-and-upcasting.md`

## Scope

- Introduce durable-record schema version markers in the relevant `vo-types` record families.
- Keep the change confined to `vo-types`.
- Ensure downstream replay code can distinguish persisted revisions without guessing.

## Constraints

- Do not introduce dependencies on non-`vo-types` crates.
- Do not rely on implicit version inference from payload contents.
- Use current workspace paths only.

## Relevant Files

- `crates/vo-types/src/events.rs`
- `crates/vo-types/src/types.rs`

## Acceptance

- Durable records carry explicit schema version information.
- Tests prove version markers are present and usable for replay/upcasting decisions.
- Invalid or missing durable version handling is covered by error-path tests.
