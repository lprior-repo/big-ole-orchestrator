# Test Plan: AI-Facing Redacted History (ADR-008/ADR-025)

## Summary

- **Bead**: ve-8fs9
- **Behaviors identified**: 29
- **Trophy allocation**: 18 unit / 14 integration / 4 e2e / 3 static
- **Proptest invariants**: 8
- **Fuzz targets**: 4
- **Kani harnesses**: 2
- **Mutation kill rate target**: ≥90%

---

## 1. Behavior Inventory

### 1.1 Redaction Completeness (No PII Leaks)

1. `[RedactionPolicy] applies Remove rule and sets field to Null when path matches`
2. `[RedactionPolicy] applies ReplaceWith rule and substitutes fixed placeholder when path matches`
3. `[RedactionPolicy] applies ReplaceWithType rule and substitutes Rust type name when path matches`
4. `[RedactionPolicy] applies Hash rule and produces deterministic SHA-256 hash when path matches`
5. `[apply_redaction] traverses nested objects recursively and applies rules at correct depth`
6. `[apply_redaction] traverses arrays recursively and applies rules to each array element`
7. `[apply_redaction] tracks redacted field paths and returns them in order applied`
8. `[RedactionKind::Hash] produces identical hash for identical input values`
9. `[RedactionKind::Hash] produces different hash for different input values`
10. `[apply_redaction] does not leak PII through field count differences after redaction`
11. `[apply_redaction] does not leak PII through object key ordering after redaction`
12. `[apply_redaction] handles empty objects and arrays without panicking`
13. `[apply_redaction] handles deeply nested structures (depth > 10) correctly`
14. `[apply_redaction] applies multiple rules to different paths simultaneously`

### 1.2 Canonical Privileged History Access Control

15. `[vault] Read permission is required to access canonical history`
16. `[vault] AccessDenied error is returned when credential lacks required permission`
17. `[CommandHistory] stores entries with envelope containing command_id, correlation_id, causation_id`
18. `[CommandHistory] undo requires entry to exist in undo_stack with Committed status`
19. `[CommandHistory] redo transitions entry from Undone back to Redone status`
20. `[CommandHistory] save_undo_point clears redo_stack (INV-009)`

### 1.3 Query Interface for AI Consumers

21. `[get_history] produces HistoryOutput with can_undo, can_redo, stack depths, and entries`
22. `[HistoryEntryOutput] contains command_id, kind, status as strings for AI parsing`
23. `[load_history] returns empty CommandHistory when history file does not exist`
24. `[save_history] creates parent directories when they do not exist`
25. `[vo-cli history --json] returns redacted operator projection without PII`

### 1.4 Retention and GC of Redacted Views

26. `[purge_instance] deletes events, snapshots, and index entries for terminal instances`
27. `[purge_instance] returns error when instance status is non-terminal (Pending, Running, Paused)`
28. `[purge_instance] returns InvalidInstanceId error when instance ID string is empty`
29. `[purge_instance] returns InstanceRunning error when instance ID not found in index`
30. `[purge_instance] returns count of purged events`
31. `[is_terminal] returns true for Completed, Failed, Cancelled statuses`
32. `[is_terminal] returns false for Pending, Running, Paused statuses`

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit (Calc)** | 18 | Pure redaction logic (apply_redaction, RedactionKind variants), hash determinism, WorkflowSnapshot checksum, stack invariants, status transitions, CommandHistory operations |
| **Integration** | 14 | apply_redaction with real serde_json, purge_instance with real fjall keyspace, history load/save with real filesystem, concurrent CommandHistory operations |
| **E2E** | 4 | CLI `vo-cli history --json`, CLI `vo-cli history --canonical`, CLI `vo purge --instance`, CLI `vo purge` on non-terminal |
| **Static** | 3 | clippy::unwrap_used, clippy::pedantic, cargo-deny audit |

**Deviation from standard ratio**: Higher unit allocation justified because redaction logic is pure functions with exhaustive combinatorial inputs (RedactionKind × field types × nesting depth). Integration tests cover storage layer with real fjall and filesystem I/O.

---

## 3. BDD Scenarios

### 3.1 Redaction Completeness

#### Behavior: RedactionPolicy applies Remove rule and sets field to Null when path matches

