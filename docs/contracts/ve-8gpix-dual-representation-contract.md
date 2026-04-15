# Dual Representation Redaction Contract (ADR-025)

> **Task**: ve-8gpix - CONTRACT: Dual representation redaction types
> **State**: 1 (Contract-First Design)
> **Reference**: ADR-025 "State Privacy and GDPR Purging"
> **Implementation**: `./crates/vo-types/src/dual_representation.rs`

---

## 1. Domain Model

### 1.1 RedactionPolicy

**Purpose**: Per-workflow-type configuration for redaction rules.

```
RedactionPolicy {
  workflow_type: String
  redaction_rules: [RedactionRule]
}
```

**Preconditions**:
- `workflow_type` must be a valid, non-empty workflow type identifier
- `redaction_rules` may be empty (identity transformation)

**Invariants**:
- `redaction_rules` is a well-formed list of rules (no duplicate paths)

**Postconditions**:
- Policy is immutable after construction

---

### 1.2 RedactionRule

**Purpose**: Defines a single field-level redaction rule.

```
RedactionRule {
  field_path: [String]    // Path from root (e.g., ["user", "ssn"])
  redaction_kind: RedactionKind
}
```

**Preconditions**:
- `field_path` must be non-empty (at least one component)
- `field_path` components must be valid JSON object keys or array indices

**Invariants**:
- `field_path` is canonicalized (no empty components)

**Postconditions**:
- Rule is immutable after construction

---

### 1.3 RedactionKind

**Purpose**: Specifies the redaction transformation to apply.

```
enum RedactionKind {
  Remove,              // Replace with null
  ReplaceWith(String), // Replace with fixed value
  ReplaceWithType,     // Replace with type name string
  Hash                 // Replace with deterministic hash
}
```

**Preconditions**:
- `ReplaceWith(replacement)`: `replacement` must be non-empty string

**Invariants**:
- `RedactionKind` is a closed type (no external variants)
- `Hash` is deterministic: same input → same output
- `Hash` preserves uniqueness: different inputs → different outputs

**Postconditions**:
- `redact_value(field, value)` returns a transformed `Value`
- `redact_value` is pure (no side effects)

---

### 1.4 OperatorProjection

**Purpose**: Redacted view for UI/CLI/AI consumption.

```
OperatorProjection {
  workflow_id: String
  workflow_type: String
  projection_json: Value        // Redacted JSON
  redacted_fields: [[String]]   // Paths of all redacted fields
}
```

**Preconditions**:
- `workflow_id` is a valid workflow identifier
- `workflow_type` matches the policy's `workflow_type`
- `projection_json` is a valid JSON value
- `redacted_fields` contains only paths that exist in `projection_json`

**Invariants**:
- **INVARIANT-1**: Operator view contains NO redacted sensitive fields
- **INVARIANT-2**: `redacted_fields` is exhaustive (no missing redactions)
- **INVARIANT-3**: Non-redacted fields are preserved with original structure
- **INVARIANT-4**: `redacted_fields` paths are unique (no duplicates)

**Postconditions**:
- Projection is serializable/deserializable (roundtrip preserved)
- Projection exposes getters for all fields

---

## 2. Core Invariants

### INVARIANT-1: Redaction Completeness

For any `OperatorProjection` created from a canonical value:

```
∀ field_path ∈ redacted_fields:
  operator_view[field_path] ∈ {null, "[REDACTED]", "HASH...", type_name}
  AND
  canonical_view[field_path] ≠ operator_view[field_path]
```

**Formal specification**:
```
P: RedactionPolicy
V: Value (canonical view)
R: OperatorProjection

Pre: policy_match(R.workflow_type, P.workflow_type)
Pre: R.projection_json = apply_redaction(V, P.redaction_rules)

Post: ∀ path ∈ R.redacted_fields:
  path ∈ paths_to_redact(V, P)
  AND
  R.projection_json @ path ≠ V @ path
```

### INVARIANT-2: Structure Preservation

Non-redacted fields preserve original structure and values:

```
∀ field_path ∉ redacted_fields:
  operator_view[field_path] = canonical_view[field_path]
```

**Formal specification**:
```
V: Value
R: RedactionPolicy
(P, redacted) = apply_redaction(V, R)

∀ path ∉ redacted:
  get_at(V, path) = get_at(P, path)
```

### INVARIANT-3: Determinism

Redaction is a pure function:

```
∀ V, R:
  apply_redaction(V, R) = apply_redaction(V, R)
```

**Formal specification**:
```
∀ V1, V2, R:
  V1 = V2 → apply_redaction(V1, R) = apply_redaction(V2, R)
```

### INVARIANT-4: Hash Uniqueness

Hash redaction preserves uniqueness for correlation:

```
∀ v1, v2:
  v1 ≠ v2 → hash(v1) ≠ hash(v2)
```

**Formal specification**:
```
∀ v1, v2 ∈ Value:
  v1 ≠ v2 → sha256(v1) ≠ sha256(v2)
  (modulo collision probability: 2^-256)
```

---

## 3. Pre/Post Conditions for Redaction Operations

### 3.1 apply_redaction

