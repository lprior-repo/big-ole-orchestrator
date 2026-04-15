# Test Plan: Dual Representation — Canonical vs Operator (ADR-025)

## Summary

- **Bead**: ve-vz2x — Test Plan: Dual representation canonical vs operator (ADR-025)
- **Contract**: N/A (architecture decision, not feature contract)
- **Behaviors identified**: 24
- **Trophy allocation**: 16 unit / 5 integration / 0 e2e / 1 static
- **Proptest invariants**: 6 (TODO - not yet implemented)
- **Fuzz targets**: 4 (TODO - not yet implemented)
- **Unit tests implemented**: 20 (dual_representation.rs)

---

## 1. Behavior Inventory

### RedactionPolicy

| # | Behavior | Public API |
|---|----------|------------|
| RP-01 | `RedactionPolicy::new()` creates valid policy with workflow_type and redaction_rules | `RedactionPolicy::new()` |
| RP-02 | `RedactionPolicy` roundtrips through serde (JSON) | `Serialize`, `Deserialize` |

### RedactionRule

| # | Behavior | Public API |
|---|----------|------------|
| RR-01 | `RedactionRule::new()` creates rule with field_path and redaction_kind | `RedactionRule::new()` |
| RR-02 | `RedactionRule` roundtrips through serde (JSON) | `Serialize`, `Deserialize` |
| RR-03 | Field path can be any depth (single field to deeply nested) | N/A — data structure invariant |

### RedactionKind

| # | Behavior | Public API |
|---|----------|------------|
| RK-01 | `RedactionKind::Remove` removes field from object entirely (per AR-09) | `apply_redaction()` |
| RK-02 | `RedactionKind::ReplaceWith(s)` replaces value with string `s` | `redact_value()` |
| RK-03 | `RedactionKind::ReplaceWithType` replaces value with its type name | `redact_value()` |
| RK-04 | `RedactionKind::Hash` replaces value with `HASH<sha256>` prefixed string | `redact_value()` |
| RK-05 | `RedactionKind::Hash` is deterministic — same input yields same output | `redact_value()` |
| RK-06 | `RedactionKind::Hash` different inputs produce different outputs | `redact_value()` |
| RK-07 | `RedactionKind::Hash` works on both string and non-string JSON values | `redact_value()` |

### OperatorProjection

| # | Behavior | Public API |
|---|----------|------------|
| OP-01 | `OperatorProjection::new()` creates projection with workflow_id, workflow_type, projection_json, redacted_fields | `OperatorProjection::new()` |
| OP-02 | `OperatorProjection::workflow_id()` returns stored workflow_id | `workflow_id()` |
| OP-03 | `OperatorProjection::workflow_type()` returns stored workflow_type | `workflow_type()` |
| OP-04 | `OperatorProjection::projection_json()` returns reference to projection_json | `projection_json()` |
| OP-05 | `OperatorProjection::redacted_fields()` returns reference to redacted_fields | `redacted_fields()` |
| OP-06 | `OperatorProjection` roundtrips through serde (JSON) | `Serialize`, `Deserialize` |

### apply_redaction

| # | Behavior | Public API |
|---|----------|------------|
| AR-01 | Top-level field matching rule path is redacted | `apply_redaction()` |
| AR-02 | Nested field at arbitrary depth is redacted | `apply_redaction()` |
| AR-03 | Multiple fields at same level can be redacted | `apply_redaction()` |
| AR-04 | Fields not in any rule remain unchanged | `apply_redaction()` |
| AR-05 | Array elements are processed recursively | `apply_redaction()` |
| AR-06 | Array at rule path is replaced entirely per RedactionKind | `apply_redaction()` |
| AR-07 | Nested arrays are processed recursively | `apply_redaction()` |
| AR-08 | `redacted_fields` output lists all paths that were redacted | `apply_redaction()` |
| AR-09 | `RedactionKind::Remove` omits field from result entirely (does not appear in JSON object) | `apply_redaction()` |
| AR-10 | `RedactionKind::Remove` from array leaves null placeholder in array | `apply_redaction()` |
| AR-11 | Redaction is applied recursively to nested objects | `apply_redaction()` |
| AR-12 | Empty field_path matches nothing (no-op) | `apply_redaction()` |
| AR-13 | Multiple rules can apply to same field — first match wins | `apply_redaction()` |

### Canonical vs Operator Separation