**Given**: JSON object `{"user": {"name": "Alice", "ssn": "123-45-6789"}}`
**When**: `apply_redaction` is called with rule `["user", "ssn"]` → `RedactionKind::Remove`
**Then**: Result is `{"user": {"name": "Alice", "ssn": null}}`
**And**: `redacted_fields` contains `[["user", "ssn"]]`

```rust
fn redaction_removes_field_and_returns_null_when_path_matches()
```

#### Behavior: RedactionPolicy applies ReplaceWith rule and substitutes fixed placeholder when path matches

**Given**: JSON object `{"password": "secret123"}`
**When**: `apply_redaction` is called with rule `["password"]` → `RedactionKind::ReplaceWith("[REDACTED]")`
**Then**: Result is `{"password": "[REDACTED]"}`

```rust
fn redaction_replaces_field_with_placeholder_when_path_matches()
```

#### Behavior: RedactionPolicy applies ReplaceWithType rule and substitutes Rust type name when path matches

**Given**: JSON object `{"value": 42}`
**When**: `apply_redaction` is called with rule `["value"]` → `RedactionKind::ReplaceWithType`
**Then**: Result's `value` field is a string containing `"i64"` or `"u64"` (integer type name)

```rust
fn redaction_replaces_field_with_type_name_when_path_matches()
```

#### Behavior: RedactionPolicy applies Hash rule and produces deterministic SHA-256 hash when path matches

**Given**: JSON object `{"email": "user@example.com"}`
**When**: `apply_redaction` is called with rule `["email"]` → `RedactionKind::Hash`
**Then**: Result's `email` field starts with `"HASH"` and is deterministic (same input → same output)

```rust
fn redaction_hashes_field_with_deterministic_output_when_path_matches()
```

#### Behavior: apply_redaction traverses nested objects recursively and applies rules at correct depth

**Given**: JSON object `{"outer": {"inner": {"secret": "value"}}}`
**When**: `apply_redaction` is called with rule `["outer", "inner", "secret"]` → `RedactionKind::Remove`
**Then**: Result is `{"outer": {"inner": {}}}`

```rust
fn redaction_applies_rules_at_correct_nested_depth()
```

#### Behavior: apply_redaction traverses arrays recursively and applies rules to each array element

**Given**: JSON object `{"users": [{"name": "Alice", "ssn": "111"}, {"name": "Bob", "ssn": "222"}]}`
**When**: `apply_redaction` is called with rule `["users", "ssn"]` → `RedactionKind::Remove`
**Then**: Both array elements have `ssn: null`
**And**: `redacted_fields` has 2 entries

```rust
fn redaction_applies_rules_to_all_array_elements()
```

#### Behavior: apply_redaction does not leak PII through field count differences after redaction

**Given**: Two objects with same top-level keys but different numbers of sensitive fields: `{"a": "x", "b": "y"}` and `{"a": "x"}`
**When**: Redaction removes `"b"` field from first object only
**Then**: Both results have same structure; observer cannot infer that `"b"` existed

```rust
fn redaction_does_not_leak_via_field_count()
```

#### Behavior: apply_redaction does not leak PII through object key ordering after redaction

**Given**: JSON objects maintain insertion order (serde_json::Map)
**When**: A field is redacted from an object with multiple keys
**Then**: The resulting object's keys are in deterministic order (alphabetical) to prevent ordering leaks

```rust
fn redaction_normalizes_key_ordering()
```

#### Behavior: apply_redaction handles deeply nested structures (depth > 10) correctly

**Given**: Object nested 15 levels with sensitive field at depth 12
**When**: Redaction rule targets the deep field
**Then**: Rule is applied correctly at depth 12 without stack overflow

```rust
fn redaction_handles_deeply_nested_structures()
```

#### Behavior: apply_redaction applies multiple rules to different paths simultaneously

**Given**: Object `{"user": {"email": "a@b.com", "password": "secret"}, "admin": {"code": "123"}}`
**When**: Three rules target `["user", "email"]`, `["user", "password"]`, `["admin", "code"]` with Hash/Remove
**Then**: All three fields are redacted correctly and independently

```rust
fn redaction_applies_multiple_rules_simultaneously()
```

### 3.2 Canonical Privileged History Access Control

#### Behavior: vault Read permission is required to access canonical history

**Given**: A vault with credential that has `Write` permission but not `Read`
**When**: Attempting to access canonical history endpoint
**Then**: `CredentialError::AccessDenied` is returned with principal and required permission

```rust
fn vault_returns_access_denied_when_credential_lacks_read_permission_for_canonical_history()
```

