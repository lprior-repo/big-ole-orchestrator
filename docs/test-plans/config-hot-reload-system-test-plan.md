# Test Plan: Config Hot-Reload System

## Summary

- **Bead**: ve-535y (Test Plan: Config hot-reload system)
- **Contract**: ve-crwr (Contract: Config hot-reload system)
- **Implementation**: `crates/vo-core/src/config_hot_reload.rs`, `crates/vo-core/src/debounce.rs`
- **Behaviors identified**: 78
- **Trophy allocation**: 55 unit / 18 integration / 3 e2e / 15 proptest / 5 kani (Total 96 tests)
- **Proptest invariants**: 12
- **Kani harnesses**: 3
- **Target Mutation Kill Rate**: ≥90%

---

## 1. Behavior Inventory

### 1.1 HotReloadConfig\<T\> Construction

1. `new` returns `Err(ConfigFileNotFound)` when config file does not exist at path
2. `new` returns `Ok(HotReloadConfig)` when config file exists at path
3. `new` initializes `current` with provided initial value
4. `new` initializes `pending` as `None`
5. `new` stores path and validator reference

### 1.2 HotReloadConfig\<T\> State Access

6. `current()` returns a clone of the current config (CHR-008)
7. `current()` never returns a reference to internal state (preserves isolation)
8. `path()` returns a reference to the stored path
9. `path()` returns the exact path provided at construction

### 1.3 HotReloadConfig\<T\> try_update

10. `try_update` with valid config returns `Ok(())` and stores in pending (CHR-002, CHR-003)
11. `try_update` with invalid config returns `Err(ValidationFailed)` and does NOT store in pending (CHR-002)
12. `try_update` calling twice overwrites previous pending (CHR-003)
13. `try_update` does not modify `current` state
14. `try_update` validates before staging

### 1.4 HotReloadConfig\<T\> commit

15. `commit` when pending exists returns `Ok(old_config)` and promotes pending to current (CHR-004)
16. `commit` when pending is `None` returns `Err(SwapFailed)` (CHR-004)
17. `commit` clears pending after successful promotion (CHR-004)
18. `commit` returns the previous current config for potential rollback
19. `commit` is the only operation that promotes pending to current (CHR-004)

### 1.5 HotReloadConfig\<T\> rollback

20. `rollback` clears pending without modifying current (CHR-005)
21. `rollback` when no pending exists is a no-op
22. `rollback` after `try_update` followed by `rollback` causes `commit` to return `SwapFailed`

### 1.6 HotReloadConfig\<T\> reload_from_file

23. `reload_from_file` reads from stored path
24. `reload_from_file` parses JSON into config type
25. `reload_from_file` validates parsed config before updating (CHR-007)
26. `reload_from_file` returns `Err(ParseError)` for malformed JSON
27. `reload_from_file` returns `Err(ReadError)` when file cannot be read
28. `reload_from_file` returns `Err(ValidationFailed)` when validation fails and does NOT update current (CHR-007)
29. `reload_from_file` returns `Ok(old_config)` with previous config
30. `reload_from_file` updates current directly bypassing pending state (CHR-006)

### 1.7 Invariant: CHR-001 (current always holds valid config)

31. `new` requires valid initial config
32. `commit` never sets invalid config as current
33. `reload_from_file` never sets invalid config as current
34. After any operation, `current()` always returns validator-approved config

### 1.8 Invariant: CHR-002 (pending is None or Some(valid_config))

35. `try_update` rejects invalid config before staging
36. If `try_update` returns error, pending remains unchanged
37. `rollback` clears pending regardless of pending validity

### 1.9 Invariant: CHR-003 (only one pending at a time)

38. Calling `try_update` twice stores only the second config in pending
39. First pending config is discarded without ceremony

### 1.10 Invariant: CHR-004 (commit is only promotion path)

40. `current` is never modified except via `commit`
41. `current` is never modified via `try_update`
42. `current` is never modified via `rollback`
43. `reload_from_file` modifies current bypassing pending (CHR-006 exception)