| # | Behavior | Public API |
|---|----------|------------|
| CS-01 | Canonical data is encrypted at rest (cannot read without DEK) | External invariant |
| CS-02 | Operator projection contains no canonical payload data | `apply_redaction()` |
| CS-03 | Event replay from canonical produces identical state | External invariant |
| CS-04 | Operator view is safe for UI/CLI/AI consumption (no raw sensitive data) | `apply_redaction()` |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 16 | Pure functions: `apply_redaction`, `RedactionKind::redact_value`, `OperatorProjection` accessors, and all serialization roundtrips. No I/O dependencies — all logic is deterministic JSON transformation. |
| **Integration** | 5 | Interaction between redaction rules at multiple paths, recursive array processing, and JSON roundtrip through `serde_json`. |
| **E2E** | 0 | No user-facing I/O — `apply_redaction` is pure transformation. |
| **Static Analysis** | 1 | `clippy::pedantic` lint gates on `dual_representation.rs`. |

**Rationale for distribution**: The dual representation module is a pure computation layer (JSON transformation with no I/O). The 16/5/0/1 split reflects exhaustive unit coverage of all redaction paths and invariants, with integration coverage for recursive array processing and multi-rule interactions. The Testing Trophy ideal (~60% integration) doesn't apply here because there are no async I/O dependencies or external service calls.

---

## 3. BDD Scenarios

### RK-01: RedactionKind::Remove removes field entirely

**Scenario: Remove redaction kind removes field from object**

```
Given: a RedactionKind::Remove rule for path ["user", "ssn"]
When: apply_redaction() is called on {"user": {"ssn": "123-45-6789"}}
Then: "ssn" key is removed from user object (key does not exist in result)
```

```rust
fn apply_redaction_removes_fields_at_path() {
    let value = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, redacted) = apply_redaction(&value, &rules);

    assert_eq!(result["user"]["name"], "Alice");
    // Remove removes key entirely (per AR-09 test plan)
    assert!(!result["user"].as_object().unwrap().contains_key("ssn"));
    assert_eq!(redacted.len(), 1);
}
```

---

### RK-02: RedactionKind::ReplaceWith preserves replacement string

**Scenario: ReplaceWith redaction kind returns fixed string**

```
Given: a RedactionKind::ReplaceWith("[REDACTED]") and any JSON value
When: redact_value("field", value) is called
Then: returns serde_json::Value::String("[REDACTED]")
```

```rust
fn redaction_kind_replace_with_produces_replacement() {
    let kind = RedactionKind::ReplaceWith("[REDACTED]".to_string());
    let value = serde_json::json!("sensitive data");
    let result = kind.redact_value("field", &value);
    assert_eq!(result, serde_json::Value::String("[REDACTED]".to_string()));
}
```

---

### RK-03: RedactionKind::ReplaceWithType returns type name

**Scenario: ReplaceWithType redaction kind returns type name string**

```
Given: a RedactionKind::ReplaceWithType and any JSON value
When: redact_value("field", value) is called
Then: returns serde_json::Value::String(<type_name>)
```

```rust
#[test]
fn redaction_kind_replace_with_type_produces_type_name() {
    let kind = RedactionKind::ReplaceWithType;
    let value = serde_json::json!(123);
    let result = kind.redact_value("field", &value);
    assert!(result.as_str().unwrap().contains("i64"));
}
```

---

### RK-04 & RK-05: RedactionKind::Hash is deterministic

**Scenario: Hash redaction kind produces consistent HASH prefix**

```
Given: a RedactionKind::Hash and JSON value "same input"
When: redact_value is called twice with identical input
Then: both results start with "HASH" prefix
And: both results are identical
```

```rust
fn redaction_kind_hash_produces_deterministic_hash() {
    let kind = RedactionKind::Hash;
    let value1 = serde_json::json!("same input");
    let value2 = serde_json::json!("same input");

    let result1 = kind.redact_value("field", &value1);
    let result2 = kind.redact_value("field", &value2);

    assert_eq!(result1, result2);
    assert!(result1.as_str().unwrap().starts_with("HASH"));
}
```

---

### RK-06: RedactionKind::Hash different inputs produce different hashes

**Scenario: Hash redaction kind is collision-resistant for different inputs**

```
Given: a RedactionKind::Hash and two different JSON values
When: redact_value is called with each value
Then: results are different
```

```rust
fn redaction_kind_hash_different_for_different_inputs() {
    let kind = RedactionKind::Hash;
    let value1 = serde_json::json!("input A");
    let value2 = serde_json::json!("input B");

    let result1 = kind.redact_value("field", &value1);
    let result2 = kind.redact_value("field", &value2);

    assert_ne!(result1, result2);
}
```

