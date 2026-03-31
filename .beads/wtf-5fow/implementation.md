# wtf-5fow implementation

## What changed

- Added `crates/wtf-actor/tests/heartbeat_expiry_recovery.rs` with four live-NATS integration tests:
  - active instance ignores `HeartbeatExpired`
  - watcher shutdown exits cleanly
  - crash recovery restores FSM instance after heartbeat removal
  - duplicate expiry signals do not create duplicate recovered actors
- Fixed `wtf-storage` instance metadata lookup to search the real namespaced KV keys in `crates/wtf-storage/src/lib.rs`.
- Added `current_state: Option<String>` to `InstanceStatusSnapshot` and threaded it through actor/API mapping so recovery assertions can verify FSM state.
- Hardened `publish_instance_started` in `crates/wtf-actor/src/instance/init.rs` so snapshot-based recovery with an empty replay tail does not republish `InstanceStarted`.
- Tightened `instance_id_from_heartbeat_key` in `crates/wtf-actor/src/heartbeat.rs` to reject empty ids and extra path segments.
- Added missing rustdoc `# Errors` sections in `wtf-common` for the clippy failures initially encountered.

## Verification run

- `cargo check -p wtf-actor`
- `cargo check -p wtf-api`
- `cargo test -p wtf-actor parse_hb_prefix_only_returns_none -- --nocapture`
- `cargo test -p wtf-actor snapshot_recovery_without_tail_skips_started_event -- --nocapture`
- `cargo test -p wtf-actor --test heartbeat_expiry_recovery -- --test-threads=1 --nocapture`
- `cargo test -p wtf-actor --test spawn_workflow_test -- --test-threads=1`
- `cargo test -p wtf-api --test get_workflow_handler_test -- --test-threads=1`

## Remaining issue

Strict clippy for this bead is still blocked by broader existing warnings in `crates/wtf-storage`, including:

- rustdoc markdown/backtick issues across multiple files
- missing `# Errors` docs in storage APIs
- style lints like `uninlined_format_args`, `map_unwrap_or`, `ref_option`, `manual_let_else`
- a truncation lint in `crates/wtf-storage/src/journal.rs`

These failures are not introduced by `wtf-5fow`, but they prevent a clean `cargo clippy -D warnings` gate for dependent crates.