#### Behavior: CommandHistory stores entries with envelope containing command_id, correlation_id, causation_id

**Given**: A new `HistoryEntry` created via `HistoryEntry::new`
**When**: Entry is inspected
**Then**: `envelope.metadata.command_id`, `correlation_id`, and `causation_id` are all non-empty strings

```rust
fn history_entry_contains_all_three_ids_in_envelope()
```

#### Behavior: CommandHistory undo requires entry to exist in undo_stack with Committed status

**Given**: A `CommandHistory` with one committed entry
**When**: `undo()` is called
**Then**: Entry's status transitions to `Undone`
**And**: CommandId moves from undo_stack to redo_stack

```rust
fn history_undo_transitions_entry_to_undone_status()
```

#### Behavior: CommandHistory redo transitions entry from Undone back to Redone status

**Given**: A `CommandHistory` with one entry that has `HistoryEntryStatus::Undone`
**When**: `redo()` is called
**Then**: Entry's status transitions to `Redone`
**And**: CommandId moves from redo_stack back to undo_stack

```rust
fn history_redo_transitions_entry_to_redone_status()
```

#### Behavior: CommandHistory save_undo_point clears redo_stack (INV-009)

**Given**: A `CommandHistory` with one entry on the redo_stack
**When**: `save_undo_point()` is called for a new command
**Then**: `redo_stack` is empty after the call

```rust
fn history_save_undo_point_clears_redo_stack()
```

### 3.3 Query Interface for AI Consumers

#### Behavior: get_history produces HistoryOutput with can_undo, can_redo, stack depths, and entries

**Given**: A `CommandHistory` with one committed entry
**When**: `get_history()` is called
**Then**: Result has `can_undo: true`, `can_redo: false`, `undo_stack_depth: 1`, `redo_stack_depth: 0`
**And**: `entries` has exactly one `HistoryEntryOutput`

```rust
fn get_history_produces_complete_output_structure()
```

#### Behavior: HistoryEntryOutput contains command_id, kind, status as strings for AI parsing

**Given**: A `HistoryEntry` with `CommandKind::NodeCreate` and `HistoryEntryStatus::Committed`
**When**: `get_history()` produces `HistoryEntryOutput`
**Then**: `kind` is `"NodeCreate"` and `status` is `"Committed"` (not enum variants)

```rust
fn history_entry_output_contains_string_fields_for_ai_consumption()
```

#### Behavior: load_history returns empty CommandHistory when history file does not exist

**Given**: A path to a non-existent file
**When**: `load_history()` is called
**Then**: Returns `Ok(CommandHistory::new())` with empty stacks

```rust
fn load_history_returns_empty_history_when_file_absent()
```

#### Behavior: vo-cli history --json returns redacted operator projection without PII

**Given**: A workflow with `{"user": {"name": "Alice", "ssn": "123-45-6789"}}`
**When**: `vo-cli history <instance> --json` is executed
**Then**: Output JSON contains `{"user": {"name": "Alice", "ssn": null}}` or redacted equivalent
**And**: No PII (SSN, full email, etc.) appears in plain text in output

```rust
fn cli_history_json_returns_redacted_projection()
```

### 3.4 Retention and GC of Redacted Views

#### Behavior: purge_instance deletes events, snapshots, and index entries for terminal instances

**Given**: A fjall keyspace with events, snapshots, and instance index for a `Completed` instance
**When**: `purge_instance(keyspace, instance_id)` is called
**Then**: All three partitions are empty for that instance_id prefix
**And**: Returns `Ok(event_count)`

```rust
fn purge_instance_deletes_all_three_partition_types_for_terminal_instance()
```

#### Behavior: purge_instance returns error when instance status is non-terminal

**Given**: A keyspace with instance in `Running` status
**When**: `purge_instance(keyspace, instance_id)` is called
**Then**: Returns `Err(StorageError::InstanceRunning)`

```rust
fn purge_instance_rejects_non_terminal_instance_status()
```

#### Behavior: purge_instance returns InvalidInstanceId error when instance ID string is empty

**Given**: An empty string for instance ID
**When**: `purge_instance(keyspace, "")` is called
**Then**: Returns `Err(StorageError::InvalidInstanceId(...))` with `ParseError::Empty`

```rust
fn purge_instance_returns_invalid_instance_id_when_input_empty()
```

#### Behavior: purge_instance returns InstanceRunning error when instance ID not found in index

