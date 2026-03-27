# Red Queen Adversarial Report: wtf-types

**Target**: 14 semantic newtypes in `crates/wtf-types/src/types.rs`
**Contract**: `.beads/wtf-acb/contract.md`
**Date**: 2026-03-27
**Tests generated**: 87 (Section 5: red_queen module)
**Total test suite**: 388 passing, 0 failing

## Verdict: CROWN DEFENDED (with observations)

No invariant violations found. The implementation faithfully enforces all contract invariants. Two observations warrant documentation but are not defects.

---

## Tests Generated (Section 5: `mod red_queen`)

### 5.1 Serde Corruption (17 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_serde_string_type_rejects_unquoted_number` (x6) | Serde type mismatch | PASS |
| `rq_serde_integer_type_rejects_string` (x5) | Serde type mismatch | PASS |
| `rq_serde_rejects_null` (x3) | Serde null injection | PASS |
| `rq_serde_rejects_array` (x2) | Serde type mismatch | PASS |
| `rq_serde_rejects_object` | Serde type mismatch | PASS |
| `rq_serde_rejects_boolean` | Serde type mismatch | PASS |
| `rq_serde_rejects_negative` | Serde negative for NonZero | PASS |
| `rq_serde_rejects_float` | Serde float for integer | PASS |
| `rq_serde_rejects_empty_string_for_seq_num` | Serde empty string | PASS |
| `rq_serde_json_u64_max_accepted` | Serde boundary | PASS |
| `rq_serde_json_zero_accepted_for_duration` | Serde zero for u64 | PASS |

### 5.2 Unicode Edge Cases for WorkflowName/NodeName (10 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_workflow_name_rejects_emoji` | Unicode injection | PASS |
| `rq_node_name_rejects_emoji` | Unicode injection | PASS |
| `rq_workflow_name_rejects_zero_width_space` | Invisible Unicode | PASS |
| `rq_workflow_name_rejects_zero_width_joiner` | Invisible Unicode | PASS |
| `rq_workflow_name_rejects_right_to_left_mark` | Unicode directionality | PASS |
| `rq_workflow_name_rejects_fullwidth_digit` | Unicode digit | PASS |
| `rq_node_name_rejects_null_byte` | Control character | PASS |
| `rq_workflow_name_rejects_tab` | Control character | PASS |
| `rq_workflow_name_rejects_newline` | Control character | PASS |
| `rq_workflow_name_rejects_carriage_return` | Control character | PASS |

### 5.3 InstanceId Crockford Base32 (4 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_instance_id_accepts_lowercase_ulid` | Case insensitivity | PASS (finding) |
| `rq_instance_id_accepts_mixed_case_ulid` | Case insensitivity | PASS (finding) |
| `rq_instance_id_preserves_original_case_in_display` | Canonical form | PASS (finding) |
| `rq_instance_id_rejects_25_chars` | Length boundary | PASS |
| `rq_instance_id_rejects_27_chars` | Length boundary | PASS |

### 5.4 BinaryHash Edge Cases (5 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_binary_hash_rejects_single_char` | Odd + below min | PASS |
| `rq_binary_hash_rejects_7_chars_odd_length` | Odd length | PASS |
| `rq_binary_hash_rejects_2_chars_below_minimum` | Below min 8 | PASS |
| `rq_binary_hash_rejects_4_chars_below_minimum` | Below min 8 | PASS |
| `rq_binary_hash_rejects_6_chars_even_below_minimum` | Even, below min | PASS |

### 5.5 Integer Type Edge Cases (7 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_empty_string_returns_not_an_integer` (x8 types) | Empty input | PASS |
| `rq_negative_zero_rejected` (x5 NonZero types) | Negative zero | PASS |
| `rq_plus_prefix_accepted` | Plus sign prefix | PASS (finding) |
| `rq_very_long_integer_string` | Leading zeros | PASS |
| `rq_whitespace_only_rejected` | Whitespace input | PASS |
| `rq_scientific_notation_rejected` | Float notation | PASS |

### 5.6 Opaque String Types (TimerId/IdempotencyKey) (4 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_timer_id_accepts_null_byte_as_opaque` | Control chars | PASS |
| `rq_timer_id_accepts_newlines_as_opaque` | Control chars | PASS |
| `rq_idempotency_key_accepts_null_byte_as_opaque` | Control chars | PASS |
| `rq_timer_id_null_byte_serde_round_trip` | Serde null byte | PASS |

### 5.7 Boundary: Multi-byte Characters (3 tests)
| Test | Dimension | Result |
|------|-----------|--------|
| `rq_timer_id_rejects_257_ascii_chars` | Char count boundary | PASS |
| `rq_timer_id_accepts_256_multi_byte_chars` | Multi-byte char count | PASS |
| `rq_idempotency_key_rejects_1025_ascii_chars` | Char count boundary | PASS |