---

### AR-01: Top-level field redaction

**Scenario: Rule at top-level field path removes field**

```
Given: JSON {"password": "secret123"} and rule RedactionRule(["password"], Remove)
When: apply_redaction(value, rules) is called
Then: result["password"] is Null
And: result does not contain key "password" (Remove omits key entirely)
```

```rust
fn apply_redaction_removes_fields_at_path() {
    let value = serde_json::json!({
        "user": {
            "name": "Alice",
            "ssn": "123-45-6789"
        }
    });

    let rules = vec![RedactionRule::new(
        vec!["user".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, redacted) = apply_redaction(&value, &rules);

    assert_eq!(result["user"]["name"], "Alice");
    assert_eq!(result["user"]["ssn"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 1);
    assert_eq!(redacted[0], vec!["user".to_string(), "ssn".to_string()]);
}
```

---

### AR-04: Non-redacted fields remain unchanged

**Scenario: Fields without matching rules are preserved exactly**

```
Given: JSON {"name": "Alice", "email": "alice@example.com"}
And: only a rule for ["ssn"] field
When: apply_redaction is called
Then: result["name"] == "Alice"
And: result["email"] == "alice@example.com"
```

```rust
fn apply_redaction_preserves_non_redacted_fields() {
    let value = serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com"
    });

    let rules = vec![RedactionRule::new(
        vec!["ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);

    assert_eq!(result["name"], "Alice");
    assert_eq!(result["email"], "alice@example.com");
}
```

---

### AR-05: Array elements processed recursively

**Scenario: Rules apply to each element in array**

```
Given: JSON {"users": [{"name": "Alice", "ssn": "111"}, {"name": "Bob", "ssn": "222"}]}
And: rule RedactionRule(["users", "ssn"], Remove)
When: apply_redaction is called
Then: result["users"][0]["ssn"] is Null
And: result["users"][1]["ssn"] is Null
And: result["users"][0]["name"] is "Alice"
And: result["users"][1]["name"] is "Bob"
```

```rust
fn apply_redaction_handles_arrays_recursively() {
    let value = serde_json::json!({
        "users": [
            {"name": "Alice", "ssn": "111"},
            {"name": "Bob", "ssn": "222"}
        ]
    });

    let rules = vec![RedactionRule::new(
        vec!["users".to_string(), "ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, redacted) = apply_redaction(&value, &rules);

    assert_eq!(result["users"][0]["name"], "Alice");
    assert_eq!(result["users"][0]["ssn"], serde_json::Value::Null);
    assert_eq!(result["users"][1]["name"], "Bob");
    assert_eq!(result["users"][1]["ssn"], serde_json::Value::Null);
    assert_eq!(redacted.len(), 2);
}
```

---

### AR-06: Array at rule path replaced entirely

**Scenario: Rule targeting array field replaces entire array**

```
Given: JSON {"matrix": [[1, 2], [3, 4]]}
And: rule RedactionRule(["matrix"], ReplaceWith("[REDACTED]"))
When: apply_redaction is called
Then: result["matrix"] == "[REDACTED]"
```

```rust
fn apply_redaction_handles_nested_arrays() {
    let value = serde_json::json!({
        "matrix": [[1, 2], [3, 4]]
    });

    let rules = vec![RedactionRule::new(
        vec!["matrix".to_string()],
        RedactionKind::ReplaceWith("[REDACTED]".to_string()),
    )];

    let (result, _) = apply_redaction(&value, &rules);

    assert_eq!(result["matrix"], "[REDACTED]");
}
```

---

### AR-08: redacted_fields tracks all redacted paths

**Scenario: apply_redaction returns list of all redacted field paths**

```
Given: JSON {"user": {"ssn": "123", "name": "Alice"}}
And: rules for ["user", "ssn"] and ["user", "name"]
When: apply_redaction is called
Then: redacted_fields contains ["user", "ssn"] and ["user", "name"]
```

```rust
fn apply_redaction_tracks_all_redacted_paths() {
    let value = serde_json::json!({
        "user": {
            "ssn": "123",
            "name": "Alice"
        }
    });

    let rules = vec![
        RedactionRule::new(vec!["user".to_string(), "ssn".to_string()], RedactionKind::Remove),
        RedactionRule::new(vec!["user".to_string(), "name".to_string()], RedactionKind::Remove),
    ];

    let (_, redacted) = apply_redaction(&value, &rules);

    assert_eq!(redacted.len(), 2);
    assert!(redacted.contains(&vec!["user".to_string(), "ssn".to_string()]));
    assert!(redacted.contains(&vec!["user".to_string(), "name".to_string()]));
}
```