**Given**: A valid-format instance ID that does not exist in the index
**When**: `purge_instance(keyspace, instance_id)` is called
**Then**: Returns `Err(StorageError::InstanceRunning)`

```rust
fn purge_instance_returns_instance_running_when_instance_not_found()
```

#### Behavior: purge_instance returns count of purged events

**Given**: A fjall keyspace with exactly 7 events for a terminal instance
**When**: `purge_instance(keyspace, instance_id)` is called
**Then**: Returns `Ok(7)`

```rust
fn purge_instance_returns_accurate_event_count()
```

#### Behavior: is_terminal returns true for Completed, Failed, Cancelled statuses

**Given**: Three instances with `InstanceStatus::Completed`, `Failed`, `Cancelled` respectively
**When**: `is_terminal(status)` is called for each
**Then**: All return `true`

```rust
fn is_terminal_returns_true_for_all_terminal_statuses()
```

#### Behavior: is_terminal returns false for Pending, Running, Paused statuses

**Given**: Three instances with `InstanceStatus::Pending`, `Running`, `Paused` respectively
**When**: `is_terminal(status)` is called for each
**Then**: All return `false`

```rust
fn is_terminal_returns_false_for_all_non_terminal_statuses()
```

---

## 4. Proptest Invariants

### Proptest: apply_redaction roundtrip

**Invariant**: Serializing a `RedactionPolicy` to JSON and deserializing it back produces an equal policy
**Strategy**: `any::<RedactionPolicy>()` with constrained `workflow_type` to alphanumeric
**Anti-invariant**: `field_path` with empty segments, invalid UTF-8

### Proptest: OperatorProjection roundtrip

**Invariant**: `OperatorProjection` serializes and deserializes without data loss
**Strategy**: `any::<OperatorProjection>()` with `workflow_id` matching `[a-z0-9]{10}`
**Anti-invariant**: `projection_json` containing extremely deep nesting (>100 levels)

### Proptest: RedactionKind::Hash determinism

**Invariant**: `hash("same input")` called twice returns identical string
**Strategy**: `any::<serde_json::Value>()` but excluding `Value::Null` (hash of Null is valid but edge case)
**Anti-invariant**: `Value::Function` or `Value::Object` with >1000 keys (performance boundary)

### Proptest: CommandHistory stack balance

**Invariant**: `entries.len() <= MAX_HISTORY_DEPTH` always holds after any operation
**Strategy**: Random sequence of `save_undo_point`, `undo`, `redo`, `apply_command`
**Anti-invariant**: Operation that would cause `entries.len() > MAX_HISTORY_DEPTH`

### Proptest: WorkflowSnapshot checksum

**Invariant**: `compute_checksum(nodes, edges)` is deterministic for identical graph structure
**Strategy**: Random `DagNode` and `Edge` configurations with ≤50 nodes
**Anti-invariant**: Nodes with identical `node_name` (not allowed by `NodeName::parse`)

### Proptest: purge_instance returns accurate event count

**Invariant**: Returned `event_count` equals actual number of events deleted from events partition
**Strategy**: Randomly populate events partition (0–100 events) for a terminal instance
**Anti-invariant**: Non-terminal instance (purge should fail before counting)

### Proptest: is_terminal boolean is exact inverse

**Invariant**: `is_terminal(status) == !is_terminal(opposite_status)` where opposite_status is the complement
**Strategy**: `any::<InstanceStatus>()` and verify the boolean matches expected variant set
**Anti-invariant**: New `InstanceStatus` variants added without updating `is_terminal`

### Proptest: HistoryOutput consistency

**Invariant**: `HistoryOutput.undo_stack_depth == history.undo_stack().len()` always
**Strategy**: Random sequence of history operations followed by `get_history()`
**Anti-invariant**: Calling `get_history()` on a history modified concurrently (not thread-safe by design)

---

## 5. Fuzz Targets

### Fuzz Target: apply_redaction with arbitrary JSON and rules

**Input type**: `(serde_json::Value, Vec<RedactionRule>)`
**Risk**: Panic on malformed JSON, stack overflow on deeply nested JSON, logic error in path matching
**Corpus seeds**:
- `{"a": {"b": {"c": "secret"}}}`
- `{"items": [{"id": 1}, {"id": 2}]}`
- `{"mixed": [1, "string", {"nested": "value"}, null]}`
- `{}` (empty object)
- `[]` (empty array)

### Fuzz Target: RedactionKind::redact_value with all Value variants