### 1.11 Invariant: CHR-005 (rollback clears pending without modifying current)

44. `rollback` leaves current unchanged
45. `rollback` returns nothing (void operation)
46. After `rollback`, `commit` returns `SwapFailed`

### 1.12 Invariant: CHR-006 (reload_from_file bypasses pending)

47. `reload_from_file` updates current without going through pending
48. `reload_from_file` does not affect pending state
49. `reload_from_file` does not clear any existing pending

### 1.13 Invariant: CHR-007 (validation required before any current modification)

50. `commit` validates pending config before promoting (implicit via try_update)
51. `reload_from_file` validates new config before updating current
52. If validation fails, current remains unchanged

### 1.14 Invariant: CHR-008 (current() returns clone)

53. Modifying returned clone does not affect internal current
54. Multiple calls to `current()` return independent clones

### 1.15 FileWatcher Construction

55. `new` creates FileWatcher with non-recursive mode by default
56. `with_recursive` creates recursive watcher when flag is true
57. `with_recursive` creates non-recursive watcher when flag is false
58. `new` returns `Err(WatcherError)` when path does not exist

### 1.16 FileWatcher State

59. `path()` returns the watched path
60. `is_recursive()` returns correct recursive flag
61. `unwatch` returns `Ok(())` on successful unwatch
62. `unwatch` returns `Err(WatcherError)` when unwatch fails

### 1.17 Invariant: CHR-009 (watch/unwatch atomic)

63. Multiple `unwatch` calls do not cause panic
64. `unwatch` on non-watched path returns error but has no other effect

### 1.18 FilteredFileWatcher Construction

65. `new` creates FilteredFileWatcher with given WatcherConfig
66. `new` returns `Err(WatcherError)` when path does not exist

### 1.19 FilteredFileWatcher Pattern Matching

67. `matches_pattern` returns `true` for path matching glob pattern
68. `matches_pattern` returns `false` for path not matching glob pattern
69. `matches_pattern` returns `true` when patterns list is empty (CHR-010)
70. `matches_pattern` correctly matches multiple patterns (OR semantics)
71. `matches_pattern` handles wildcard `*.json` pattern
72. `matches_pattern` handles multiple wildcards `**/*.json` pattern
73. `matches_pattern` returns `false` for invalid glob pattern

### 1.20 FilteredFileWatcher State

74. `path()` returns the watched path
75. `unwatch` returns `Ok(())` on successful unwatch

### 1.21 DebouncedFileWatcher Construction

76. `new` creates DebouncedFileWatcher with EventChannel
77. `new` returns `Err(DebounceError)` when debouncer creation fails
78. `new` returns `Err(WatcherError)` when watcher creation fails

### 1.22 DebouncedFileWatcher State

79. `path()` returns the watched path
80. `is_recursive()` returns correct recursive flag from WatcherConfig
81. `unwatch` returns `Ok(())` on successful unwatch

### 1.23 EventChannel Construction

82. `new(capacity)` creates channel with specified capacity
83. `sender()` returns a clone of the sender handle

### 1.24 EventChannel Operations

84. `send` returns `Ok(())` when event sent successfully
85. `send` returns `Err(EventQueueClosed)` when receiver dropped (CHR-012)
86. `send` is async and bounded

### 1.25 WatcherConfig Defaults

87. `Default` sets `recursive: true`
88. `Default` sets `debounce_duration: Some(300ms)`
89. `Default` sets `patterns: ["*"]`

---

## 2. Trophy Allocation

| Layer | Count | Description |
|-------|-------|-------------|
| Unit Tests | 55 | Core logic, invariants, error variants |
| Integration Tests | 18 | Cross-component, file system interactions |
| E2E Tests | 3 | Full hot-reload workflow |
| Proptest | 15 | Property-based testing for parsers and invariants |
| Kani | 5 | Formal verification of critical invariants |
| **Total** | **96** | |

### Unit Tests (55)