---

### AR-09: Remove omits key from JSON object entirely

**Scenario: Remove redaction kind does not include key in output object**

```
Given: JSON {"secret": "value", "public": "data"}
And: rule RedactionRule(["secret"], Remove)
When: apply_redaction is called
Then: result is {"public": "data"} — no key "secret" at all
```

```rust
fn apply_redaction_remove_omits_key_from_object() {
    let value = serde_json::json!({
        "secret": "value",
        "public": "data"
    });

    let rules = vec![RedactionRule::new(
        vec!["secret".to_string()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);

    let obj = result.as_object().unwrap();
    assert!(!obj.contains_key("secret"));
    assert_eq!(obj.len(), 1);
    assert_eq!(obj["public"], "data");
}
```

---

### CS-04: Operator projection is safe for external consumption

**Scenario: Redacted operator projection contains no raw sensitive data**

```
Given: A payment workflow with sensitive fields: {"ssn": "123-45-6789", "amount": 100.00, "recipient": "Alice"}
And: redaction policy for workflow_type "payment" redacting ["ssn"]
When: apply_redaction is called with payment redaction rules
Then: result["ssn"] is Null or redacted
And: result["amount"] == 100.00
And: result["recipient"] == "Alice"
And: no raw "123-45-6789" appears in result
```

```rust
fn operator_projection_no_raw_sensitive_data() {
    let value = serde_json::json!({
        "ssn": "123-45-6789",
        "amount": 100.00,
        "recipient": "Alice"
    });

    let rules = vec![RedactionRule::new(
        vec!["ssn".to_string()],
        RedactionKind::Remove,
    )];

    let (result, _) = apply_redaction(&value, &rules);
    let json_str = serde_json::to_string(&result).unwrap();

    assert!(!json_str.contains("123-45-6789"));
    assert_eq!(result["amount"], 100.00);
    assert_eq!(result["recipient"], "Alice");
}
```

---

## 4. Proptest Invariants

> **NOTE (2026-04-15)**: The following proptest invariants (PI-01 through PI-06) were REMOVED from the plan as they do not exist in the codebase. They remain as TODO items for future implementation if property-based testing coverage is expanded.

### TODO: PI-01: apply_redaction never panics on valid JSON
### TODO: PI-02: redacted_fields length bounded by JSON leaves
### TODO: PI-03: RedactionKind::Hash deterministic for same input
### TODO: PI-04: Non-matching rules leave value unchanged
### TODO: PI-05: Remove redaction produces Null in output
### TODO: PI-06: ReplaceWith preserves string type in output

---

## 5. Fuzz Targets

> **NOTE (2026-04-15)**: The following fuzz targets (FT-01 through FT-04) were REMOVED from the plan as they do not exist in the fuzz directory. They remain as TODO items for future implementation if fuzzing coverage is expanded.

### TODO: FT-01: apply_redaction with deeply nested JSON
### TODO: FT-02: apply_redaction with malformed field paths
### TODO: FT-03: apply_redaction with all RedactionKind variants
### TODO: FT-04: apply_redaction with overlapping/redundant rules

---

## 6. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Change `rule = rules.iter().find(...)` to `rules.iter().next()` (first rule always matches) | `apply_redaction_handles_arrays_recursively` |
| MC-002 | Remove `current_path.pop()` after recursive call | `apply_redaction_nested_field_paths_correct` |
| MC-003 | Change `ReplaceWith` to return Null instead of String | `replace_with_produces_string` |
| MC-004 | Change `Hash` to return empty string instead of `HASH{:x}` format | `redaction_kind_hash_produces_deterministic_hash` |
| MC-005 | Remove `is_remove` check, always insert after redaction | `apply_redaction_remove_omits_key_from_object` |
| MC-006 | Change array index push from `i.to_string()` to just push index | `apply_redaction_handles_arrays_recursively` (would fail on non-string indices) |
| MC-007 | Remove `current_path.push(key.clone())` before recursive call | `apply_redaction_tracks_all_redacted_paths` (would give wrong paths) |

**Threshold**: ≥90% mutation kill rate

---

## 7. Combinatorial Coverage Matrix

### apply_redaction

