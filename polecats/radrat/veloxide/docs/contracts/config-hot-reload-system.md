## Contract: Config Hot-Reload System

### 1. Purpose

Defines the contract for configuration hot-reload with file watching, atomic swap, validation, and rollback. This contract establishes the types, invariants, lifecycle states, and error taxonomy for the configuration hot-reload subsystem, enabling configuration changes to be applied atomically with validation and the ability to rollback on failure.

### 2. Source ADRs

- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (atomic update patterns)
- `docs/adr/v2/ADR-041-v2-managed-connector-runtime-contract.md` (prepare/commit pattern)

### 3. Config Types

#### 3.1 HotReloadConfig\<T\>

Generic configuration wrapper with pending/committed state pattern.

```
HotReloadConfig<T: Clone + Send + Sync> {
  current: RwLock<T>,      // Currently active config
  pending: RwLock<Option<T>>, // Staged config awaiting commit
  path: PathBuf,            // Config file path
  validator: Arc<dyn ConfigValidator<T>>,
}
```

#### 3.2 ConfigValidator\<T\>

Trait for validating configuration before applying.

```
trait ConfigValidator<T: Clone + Send + Sync>: Send + Sync {
  fn validate(&self, config: &T) -> Result<(), String>;
}
```

#### 3.3 FileWatcher

Low-level file system watcher using the `notify` crate.

```
FileWatcher {
  watcher: RecommendedWatcher,
  path: PathBuf,
  recursive: bool,
}
```

#### 3.4 FilteredFileWatcher

FileWatcher with glob pattern filtering.

```
FilteredFileWatcher {
  watcher: RecommendedWatcher,
  path: PathBuf,
  config: WatcherConfig,
}
```

#### 3.5 DebouncedFileWatcher

FileWatcher with debouncing to coalesce rapid file changes.

```
DebouncedFileWatcher {
  watcher: RecommendedWatcher,
  path: PathBuf,
  config: WatcherConfig,
  debouncer: Option<Debouncer>,
}
```

#### 3.6 WatcherConfig

Configuration for file watchers.

```
WatcherConfig {
  recursive: bool,                    // Recursive directory watching
  debounce_duration: Option<Duration>, // Debounce window
  patterns: Vec<String>,             // Glob patterns to match
}
```

#### 3.7 EventChannel

Async channel for distributing debounced file events.

```
EventChannel {
  tx: tokio::sync::mpsc::Sender<DebouncedFileEvent>,
}
```

#### 3.8 DebouncedFileEvent

File events produced by the debouncer.

```
enum DebouncedFileEvent {
  Modify(PathBuf),
  Delete(PathBuf),
}
```

### 4. Lifecycle States

#### 4.1 HotReloadConfig State Machine

```
ConfigState {
  Validated,    // Current config is valid and active
  Pending,     // New config staged, awaiting commit/rollback
  Reloading,   // Actively reading from file
}
```

#### 4.2 ConfigTransition Events

```
enum ConfigTransition {
  TryUpdate(T),        // Stage new config with validation
  Commit,              // Apply pending config atomically
  Rollback,            // Discard pending config
  ReloadFromFile,      // Read and apply from disk
}
```

### 5. Invariants (CHR-*)

- **CHR-001**: `current` always holds a valid, validator-approved configuration
- **CHR-002**: `pending` is `None` or `Some(valid_config)` — never contains invalid config
- **CHR-003**: Only one pending config can exist at a time — calling `try_update` overwrites any existing pending
- **CHR-004**: `commit()` is the only operation that promotes `pending` to `current`
- **CHR-005**: `rollback()` clears pending without modifying current
- **CHR-006**: `reload_from_file()` updates current directly, bypassing pending state
- **CHR-007**: All config updates (direct or via pending) must pass validation before modifying current
- **CHR-008**: `current()` returns a clone, preserving internal state isolation
- **CHR-009**: FileWatcher operations are atomic — watch/unwatch either succeed or have no effect
- **CHR-010**: DebouncedFileWatcher coalesces multiple Modify events for the same path into single yields
- **CHR-011**: Delete events cancel any pending debounce timer for that path
- **CHR-012**: EventChannel send is async and bounded — callers must handle ChannelClosed error