- HotReloadConfig: 25 tests (construction, state access, operations, invariants)
- FileWatcher: 8 tests (construction, state, unwatch)
- FilteredFileWatcher: 12 tests (construction, pattern matching, state)
- DebouncedFileWatcher: 5 tests (construction, state)
- EventChannel: 5 tests (construction, send, capacity)

### Integration Tests (18)

- HotReloadConfig + FileWatcher integration: 5 tests
- FilteredFileWatcher + DebouncedFileWatcher integration: 5 tests
- ConfigValidator integration: 8 tests

### E2E Tests (3)

- Full hot-reload cycle with actual file watching
- Rollback workflow end-to-end
- Concurrent update and commit scenarios

### Proptest (15)

- Debouncer event deduplication
- Debouncer timer precision
- Multiple concurrent file events
- Invalid glob pattern handling

### Kani (5)

- CHR-001: current always holds valid config
- CHR-002: pending is None or Some(valid_config)
- CHR-003: only one pending at a time
- CHR-004: commit is only promotion path
- CHR-007: validation required before current modification

---

## 3. BDD Scenarios

### HotReloadConfig Lifecycle

#### Scenario: Successful Config Update via Pending
```
Given: A HotReloadConfig with valid initial config at path P
  And: The validator accepts all configs
When: I call try_update with a new valid config
Then: pending contains the new config
  And: current still returns the initial config
  And: commit returns Ok(old_config)
  And: current returns the new config
```

#### Scenario: Config Update Rejected by Validator
```
Given: A HotReloadConfig with validator that rejects "invalid"
When: I call try_update with "invalid"
Then: try_update returns Err(ValidationFailed)
  And: pending remains None
  And: current returns the initial config
```

#### Scenario: Rollback Discards Pending
```
Given: A HotReloadConfig with pending config
When: I call rollback
Then: pending becomes None
  And: current remains unchanged
  And: subsequent commit returns Err(SwapFailed)
```

#### Scenario: reload_from_file Updates Current Directly
```
Given: A HotReloadConfig with current = {"version": 1}
  And: pending is None
When: I modify the file at path to contain {"version": 2}
  And: I call reload_from_file
Then: current returns {"version": 2}
  And: pending remains None
```

#### Scenario: reload_from_file Validates Before Update
```
Given: A HotReloadConfig with validator that rejects "invalid"
  And: current = {"key": "value"}
When: I write "invalid" to the file at path
  And: I call reload_from_file
Then: reload_from_file returns Err(ValidationFailed)
  And: current returns {"key": "value"} unchanged
```

### File Watcher Behavior

#### Scenario: Watch and Unwatch
```
Given: A valid file at path P
When: I create a FileWatcher for P
Then: watch is active
  And: is_recursive returns the configured value
When: I call unwatch
Then: unwatch returns Ok(())
```

#### Scenario: Unwatch Non-existent Path
```
Given: A FileWatcher for path P
When: P does not exist
Then: unwatch returns Err(WatcherError)
```

### Pattern Matching

#### Scenario: FilteredFileWatcher Matches Glob
```
Given: A FilteredFileWatcher with patterns ["*.json", "*.toml"]
When: I call matches_pattern with "/path/file.json"
Then: returns true
When: I call matches_pattern with "/path/file.txt"
Then: returns false
```

#### Scenario: Empty Patterns Match All
```
Given: A FilteredFileWatcher with patterns []
When: I call matches_pattern with any path
Then: returns true
```

### Debouncer Behavior

#### Scenario: Rapid Events Coalesced
```
Given: A Debouncer with 100ms duration
When: I send Modify("/path/file") three times within 50ms
Then: Only one event is yielded after debounce duration
  And: The yielded path is "/path/file"
```

#### Scenario: Delete Cancels Pending
```
Given: A Debouncer with 100ms duration
  And: I have sent Modify("/path/file") and 50ms has elapsed
When: I send Delete("/path/file")
Then: After 100ms total, no event is yielded
```

---

## 4. Proptest Invariants

### Debouncer Event Deduplication
```
Property: Multiple Modify events for the same file within debounce window
         produce exactly one yield
Strategy: vec![FileEvent] with timing constraints
Anti-invariant: More than one event yielded for same path
```

