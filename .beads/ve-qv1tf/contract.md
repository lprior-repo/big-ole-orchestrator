# Contract: Runtime Configuration Types (ve-qv1tf)

## Overview

This contract specifies the Design-by-Contract (DbC) terms for runtime configuration types used in veloxide. Runtime configuration enables hot-reloading of system parameters without restart.

## Reference

- Parent Issue: ve-ag3n1 (Runtime configuration for veloxide polecat)
- Phase: Go State 1 - Design-by-contract specification

---

## Types

### RuntimeConfig Struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Worker pool configuration
    pub worker: WorkerConfig,
    /// Executor configuration
    pub executor: ExecutorConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Actor system configuration
    pub actor: ActorConfig,
    /// Observability configuration
    pub observability: ObservabilityConfig,
}
```

### WorkerConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Maximum number of concurrent workers
    pub max_workers: usize,
    /// Worker timeout duration
    pub worker_timeout_secs: u64,
    /// Queue depth per worker
    pub queue_depth: usize,
}
```

### ExecutorConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    /// Maximum concurrent subprocess spawns
    pub max_concurrent_binaries: usize,
    /// Subprocess timeout
    pub subprocess_timeout_secs: u64,
    /// Memory limit per subprocess (bytes)
    pub subprocess_memory_limit_bytes: u64,
}
```

### StorageConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Fjall data directory
    pub data_dir: PathBuf,
    /// Maximum storage size (bytes)
    pub max_size_bytes: u64,
    /// Snapshot interval
    pub snapshot_interval_secs: u64,
}
```

### ActorConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    /// Semaphore configuration
    pub semaphore: SemaphoreConfig,
    /// Reanimator configuration
    pub reanimator: ReanimatorConfig,
    /// Message router configuration
    pub router: RouterConfig,
}
```

### ObservabilityConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Tracing endpoint
    pub tracing_endpoint: Option<String>,
    /// Metrics interval
    pub metrics_interval_secs: u64,
    /// Log level
    pub log_level: LogLevel,
}
```

---

## Validation Invariants

### I1: Worker Pool Bounds
- `max_workers > 0`
- `max_workers <= MAX_WORKERS` (implementation limit)
- `queue_depth > 0`

### I2: Executor Bounds
- `max_concurrent_binaries > 0`
- `max_concurrent_binaries <= MAX_CONCURRENT_BINARIES`
- `subprocess_timeout_secs > 0`
- `subprocess_memory_limit_bytes >= MIN_MEMORY_LIMIT`

### I3: Storage Bounds
- `data_dir` must be a valid directory path
- `max_size_bytes > 0`
- `snapshot_interval_secs > 0`

### I4: Actor Config Consistency
- Semaphore permits must be <= executor concurrent binaries
- Reanimator retry policy must have `max_attempts > 0`
- Router queue depth must be > 0

---

## Preconditions

### P1: Config Loading
**Requires**: Config file exists and is readable
**Enforces**: Parsed into `RuntimeConfig` struct

### P2: Config Validation
**Requires**: `RuntimeConfig` struct with all fields populated
**Enforces**: All invariants I1-I4 satisfied, returns `Result<(), ValidationError>`

### P3: Config Hot Reload
**Requires**: New config file content, current running config
**Enforces**: Atomic swap to new config if validation passes

---

## Postconditions

### Q1: After Successful Load
- `current() == loaded_config`
- All invariants satisfied
- Previous config returned for rollback

### Q2: After Failed Validation
- `current() == previous_config` (unchanged)
- Error with specific violation details

### Q3: After Hot Reload
- `current() == new_config`
- Subsystems notified of config change
- Metrics emitted for observability

---

## Error Taxonomy

### ConfigError Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Failed to read config: {path}")]
    ReadError { path: PathBuf, cause: String },

    #[error("Parse error: {details}")]
    ParseError { details: String },

    #[error("Validation failed: {violations}")]
    ValidationFailed { violations: Vec<ValidationViolation> },

    #[error("Watcher error: {cause}")]
    WatcherError { cause: String },

    #[error("Channel closed unexpectedly")]
    ChannelClosed,
}
```

### ValidationViolation Struct

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationViolation {
    pub field: String,
    pub constraint: String,
    pub actual_value: String,
}
```

---

## Test Scenarios

### Happy Paths

1. **T1**: Load valid config file → config applied successfully
2. **T2**: Hot reload with valid new config → atomic swap
3. **T3**: Config validation passes all invariants → no error

### Error Paths

4. **T2**: Load missing config file → `FileNotFound` error
5. **T3**: Load malformed JSON → `ParseError` with details
6. **T4**: Load config violating invariants → `ValidationFailed` with violations
7. **T5**: Hot reload with invalid config → current config unchanged

---

## Implementation Notes

- Use `HotReloadConfig<T>` from `config_hot_reload.rs` as foundation
- Implement `ConfigValidator<RuntimeConfig>` trait
- Ensure all config changes are atomic
- Emit observability events on config changes