### 6. Error Taxonomy

```rust
enum Error {
    // File system errors
    #[error("Config file not found: {0}")]
    ConfigFileNotFound(PathBuf),

    #[error("Failed to read config file: {0}")]
    ReadError(PathBuf),

    // Parse and validation errors
    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    // Watcher errors
    #[error("Watcher error: {0}")]
    WatcherError(String),

    // Channel errors
    #[error("Channel closed unexpectedly")]
    ChannelClosed,

    #[error("Event queue closed unexpectedly")]
    EventQueueClosed,

    // State errors
    #[error("Swap failed: no valid config to swap to")]
    SwapFailed,

    // Pattern errors
    #[error("Invalid glob pattern: {0}")]
    InvalidGlobPattern(String),

    // Debouncer errors
    #[error("Debounce error: {0}")]
    DebounceError(String),
}
```

### 7. Related Error Types (Debouncer)

```rust
enum DebouncerError {
    #[error("Invalid debounce duration configured: duration cannot be zero")]
    InvalidDebounceDuration,

    #[error("Watcher channel closed unexpectedly")]
    WatcherChannelClosed,

    #[error("Debouncer encountered an internal error")]
    DebouncerInternal,

    #[error("No tokio runtime available; debouncer requires an active async runtime")]
    NoRuntime,
}
```

### 8. Operation Semantics

#### 8.1 try_update

1. Validate the new config using the configured validator
2. If validation fails, return `ValidationFailed` error
3. Atomically store the config in `pending` slot
4. Return `Ok(())`

#### 8.2 commit

1. Atomically read and clear the `pending` slot
2. If `pending` was `None`, return `SwapFailed` error
3. Atomically update `current` with the pending value
4. Return the old config value for potential rollback

#### 8.3 rollback

1. Atomically clear the `pending` slot
2. `current` remains unchanged

#### 8.4 reload_from_file

1. Read file contents from `path`
2. Parse JSON into config type
3. Validate the parsed config
4. If validation fails, return `ValidationFailed` and do NOT update current
5. Atomically update `current` with new config
6. Return the old config value

### 9. Watcher Semantics

#### 9.1 FileWatcher

- Creates a native file system watcher using `notify` crate
- `watch()` and `unwatch()` are idempotent operations
- Recursive mode watches all subdirectories

#### 9.2 FilteredFileWatcher

- Applies glob pattern filtering before event emission
- Empty patterns list matches all files
- Uses `glob::Pattern` for matching

#### 9.3 DebouncedFileWatcher

- Debounce window coalesces rapid Modify events for the same path
- Delete events cancel any pending debounce for that path
- Returns one event per "quiet period" per file

### 10. Constraints

- Config type `T` must be `Clone + Send + Sync + 'static` for thread-safe sharing
- File watchers are blocking in the sense they use `notify` blocking API internally
- Debouncer requires an active Tokio runtime
- Debounce duration must be > 0
- Channel capacity is bounded; senders must handle `ChannelClosed` error

### 11. Relevant Files

- `crates/vo-core/src/config_hot_reload.rs` (main implementation)
- `crates/vo-core/src/debounce.rs` (debouncer and file events)
- `crates/vo-core/src/lib.rs` (module exports)

### 12. Acceptance Criteria

- HotReloadConfig type covers current/pending state with atomic commit/rollback
- ConfigValidator trait enables arbitrary validation logic injection
- FileWatcher, FilteredFileWatcher, DebouncedFileWatcher provide layered file watching
- All invariants (CHR-001 through CHR-012) are formally stated
- Error taxonomy is exhaustive for file, parse, validation, watcher, and channel failures
- Debouncer correctly coalesces rapid events and cancels on delete
- The contract is self-contained and references only existing implementation files