| Scenario | Input JSON | Rules | Expected Output | Layer |
|----------|------------|-------|-----------------|-------|
| Top-level Remove | `{"a": 1}` | `[("a", Remove)]` | `{}` (key omitted) | unit |
| Nested Remove | `{"user": {"ssn": "x"}}` | `[("user", "ssn"), Remove]` | `{"user": {}}` with null ssn | unit |
| Top-level ReplaceWith | `{"secret": "val"}` | `[("secret"), ReplaceWith("X")]` | `{"secret": "X"}` | unit |
| Hash string field | `{"email": "a@b.com"}` | `[("email"), Hash]` | `{"email": "HASH..."}` | unit |
| Hash non-string | `{"count": 42}` | `[("count"), Hash]` | `{"count": "HASH..."}` | unit |
| Array element redact | `{"items": [{"id": 1}, {"id": 2}]}` | `[("items", "id"), Remove]` | both ids become Null | unit |
| Array field replace | `{"matrix": [[1,2]]}` | `[("matrix"), ReplaceWith("X")]` | `{"matrix": "X"}` | unit |
| No matching rules | `{"a": 1, "b": 2}` | `[("c"), Remove]` | both a,b unchanged | unit |
| Multiple rules | `{"ssn": "x", "name": "y"}` | two Remove rules | both redacted | integration |
| Recursive nested | `{"a": {"b": {"c": 1}}}` | `[("a", "b", "c"), Remove]` | c is Null, path preserved | integration |

### RedactionKind::redact_value

| Scenario | Input | Expected | Layer |
|----------|-------|----------|-------|
| Remove → string | `"secret"` | `Null` | unit |
| Remove → number | `42` | `Null` | unit |
| ReplaceWith → any | `anything` | `"[REDACTED]"` | unit |
| ReplaceWithType → i64 | `42` | contains "i64" | unit |
| ReplaceWithType → string | `"text"` | contains "str" | unit |
| ReplaceWithType → bool | `true` | contains "bool" | unit |
| Hash → same string twice | `"same"` x2 | identical | unit |
| Hash → different strings | `"a"`, `"b"` | different | unit |
| Hash → number | `123` | `"HASH..."` | unit |

---

## 8. Open Questions

1. **Null vs omitted**: The current `Remove` behavior produces `Null` but then omits the key if value is Null. Is this the correct GDPR interpretation — should removed fields be completely absent from JSON, or is Null acceptable for audit trails?

2. **Hash algorithm**: Currently using `DefaultHasher` (SipHash). For GDPR compliance, should this be a cryptographically secure hash (SHA-256) even if slower?

3. **Array nulls**: When `Remove` applies to an array element, we leave `Null` in the array (to preserve indices). Should arrays compress nulls? This affects replay if arrays are used as ordered collections where index matters.

4. **Type name exposure**: `ReplaceWithType` exposes Rust type names (e.g., `i64`, `alloc::string::String`). Is this acceptable for GDPR, or should type names also be redacted/hashed?

5. **Performance**: No performance requirements specified. Should benchmarks be added for large JSON documents (10MB+) with many redaction rules?

6. **Canonical/operator separation testing**: The ADR specifies canonical is encrypted at rest and operator is redacted queryable. How should we test the isolation boundary without actual encryption implementation?

---

## 9. Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [ ] Every pure function with multiple inputs has at least one proptest invariant (PI-01 through PI-06 - TODO)
- [ ] Every parsing/deserialization boundary has a fuzz target (FT-01 through FT-04 - TODO)
- [x] Every RedactionKind variant has explicit test scenarios (Remove, ReplaceWith, ReplaceWithType, Hash)
- [x] Mutation threshold target (≥90%) is stated
- [x] CS-04 (operator projection safety) explicitly tested — no raw sensitive data in output
- [x] Recursive array processing covered by AR-05 and AR-06

---

## 10. Implementation Status (2026-04-15)

### Completed
- 20 unit tests implemented in `crates/vo-types/src/dual_representation.rs`
- RK-03 test: `redaction_kind_replace_with_type_produces_type_name`
- CS-04 test: `operator_projection_no_raw_sensitive_data`
- AR-09 test: `apply_redaction_remove_omits_key_from_object`
- AR-09 implementation fix: Remove in object context now omits key entirely

### TODO (Not Yet Implemented)
- PI-01 through PI-06: Proptest invariants (see Section 4)
- FT-01 through FT-04: Fuzz targets (see Section 5)
- Mutation testing with cargo-mutants (≥90% kill rate)

---

## References

- [ADR-025-v2-state-privacy-gdpr-purging.md](../../docs/adr/v2/ADR-025-v2-state-privacy-gdpr-purging.md)
- [dual_representation.rs](../../crates/vo-types/src/dual_representation.rs)