**Input type**: `serde_json::Value`
**Risk**: Panic in `as_str().unwrap()` when value is not a string, type_name extraction failure
**Corpus seeds**: All 7 `serde_json::Value` variants: `Null`, `Bool`, `Number`, `String`, `Array`, `Object`, `Function`

### Fuzz Target: CommandHistory::apply_command with random snapshots

**Input type**: `(CommandKind, WorkflowSnapshot, WorkflowSnapshot)`
**Risk**: Checksum collision (unlikely), capacity overflow, stack imbalance
**Corpus seeds**:
- `CommandKind::NodeCreate` with single-node graph
- `CommandKind::ExtensionApply` with batch metadata
- `CommandKind::EdgeCreate` with edge connecting two nodes

### Fuzz Target: purge_instance with malformed instance IDs

**Input type**: `String` (instance ID)
**Risk**: Panic on invalid ULID parsing, incorrect error variant mapping, index out of bounds
**Corpus seeds**:
- Empty string
- Valid ULID format but not in index
- Invalid UTF-8 byte sequence
- ULID that is in index with terminal status
- ULID that is in index with non-terminal status

---

## 6. Kani Harnesses

### Kani Harness: apply_redaction path matching correctness

**Property**: For any `value`, `rules`, and `path`, if `path` is present in `value` and a rule exists for `path`, then the field at `path` in the result is the redacted value AND `path` appears in `redacted_fields`
**Bound**: JSON depth ≤5, number of rules ≤10, object keys ≤20
**Rationale**: Formal proof needed because redaction correctness is privacy-critical; proptest can only show presence of bugs, not absence

### Kani Harness: CommandHistory invariant: entries.len() ≤ capacity

**Property**: After any sequence of `save_undo_point`, `undo`, `redo`, `apply_command`, the invariant `entries.len() ≤ capacity` holds
**Bound**: ≤50 operations, initial capacity = 100
**Rationale**: Capacity enforcement prevents unbounded growth; if violated, memory usage could grow unboundedly

---

## 7. Mutation Checkpoints

### Critical mutations to survive:

| Function/Branch | Mutation | Must be caught by test |
|-----------------|----------|------------------------|
| `apply_redaction` line 164 | Change `!was_redacted \|\| new_val != Null` to `true` (skip Remove) | `redaction_removes_field_and_returns_null_when_path_matches` |
| `apply_redaction` line 152 | Change `.find()` to `.find(\|r\| false)` (no rule match) | `redaction_applies_rules_at_correct_nested_depth` |
| `RedactionKind::Hash` line 77 | Change `s.hash()` to `value.hash()` (loses string specificity) | `redaction_hashes_field_with_deterministic_output_when_path_matches` |
| `purge_instance` line 33 | Change `!is_terminal(...)` to `false` (allow non-terminal) | `purge_instance_rejects_non_terminal_instance_status` |
| `purge_instance` line 29 | Change `.ok_or(...)` to `.ok()` (return None on not found) | `purge_instance_returns_instance_running_when_instance_not_found` |
| `CommandHistory::save_undo_point` line 624 | Remove `self.redo_stack.clear()` (break INV-009) | `history_save_undo_point_clears_redo_stack` |
| `WorkflowSnapshot::compute_checksum` line 396 | Change `sort()` to `collect()` without sort (non-deterministic) | `workflow_snapshot_checksum_deterministic` |
| `CommandHistory::undo` line 668 | Change `status = Undone` to `Redone` (wrong status) | `history_undo_transitions_entry_to_undone_status` |
| `CommandHistory::redo` line 693 | Change `status = Redone` to `Undone` (wrong status) | `history_redo_transitions_entry_to_redone_status` |

**Threshold**: 90% mutation kill rate minimum.

---

## 8. Combinatorial Coverage Matrix

### Unit: RedactionPolicy + RedactionRule + RedactionKind

| Scenario | Policy rules | Rule field_path depth | Kind variant | Expected |
|----------|--------------|----------------------|--------------|----------|
| happy path - Remove | 1 | 1 | Remove | field → Null |
| happy path - ReplaceWith | 1 | 2 | ReplaceWith | field → placeholder |
| happy path - ReplaceWithType | 1 | 3 | ReplaceWithType | field → type_name |
| happy path - Hash | 1 | 1 | Hash | field → HASH... |
| multiple rules | 3 | 1, 2, 3 | Mix | all redacted |
| no matching rule | 1 | 5 | Any | unchanged |
| empty object | 1 | 1 | Remove | {} |
| empty array | 1 | 1 | Remove | [] |