### Debouncer Timer Reset
```
Property: Sending Modify before debounce expires resets the timer
Strategy: Two Modify events with controlled timing
Anti-invariant: Two yields when should be one
```

### Debouncer Delete Cancellation
```
Property: Delete for a path cancels any pending debounce for that path
Strategy: Modify followed by Delete followed by time advance
Anti-invariant: Event still yielded after delete
```

### Pattern Matching Soundness
```
Property: FilteredFileWatcher.matches_pattern never panics
         for any valid Path input
Strategy: Arbitrary PathBuf
Anti-invariant: Panic on malformed path
```

### Multiple Concurrent Paths
```
Property: Events for different paths are all eventually yielded
Strategy: Multiple distinct PathBufs with interleaved timing
Anti-invariant: Events for some paths are lost
```

### HotReloadConfig State Isolation
```
Property: current() returns independent clones
Strategy: Call current() multiple times, modify returned values
Anti-invariant: Modifying clone affects internal state
```

### try_update Validation
```
Property: try_update only accepts configs passing validator
Strategy: Arbitrary JSON value with varying validator
Anti-invariant: Invalid config staged in pending
```

### commit Promotion
```
Property: commit only succeeds when pending is Some
Strategy: Sequence of try_update, commit, rollback operations
Anti-invariant: commit succeeds with no pending
```

### reload_from_file Atomicity
```
Property: reload_from_file either fully updates current or leaves unchanged
Strategy: Valid/invalid file content with varying validators
Anti-invariant: Partial update or corruption on validation failure
```

### Path clone independence
```
Property: Modifying PathBuf passed to new does not affect HotReloadConfig
Strategy: Create config, modify original PathBuf
Anti-invariant: Internal path state affected by external mutation
```

### Channel Capacity
```
Property: EventChannel.send respects capacity bounds
Strategy: Send more events than capacity
Anti-invariant: send blocks indefinitely or panics
```

### Error Propagation
```
Property: All error variants are correctly propagated with context
Strategy: Various failure scenarios for each error type
Anti-invariant: Error type confusion or context loss
```

---

## 5. Kani Harnesses

### Harness 1: CHR-001 (current always valid)
```rust
#[kani::proof]
fn verify_current_always_valid() {
    // Verify that after any sequence of operations,
    // current always holds a validator-approved config
}
```

### Harness 2: CHR-002 (pending validity)
```rust
#[kani::proof]
fn verify_pending_validity() {
    // Verify pending is None OR contains valid config
    // Never contains invalid config
}
```

### Harness 3: CHR-004 (commit atomicity)
```rust
#[kani::proof]
fn verify_commit_atomicity() {
    // Verify commit either fully promotes pending to current
    // or returns error with no partial effects
}
```

### Harness 4: CHR-007 (validation gate)
```rust
#[kani::proof]
fn verify_validation_gate() {
    // Verify no config modification happens without validation passing
}
```

### Harness 5: State Isolation
```rust
#[kani::proof]
fn verify_current_clone_isolation() {
    // Verify current() returning clone does not expose internal state
}
```

---

## 6. Error Taxonomy Coverage

| Error Variant | Construction Test | Propagation Test | Display Test |
|---------------|------------------|------------------|--------------|
| ConfigFileNotFound | new returns error when file missing | N/A | Display format |
| ReadError | N/A | reload_from_file on unreadable | Display format |
| ParseError | N/A | reload_from_file with bad JSON | Display format |
| ValidationFailed | try_update with invalid | Both try_update and reload | Display format |
| WatcherError | FileWatcher bad path | unwatch failure | Display format |
| ChannelClosed | N/A | EventChannel send after drop | N/A |
| SwapFailed | N/A | commit with no pending | Display format |
| InvalidGlobPattern | matches_pattern bad glob | N/A | N/A |
| DebounceError | Debouncer zero duration | N/A | Display format |
| EventQueueClosed | N/A | EventChannel send failure | Display format |

