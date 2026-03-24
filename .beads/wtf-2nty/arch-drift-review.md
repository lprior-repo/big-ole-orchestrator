# Architecture Drift Review — wtf-2nty

**Date:** 2026-03-23
**Agent:** architectural-drift

## Files Inspected

| File | Lines | Limit | Verdict |
|------|-------|-------|---------|
| `crates/wtf-cli/src/commands/serve.rs` | 230 | 300 | ✅ PASS |
| `crates/wtf-actor/src/master/state.rs` | 276 | 300 | ✅ PASS |

## DDD / Scott Wlaschin Assessment

### serve.rs (230 lines)
- **Primitive obsession**: None. `ServeConfig` is a proper struct. `nats_url: String` is acceptable — parsing to `NatsConfig` happens at the boundary via `From<ServeConfig>`.
- **Single responsibility**: ✅ Serve command startup, provisioning, and graceful shutdown.
- **Module cohesion**: ✅ Functions well-decomposed (`load_definitions_from_kv`, `drain_runtime`, `provision_storage`, `wait_for_shutdown_signal`).

### state.rs (276 lines — 117 prod / 159 tests)
- **Primitive obsession**: None. `InstanceId` newtype used consistently. Config fields are appropriate infrastructure config.
- **Single responsibility**: ✅ `OrchestratorConfig` + `OrchestratorState` — tightly coupled orchestrator state management.
- **Illegal states unrepresentable**: ✅ State is encapsulated behind `register`/`deregister`/`get`/`has_capacity`.
- **Parse at boundaries**: ✅ `build_instance_args` is a clean factory wiring config into instance arguments.

## Verdict

**STATUS: PERFECT** — No refactoring required. Both files are well under the 300-line limit, DDD-compliant, and exhibit clean module cohesion.