**Signature**:
```
fn apply_redaction(value: &Value, rules: &[RedactionRule]) -> (Value, [[String]])
```

**Preconditions**:
1. `rules` is a valid list of rules (no malformed paths)
2. Each rule's `field_path` is non-empty
3. Each rule's `field_path` components are valid JSON keys/indices

**Postconditions**:
1. `(result, redacted)` satisfies:
   ```
   ∀ path ∈ redacted:
     result @ path ∈ {null, "[REDACTED]", "HASH...", type_name}
   ```
2. `redacted` is exhaustive:
   ```
   ∀ path where path matches a rule:
     path ∈ redacted
   ```
3. Structure preservation:
   ```
   ∀ path ∉ redacted:
     result @ path = value @ path
   ```
4. Determinism:
   ```
   apply_redaction(value, rules) = apply_redaction(value, rules)
   ```

**Failure modes**:
- **None**: `apply_redaction` never panics; it's a pure function

---

### 3.2 OperatorProjection::new

**Signature**:
```
fn new(
  workflow_id: String,
  workflow_type: String,
  projection_json: Value,
  redacted_fields: [[String]],
) -> Self
```

**Preconditions**:
1. `workflow_id` is non-empty
2. `workflow_type` is non-empty
3. `projection_json` is valid JSON
4. `redacted_fields` is well-formed (non-empty paths)
5. **CRITICAL**: `projection_json` already has redactions applied

**Postconditions**:
1. `self.workflow_id = workflow_id`
2. `self.workflow_type = workflow_type`
3. `self.projection_json = projection_json`
4. `self.redacted_fields = redacted_fields`

**Failure modes**:
- **None**: Constructor is infallible; preconditions are caller's responsibility

---

## 4. Error Taxonomy

### E1: Policy Misconfiguration

**Description**: Invalid or conflicting redaction policy.

**Subtypes**:
- **E1-1**: Duplicate field paths in rules
- **E1-2**: Empty field path in rule
- **E1-3**: Invalid JSON path components

**Prevention**: Policy validation before construction.

**Recovery**: Reject policy; return validation error to user.

---

### E2: Projection Mismatch

**Description**: `OperatorProjection.redacted_fields` doesn't match `projection_json`.

**Subtypes**:
- **E2-1**: Missing redacted field path
- **E2-2**: Extra redacted field path (not in JSON)
- **E2-3**: Redacted path exists but value not transformed

**Prevention**: Contract enforcement in `OperatorProjection::new`.

**Recovery**: Reject projection; log invariant violation.

---

### E3: Hash Collision

**Description**: Two different values produce same hash.

**Probability**: 2^-256 (negligible for practical purposes).

**Prevention**: Use SHA-256 (already implemented).

**Recovery**: Accept collision risk; it's cryptographically infeasible.

---

### E4: Structure Loss

**Description**: Redaction removes entire object/array instead of field.

**Cause**: Incorrect path matching logic.

**Prevention**: Implement path matching as per ADR-025 spec.

**Recovery**: Audit `apply_redaction` implementation.

---

## 5. Formal Specifications

### 5.1 Path Matching

**Specification**:
```
matches_rule(current_path: [String], rule_path: [String]) -> bool

Pre: rule_path is non-empty
Pre: current_path contains rule_path as prefix
Pre: rule_path indices (if any) don't exceed array bounds

Post: result == (current_path starts with rule_path)
```

**Implementation note**: Current implementation skips array indices in comparison, which may be a bug.

---

### 5.2 Redaction Semantics

**Specification**:
```
redact_value(kind: RedactionKind, field_name: String, value: Value) -> Value

Post:
  kind = Remove        → result = null
  kind = ReplaceWith(r) → result = r
  kind = ReplaceWithType → result = type_name(value)
  kind = Hash          → result = hash(value)
```

**Postconditions**:
- `RedactionKind::Remove` always produces `Value::Null`
- `RedactionKind::ReplaceWith` always produces `Value::String(replacement)`
- `RedactionKind::ReplaceWithType` always produces `Value::String(type_name)`
- `RedactionKind::Hash` always produces `Value::String("HASH{hex}")`

---

### 5.3 Recursive Application

**Specification**:
```
apply_recursive(value: Value, rules: [RedactionRule]) -> Value

Pre: value is valid JSON
Pre: rules is valid redaction rules

Post:
  ∀ path where path matches a rule:
    result @ path ∈ {null, "[REDACTED]", "HASH...", type_name}
  ∀ path where path doesn't match any rule:
    result @ path = value @ path
```

**Implementation note**: Current implementation tracks redacted fields but may have path-matching bugs.

---

## 6. Contract Violations

### Violation: Incomplete Redaction

**Scenario**: A sensitive field is not redacted in operator view.

**Impact**: **CRITICAL** - GDPR violation, data leak.

**Detection**:
```
∀ policy ∈ RedactionPolicy:
  ∀ field_path ∈ policy.redaction_rules:
    operator_view @ field_path ≠ canonical_view @ field_path
```

**Prevention**:
1. Contract enforcement in `OperatorProjection::new`
2. Property-based tests (proptest)
3. Integration tests with ADR-025 scenarios

