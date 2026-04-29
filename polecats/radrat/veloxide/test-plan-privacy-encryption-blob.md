# Test Plan: Privacy/Encryption/Blob Exhaustive Test Strategy

## Summary
- Behaviors identified: 18
- Trophy allocation: 12 unit / 4 integration / 2 proptest
- Fuzz targets: 2
- Kani harnesses: 0

## 1. Behavior Inventory

### Redaction Behaviors (apply_redaction)

1. "RedactionPolicy redact_value returns Null when RedactionKind::Remove"
2. "RedactionPolicy redact_value returns replacement string when RedactionKind::ReplaceWith"
3. "RedactionPolicy redact_value returns type name when RedactionKind::ReplaceWithType"
4. "RedactionPolicy redact_value returns deterministic hash when RedactionKind::Hash"
5. "apply_redaction removes fields matching rule path"
6. "apply_redaction replaces fields at nested paths"
7. "apply_redaction hashes fields at array indices"
8. "apply_redaction handles arrays recursively"
9. "apply_redaction preserves non-matching structure"
10. "apply_redaction with empty rules produces identity"

### DEK/KEK Lifecycle Behaviors

11. "DekStore generates new DEK and stores wrapped DEK"
12. "DekStore retrieves active DEK for instance"
13. "DekStore rotates DEK: retires old, generates new"
14. "DekStore retires DEK (crypto-shred)"
15. "DekStore returns error when retrieving retired DEK"

### Blob Behaviors

16. "BlobRef validates blob_id as ULID, size_bytes > 0, content_hash as lowercase hex"
17. "BlobStatus state machine: Pending → DurablyStored → Published (terminal)"
18. "BlobStatus state machine: Pending → Failed (terminal)"

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| Unit (Calc) | 12 | Pure redaction calc, BlobRef validation, state transitions |
| Integration | 4 | DekStore lifecycle with real storage, redaction + policy integration |
| Proptest | 2 | Redaction invariants, encryption correctness |
| Fuzz | 2 | BlobRef parsing, redaction JSON parsing |

## 3. BDD Scenarios

### Redaction Scenarios

#### Behavior 1: RedactionKind::Remove produces Null

```
Given: RedactionKind::Remove
When: redact_value("field", serde_json::json!("sensitive")) is called
Then: returns serde_json::Value::Null
```

**Test name**: `fn redaction_kind_remove_produces_null`

#### Behavior 2: RedactionKind::ReplaceWith produces replacement

```
Given: RedactionKind::ReplaceWith("[REDACTED]".to_string())
When: redact_value("field", serde_json::json!("secret")) is called
Then: returns serde_json::Value::String("[REDACTED]")
```

**Test name**: `fn redaction_kind_replace_with_produces_replacement`

#### Behavior 3: RedactionKind::Hash produces deterministic hash

```
Given: RedactionKind::Hash
When: redact_value("field", serde_json::json!("same")) called twice
Then: returns same hash both times
And: hash starts with "HASH"
```

**Test name**: `fn redaction_kind_hash_produces_deterministic_hash`

#### Behavior 4: apply_redaction removes fields at path

```
Given: JSON {"user": {"name": "Alice", "ssn": "123-45-6789"}}
And: rule for ["user", "ssn"] with Remove
When: apply_redaction is called
Then: result["user"]["ssn"] is Null
And: result["user"]["name"] is "Alice"
```

**Test name**: `fn apply_redaction_removes_fields_at_path`

#### Behavior 5: apply_redaction handles arrays recursively

```
Given: JSON {"users": [{"name": "Alice", "ssn": "111"}, {"name": "Bob", "ssn": "222"}]}
And: rule for ["users", "ssn"] with Remove
When: apply_redaction is called
Then: both array elements have ssn as Null
```

**Test name**: `fn apply_redaction_handles_arrays_recursively`

#### Behavior 6: apply_redaction empty rules produces identity

```
Given: JSON {"key": "value", "nested": {"a": 1}}
And: empty rules
When: apply_redaction is called
Then: result equals original value
```

**Test name**: `fn apply_redaction_empty_rules_produces_identity`

### DEK/KEK Lifecycle Scenarios

#### Behavior 7: Generate DEK

```
Given: instance_id, kek
When: generate_and_store_dek(instance_id, kek) is called
Then: returns DekId
And: retrieve_dek(instance_id, kek) succeeds
```

**Test name**: `fn dek_store_generates_and_stores_dek`

#### Behavior 8: Retrieve DEK

```
Given: existing DEK for instance_id
When: retrieve_dek(instance_id, kek) is called
Then: returns [u8; 32] (unwrapped DEK)
```

**Test name**: `fn dek_store_retrieves_active_dek`

#### Behavior 9: Rotate DEK

```
Given: existing DEK for instance_id
When: rotate_dek(instance_id, kek) is called
Then: returns new DekId
And: old DEK is marked Retired
And: new DEK is Active
```

**Test name**: `fn dek_store_rotates_dek`

#### Behavior 10: Retire DEK (crypto-shred)

```
Given: existing DEK for instance_id
When: retire_dek(instance_id) is called
Then: retrieve_dek returns DekStoreError::DekRetired
```

**Test name**: `fn dek_store_retire_dek_makes_it_unrecoverable`

#### Behavior 11: Error on retired DEK retrieval

```
Given: retired DEK
When: retrieve_dek(instance_id, kek) is called
Then: returns DekStoreError::DekRetired
```

**Test name**: `fn dek_store_returns_error_for_retired_dek`

### Blob Scenarios

#### Behavior 12: BlobRef valid construction