---

## 7. Mutation Checkpoints

Critical mutations that must be detected:

| Mutation | Test That Catches It |
|----------|---------------------|
| Remove validation check in try_update | try_update_fails_with_invalid_config |
| Remove pending.take() in commit | commit_returns_error_when_no_pending |
| Skip validation in reload_from_file | reload_validates_before_update |
| Modify is_recursive return value | watcher_recursive_flag_preserved |
| Change debounce duration check | debouncer_rejects_zero_duration |
| Remove pending.remove() on Delete | debouncer_delete_cancels_pending |
| Skip pattern validation | filtered_watcher_invalid_pattern_handled |
| Modify channel capacity calculation | event_channel_capacity_respected |

**Threshold**: 90% mutation kill rate minimum
**Coverage**: 90% line coverage minimum

---

## 8. Combinatorial Coverage Matrix

### HotReloadConfig Operations

| Operation | Valid Input | Invalid Input | No Pending | Has Pending |
|-----------|-------------|---------------|------------|-------------|
| try_update | Ok(()) | ValidationFailed | N/A | Overwrites |
| commit | Ok(old) | N/A | SwapFailed | Ok(old) |
| rollback | () | N/A | () | () |
| reload_from_file | Ok(old) | ValidationFailed/ParseError | Ok(old) | Ok(old) |

### FileWatcher

| Scenario | Path Exists | Path Missing | Recursive | Non-recursive |
|----------|-------------|--------------|-----------|---------------|
| new | Ok | WatcherError | N/A | N/A |
| with_recursive | Ok | WatcherError | Ok | Ok |
| unwatch | Ok | WatcherError | Ok | Ok |

### Pattern Matching

| Pattern | file.json | file.txt | dir/file.json | **/*.json |
|---------|-----------|----------|---------------|-----------|
| *.json | true | false | false | false |
| **/*.json | true | false | true | true |
| [] | true | true | true | true |
| *.json,*.toml | true (json) | false | false | true (json) |

### Debouncer Timing

| Scenario | 50ms events | 100ms events | 150ms events | Delete before expiry |
|----------|--------------|--------------|--------------|---------------------|
| 100ms debounce | 1 yield | 1 yield | 1 yield | 0 yield |
| Continuous writes | 1 yield | 1 yield | 1 yield | 0 yield |

---

## 9. Test Implementation Locations

### Unit Tests (in `config_hot_reload.rs`)
- `mod tests` - Basic unit tests for HotReloadConfig
- `mod tests` in `debounce.rs` - Basic unit tests for Debouncer

### Integration Tests
- `crates/vo-core/tests/config_hot_reload_integration.rs` - Cross-component tests

### Proptest
- `crates/vo-core/tests/config_hot_reload_proptest.rs` - Property-based tests

### Kani
- `crates/vo-core/src/config_hot_reload.rs` - `#[cfg(kani)] mod verification`

### E2E
- `crates/vo-core/tests/config_hot_reload_e2e.rs` - Full workflow tests

---

## 10. Execution Plan

1. **Phase 1**: Implement unit tests for all 78 behaviors (55 tests)
2. **Phase 2**: Implement integration tests (18 tests)
3. **Phase 3**: Implement proptest invariants (15 tests)
4. **Phase 4**: Implement Kani harnesses (5 tests)
5. **Phase 5**: Implement E2E tests (3 tests)
6. **Phase 6**: Run mutation testing, achieve ≥90% kill rate
7. **Phase 7**: Run coverage, achieve ≥90% line coverage

---

## 11. Acceptance Criteria

- [ ] All 78 behaviors have corresponding test cases
- [ ] All 11 error variants are tested for construction, propagation, and display
- [ ] All 12 invariants (CHR-001 through CHR-012) have explicit verification
- [ ] Proptest invariants never panic and detect anti-invariants
- [ ] Kani harnesses verify critical concurrent safety properties
- [ ] Mutation kill rate ≥90%
- [ ] Line coverage ≥90%
- [ ] All tests pass on main branch