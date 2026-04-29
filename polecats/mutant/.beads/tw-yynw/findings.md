# Findings: tw-yynw - Wire hardline command handler to CLI

## Problem
`HandlerRegistry::default()` did NOT register a `HardlineHandler` for the `hardline` command, even though CLI parsing was wired up (command_key correctly maps `Command::Hardline` to `"hardline"`).

## Root Cause
The `HardlineHandler` was missing from both:
1. The handlers module in `registry.rs`
2. The `HandlerRegistry::default()` registration

## Fix Applied
Added to `crates/vo-cli/src/registry.rs`:

1. **Import**: Added `use std::time::Duration;` for the timeout duration

2. **Handler struct**: Added `HardlineHandler` in the `handlers` module (after `ApiKeyHandler`):
   - Implements `CommandHandler` trait
   - Extracts `target`, `engine_url`, `timeout`, `force`, `dry_run` from `Command::Hardline`
   - Makes HTTP POST to `/{engine_url}/api/v1/hardline` with JSON body containing target, force, and dry_run fields
   - Handles success/failure responses appropriately

3. **Registration**: Added `registry.register(Box::new(handlers::HardlineHandler));` in `HandlerRegistry::default()`

4. **Tests**:
   - Added `assert!(names.contains(&"hardline"));` to `registry_contains_all_commands` test
   - Added `registry_lookup_hardline` test case

## Verification
- The handler is now registered and will be found when `HandlerRegistry::get()` is called with a `Cli { command: Command::Hardline { ... } }`
- The implementation follows the same pattern as `CompensateHandler` which also makes HTTP calls to the engine

## Notes
- The vo-api crate has pre-existing compilation errors (HistoryQueryParams private type issue) unrelated to this change
- The hardline handler implementation POSTs to `/{engine_url}/api/v1/hardline` - this endpoint must exist in the API for the handler to work at runtime