```
Given: valid blob_id (26-char ULID), size_bytes > 0, valid lowercase hex content_hash
When: BlobRef::new(blob_id, size_bytes, content_hash) is called
Then: returns Ok(BlobRef)
```

**Test name**: `fn blobref_constructs_with_valid_fields`

#### Behavior 13: BlobRef rejects invalid ULID

```
Given: blob_id = "not-a-ulid"
When: BlobRef::new is called
Then: returns Err(ParseError::InvalidFormat)
```

**Test name**: `fn blobref_rejects_invalid_ulid_blob_id`

#### Behavior 14: BlobRef rejects zero size

```
Given: size_bytes = 0
When: BlobRef::new is called
Then: returns Err(ParseError::ZeroValue)
```

**Test name**: `fn blobref_rejects_zero_size_bytes`

#### Behavior 15: BlobRef rejects invalid hex

```
Given: content_hash = "GHIJKLMN" (not lowercase hex)
When: BlobRef::new is called
Then: returns Err(ParseError::InvalidCharacters)
```

**Test name**: `fn blobref_rejects_invalid_hex_characters`

#### Behavior 16: BlobStatus valid transitions

```
Given: BlobStatus::Pending
When: can_transition_to(DurablyStored) is called
Then: returns true

Given: BlobStatus::Pending
When: can_transition_to(Published) is called
Then: returns false
```

**Test names**:
- `fn blob_status_pending_can_transition_to_durably_stored`
- `fn blob_status_pending_cannot_skip_to_published`

#### Behavior 17: BlobStatus Pending to Failed

```
Given: BlobStatus::Pending
When: can_transition_to(Failed) is called
Then: returns true
```

**Test name**: `fn blob_status_pending_can_transition_to_failed`

#### Behavior 18: BlobStatus all variants in order

```
Given: BlobStatus enum
When: all_variants() is called
Then: returns [Pending, DurablyStored, Published, Failed] in order
```

**Test name**: `fn blob_status_all_variants_returns_four_in_declared_order`

## 4. Proptest Invariants

### Proptest 1: Redaction idempotency

**Invariant**: Applying redaction twice produces same result as once
```
apply_redaction(apply_redaction(v, r), r) == apply_redaction(v, r)
```

**Strategy**: Random JSON values, random redaction rules

### Proptest 2: Redaction commutativity

**Invariant**: Order of rules doesn't matter
```
apply_redaction(v, [r1, r2]) == apply_redaction(v, [r2, r1])
```

**Strategy**: Random JSON values, pairs of random rules

### Proptest 3: Hash determinism

**Invariant**: Same input always produces same hash
```
hash(v1) == hash(v2) iff v1 == v2 (for string values)
```

**Strategy**: Random string values

### Proptest 4: DEK encryption roundtrip

**Invariant**: encrypt(decrypt(encrypt(dek))) == encrypt(dek)
```
wrap_dek(unwrap_dek(wrap_dek(dek))) == wrap_dek(dek)
```

**Strategy**: Random 32-byte keys

## 5. Fuzz Targets

### Fuzz Target 1: BlobRef::new

**Input type**: Random bytes → try to construct BlobRef
**Risk**: Panic on invalid input, bypass validation
**Corpus seeds**: Valid BlobRef, edge cases (empty, wrong length, invalid ULID)

### Fuzz Target 2: apply_redaction JSON parsing

**Input type**: Random JSON bytes
**Risk**: Panic on malformed JSON, unexpected types
**Corpus seeds**: Valid JSON with various structures, empty objects, deep nesting

## 6. Mutation Checkpoints

**Critical mutations to survive**:

| Mutation | Must be caught by |
|----------|------------------|
| Remove → return original value | `apply_redaction_removes_fields_at_path` |
| Hash → return original value | `apply_redaction_hashes_fields_at_path` |
| BlobStatus can_transition_to skip check | `blob_status_pending_cannot_skip_to_published` |
| DEK retire → mark active | `dek_store_retire_dek_makes_it_unrecoverable` |

**Threshold**: 90% mutation kill rate minimum

## 7. Combinatorial Coverage Matrix

### RedactionKind

| Input Type | Remove | ReplaceWith | ReplaceWithType | Hash |
|------------|--------|-------------|-----------------|------|
| String | Null | replacement | type name | HASH{...} |
| Number | Null | replacement | type name | HASH{...} |
| Object | Null | replacement | type name | HASH{...} |
| Array | Null | replacement | type name | HASH{...} |
| Bool | Null | replacement | type name | HASH{...} |
| Null | Null | replacement | type name | HASH{...} |

### BlobStatus Transitions

| From | To Pending | To DurablyStored | To Published | To Failed |
|------|------------|-----------------|--------------|-----------|
| Pending | - | ✓ | ✗ | ✓ |
| DurablyStored | ✗ | - | ✓ | ✗ |
| Published | ✗ | ✗ | - | ✗ |
| Failed | ✗ | ✗ | ✗ | - |

### DekStore Operations

| Operation | Success | NotFound | AlreadyExists | Retired |
|-----------|---------|----------|--------------|---------|
| generate | ✓ | N/A | ✓ | N/A |
| retrieve | ✓ | ✓ | N/A | ✓ |
| rotate | ✓ | ✓ | N/A | N/A |
| retire | ✓ | ✓ | N/A | N/A |

## Open Questions

1. Should retire_dek be idempotent (success if already retired)?
2. Is there a maximum DEK rotation history to retain for audit?
3. Should BlobRef content_hash length be capped?
4. Should OutputRef inline data be compressed before storing?