### Unit: CommandHistory Stack Operations

| Scenario | Initial state | Operation | Expected |
|----------|--------------|-----------|----------|
| undo empty | new() | undo | Ok(false) |
| redo empty | new() | redo | Ok(false) |
| undo after save | 1 entry | undo | Ok(true), status=Undone |
| redo after undo | 1 undone | redo | Ok(true), status=Redone |
| redo cleared after new command | 1 undone, then new entry | redo | Ok(false) |
| capacity eviction | 101 saves | save_undo_point | oldest committed entry removed |

### Integration: purge_instance with fjall

| Scenario | Instance status | Events count | Snapshots count | Expected |
|----------|----------------|--------------|-----------------|----------|
| completed with events | Completed | 5 | 2 | Ok(5) |
| failed with zero events | Failed | 0 | 0 | Ok(0) |
| cancelled with snapshots | Cancelled | 3 | 1 | Ok(3) |
| running (reject) | Running | 0 | 0 | Err(InstanceRunning) |
| pending (reject) | Pending | 0 | 0 | Err(InstanceRunning) |
| paused (reject) | Paused | 0 | 0 | Err(InstanceRunning) |
| not found (reject) | n/a | 0 | 0 | Err(InstanceRunning) |
| empty ID (reject) | n/a | 0 | 0 | Err(InvalidInstanceId) |

### E2E: CLI Commands

| Scenario | Command | Expected output |
|----------|---------|-----------------|
| history --json | `vo-cli history wf-123 --json` | Valid JSON with redacted fields, no PII |
| history --canonical (with permission) | `vo-cli history wf-123 --canonical` | Full canonical data including encrypted payloads |
| history --canonical (without permission) | `vo-cli history wf-123 --canonical` | AccessDenied error |
| purge terminal | `vo purge --instance inst-123` | Success message with event count |
| purge non-terminal | `vo purge --instance inst-456` | Error: instance not terminal |
| purge not found | `vo purge --instance inst-999` | Error: instance not found |

---

## Open Questions

1. **ADR-008 mentions `--canonical` flag** but current `vo-cli` implementation only has `vo-cli history` with `--json` output. Is `--canonical` implemented separately or is it a planned feature? The test assumes it is a future feature that needs integration testing.

2. **Credential/Permission system in vault** — Is there an existing CLI flag or environment variable to set credentials for the canonical privileged path? Test assumes `VO_VAULT_CREDENTIAL` env var with appropriate permissions.

3. **GDPR purge does not destroy DEK** — Per ADR-025 §3, purge destroys the per-instance DEK. Is there a `purge_instance_with_key_destruction()` function that also destroys the encryption key, or is key destruction handled separately by the key management subsystem?

4. **RedactionPolicy configuration** — Is there a default `RedactionPolicy` per workflow type, or is it always explicitly provided? Tests assume explicit policy for clarity.

5. **vo-cli history --json vs --canonical** — The CLI currently produces `HistoryOutput` which is command history, not the `OperatorProjection` from ADR-025. Is there a separate command for querying workflow state projections, or is the command history the primary AI-facing interface?

---

## Files to Test

| File | Test module |
|------|-------------|
| `crates/vo-types/src/dual_representation.rs` | `mod tests` (existing) + new BDD scenarios |
| `crates/vo-types/src/command_history.rs` | `mod tests` (existing) + new BDD scenarios |
| `crates/vo-storage/src/purge.rs` | `mod tests` (existing) + new BDD scenarios |
| `crates/vo-cli/src/commands/history.rs` | `mod tests` (existing) + new BDD scenarios |
| `crates/vo-core/src/vault/mod.rs` | Permission/access control tests |

---

## Test Implementation Notes

- Use `serde_json::json!` macro for constructing test JSON
- Use `proptest::proptest` for property-based tests
- Use `kani:: harness` attribute for formal verification harnesses
- Use `cargo-fuzz` for fuzz targets (add to `fuzz/corpus/` directory)
- Use `cargo-mutants` for mutation testing: `cargo mutants --output-dir mutants_out`
- All error assertions must specify exact error variant, not just `is_err()`
- Use `rstest::rstest` for parametrized tests on `InstanceStatus` variants

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target
- [x] Every error variant in `StorageError` and `CommandHistoryError` has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