### 5.8-5.20 Structural Invariants (37 tests)
Type system verification: Copy trait, Debug transparency, error type_name correctness, TryFrom<u64> rejection of zero, From<T> direction correctness, Ord consistency, as_str lifetime, Ord absence on string types.

---

## Findings

### Finding 1: InstanceId Case Preservation (OBSERVATION)
**Severity**: OBSERVATION
**Dimension**: `invariant-case-normalization`

The `ulid::Ulid::from_string()` accepts lowercase and mixed-case input without normalizing. The inner string preserves whatever case was provided.

**Impact**: Two InstanceIds representing the same ULID but with different case are NOT equal (`PartialEq` compares inner strings per I-6). This means:
```
InstanceId("01h5...") != InstanceId("01H5...")
```

**Contract alignment**: This is correct per I-6 and I-12. I-12 explicitly states Crockford Base32 is case-insensitive. I-6 says equality is based on inner value.

**Recommendation**: Consider normalizing to uppercase in `parse()` to ensure canonical representation, which would prevent semantic duplicates. This is NOT a contract violation but a defensive hardening opportunity.

### Finding 2: Plus Sign Prefix Accepted (OBSERVATION)
**Severity**: OBSERVATION
**Dimension**: `invariant-prefix-rejection`

Rust's `u64::from_str("+42")` returns `Ok(42)`. Since all integer newtypes delegate to `u64::from_str`, they silently accept the `+` prefix.

**Impact**: `SequenceNumber::parse("+42")` succeeds. This is technically valid but could mask bugs where a caller accidentally prepends `+`.

**Contract alignment**: P-10 explicitly forbids the negative sign (`-`). P-8 forbids hex/octal/binary prefixes. The positive sign (`+`) is not mentioned. This is a minor contract gap.

**Recommendation**: Add P-11 to the contract: "Positive sign prefix (`+`) is NOT supported." Then reject it in `parse_u64_str` by checking for leading `+` before calling `u64::from_str`.

---

## Dimensions Probed

| Dimension | Tests | Survivors | Fitness |
|-----------|-------|-----------|---------|
| `serde-type-mismatch` | 17 | 0 | 0.000 (exhausted) |
| `unicode-injection` | 10 | 0 | 0.000 (exhausted) |
| `crockford-case` | 5 | 0 | 0.000 (exhausted) |
| `binary-hash-boundary` | 5 | 0 | 0.000 (exhausted) |
| `integer-edge-cases` | 7 | 0 | 0.000 (exhausted) |
| `opaque-control-chars` | 4 | 0 | 0.000 (exhausted) |
| `multi-byte-boundary` | 3 | 0 | 0.000 (exhausted) |
| `structural-invariants` | 37 | 0 | 0.000 (exhausted) |

**Total**: 88 tests, 0 survivors. All dimensions exhausted.

---

## Invariant Coverage Matrix

| Contract Invariant | Tested | Status |
|--------------------|--------|--------|
| I-1: No Default | Yes (compile-time) | PASS |
| I-2: Immutability | Yes (no &mut accessors) | PASS |
| I-3: Round-trip | Yes (14 Display round-trips + proptests) | PASS |
| I-4: No public inner field | Yes (`pub(crate)` verified) | PASS |
| I-5: Debug transparency | Yes | PASS |
| I-6: Hash/Eq consistency | Yes (proptest) | PASS |
| I-7: Clone is shallow | Yes (proptest) | PASS |
| I-10-I-12: InstanceId | Yes (ULID, 26 chars, Crockford) | PASS |
| I-13-I-16: WorkflowName | Yes (chars, length, boundaries) | PASS |
| I-17-I-20: NodeName | Yes (chars, length, boundaries) | PASS |
| I-21-I-24: BinaryHash | Yes (hex, even, min 8) | PASS |
| I-25-I-27: SequenceNumber | Yes (NonZero, range) | PASS |
| I-28-I-30: EventVersion | Yes (NonZero, range) | PASS |
| I-31-I-33: AttemptNumber | Yes (NonZero, range) | PASS |
| I-34-I-35: TimerId | Yes (non-empty, max 256 chars) | PASS |
| I-36-I-37: IdempotencyKey | Yes (non-empty, max 1024 chars) | PASS |
| I-38-I-40: TimeoutMs | Yes (NonZero, range) | PASS |
| I-41-I-43: DurationMs | Yes (u64, zero valid) | PASS |
| I-44-I-46: TimestampMs | Yes (u64, zero valid) | PASS |
| I-47-I-50: FireAtMs | Yes (u64, zero valid, no past check) | PASS |
| I-51-I-53: MaxAttempts | Yes (NonZero, range) | PASS |
| PO-11: Serialize matches Display | Yes (14 types) | PASS |
| PO-12: Deserialize routes through parse | Yes (serde rejection tests) | PASS |
| NG-4: No From<primitive> bypass | Yes (TryFrom verified) | PASS |
| NG-7: No Ord for string types | Yes (compile-time) | PASS |
| NG-13: No Copy on string types | Yes (compile-time) | PASS |