---

### Violation: False Redaction

**Scenario**: A public field is incorrectly redacted.

**Impact**: **HIGH** - Data loss, broken UI.

**Detection**:
```
∀ field_path ∉ policy.redaction_rules:
  operator_view @ field_path = canonical_view @ field_path
```

**Prevention**:
1. Policy validation tests
2. Roundtrip verification

---

## 7. Testing Requirements

### Unit Tests

1. **RedactionKind variants**:
   - `Remove` → `Value::Null`
   - `ReplaceWith` → replacement string
   - `ReplaceWithType` → type name string
   - `Hash` → deterministic hash string

2. **apply_redaction**:
   - Single field redaction
   - Nested field redaction
   - Array element redaction
   - Multiple simultaneous redactions
   - Empty rules (identity)
   - Deeply nested paths

3. **OperatorProjection**:
   - Roundtrip serialization
   - Getter methods
   - Redacted fields tracking

---

### Property-Based Tests (proptest)

```rust
proptest! {
  #[test]
  fn redaction_is_deterministic(value in json_value_arb(), rules in redaction_rules_arb()) {
    let (r1, _) = apply_redaction(&value, &rules);
    let (r2, _) = apply_redaction(&value, &rules);
    prop_assert_eq!(r1, r2);
  }

  #[test]
  fn redaction_completeness(value in json_value_arb(), rules in redaction_rules_arb()) {
    let (result, redacted) = apply_redaction(&value, &rules);
    for path in &redacted {
      // Verify field is actually redacted
      let redacted_value = get_at(&result, path);
      prop_assert!(is_redacted(redacted_value));
    }
  }

  #[test]
  fn non_redaction_preservation(value in json_value_arb(), rules in redaction_rules_arb()) {
    let (result, redacted) = apply_redaction(&value, &rules);
    for path in all_paths(&value) {
      if !redacted.contains(&path) {
        prop_assert_eq!(get_at(&value, path), get_at(&result, path));
      }
    }
  }
}
```

---

## 8. Implementation Notes

### 8.1 Path Matching Bug (Potential)

**Location**: `dual_representation.rs:140-155`

**Issue**: Path matching logic skips array indices:
```rust
while cpi < current_path.len() && current_path[cpi].parse::<usize>().is_ok() {
    cpi += 1;  // Skips array indices
}
```

**Problem**: This means `"users[0].ssn"` won't match `"users.ssn"`.

**Correction needed**: Implement proper array index handling.

---

### 8.2 Null Preservation

**Observation**: Current implementation replaces removed fields with `null`, not removing them from JSON.

**ADR-025 spec**: "Field is removed entirely from the operator projection."

**Correction needed**: Decide between:
- Option A: Replace with `null` (current)
- Option B: Remove key entirely from object

---

## 9. Summary

| Type | Fields | Invariants |
|------|--------|------------|
| `RedactionPolicy` | `workflow_type`, `redaction_rules` | Policy is immutable, rules are well-formed |
| `RedactionRule` | `field_path`, `redaction_kind` | Path is non-empty, canonical |
| `RedactionKind` | enum | Closed type, deterministic |
| `OperatorProjection` | `workflow_id`, `workflow_type`, `projection_json`, `redacted_fields` | Redaction completeness, structure preservation, tracking |

| Operation | Preconditions | Postconditions |
|-----------|---------------|----------------|
| `apply_redaction` | Valid rules, valid JSON | Redacted result, exhaustive redacted list |
| `OperatorProjection::new` | Valid workflow IDs, pre-redacted JSON | Immutable projection with getters |

| Error | Severity | Prevention |
|-------|----------|------------|
| Incomplete redaction | CRITICAL | Contract enforcement, tests |
| False redaction | HIGH | Policy validation |
| Hash collision | LOW | SHA-256 (negligible) |

---

## 10. References

- **ADR-025**: "State Privacy and GDPR Purging"
- **Implementation**: `./crates/vo-types/src/dual_representation.rs`
- **Task**: ve-8gpix (CONTRACT: Dual representation redaction types)

---

## 11. Open Issues

### Issue 1: Path Matching Logic

**Status**: Needs investigation

**Description**: The path matching logic in `matches_rule` skips array indices, which may prevent correct matching of array element paths.

**Example**:
```json
{
  "users": [
    {"ssn": "123-45-6789"}
  ]
}
```
Rule: `["users", "ssn"]` (missing array index)
Should this match `users[0].ssn`?

**Action**: Clarify ADR-025 spec or file follow-up bead.

---

### Issue 2: Remove vs Null

**Status**: Needs clarification

**Description**: ADR-025 says "Field is removed entirely", but implementation uses `null`.

**Question**: Should `RedactionKind::Remove` remove the key entirely or set to `null`?

**Action**: Clarify with ADR author or decide based on GDPR requirements.

---

### Issue 3: RedactionPath vs Path

**Status**: Design decision needed

**Description**: Should we use dot notation (`user.ssn`), bracket notation (`users[0].ssn`), or both?

**Current implementation**: String vector `["users", "ssn"]` (dot notation).

**Action**: Define canonical path format in ADR-025.

---
