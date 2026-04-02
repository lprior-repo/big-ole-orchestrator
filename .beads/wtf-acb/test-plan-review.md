bead_id: wtf-acb
bead_title: wtf-types: define all semantic newtypes
phase: state-1.5
updated_at: 2026-03-27T05:45:00Z
reviewer: test-inquisitor (Mode 1 — Plan Inquisition, Re-review)

# Test Plan Review: wtf-types: define all semantic newtypes (Re-review)

## VERDICT: APPROVED

---

### Previous Review Verification

Every finding from the previous REJECTED review has been verified against the updated test-plan.md.

#### LETHAL-1 (test density < 5x) — RESOLVED

| Metric | Previous | Current | Threshold |
|---|---|---|---|
| Public functions | 41 | 41 | — |
| 5x threshold | 205 | 205 | — |
| BDD scenarios (Section 3) | 142 | 183 | — |
| Proptest invariants (Section 4) | 26 | 26 | — |
| **Total** | **168** | **209** | **≥205** |

209 ≥ 205. Density threshold met. ✓

The summary now correctly states "22 categories, 209 individual test scenarios" with "183 (143 original + 40 promoted from Section 8 matrix)" BDD scenarios plus 26 proptest invariants.

#### MAJOR-A (NodeName BoundaryViolation reason wildcards) — RESOLVED

All 6 NodeName BoundaryViolation scenarios in Section 3.4 (lines 641, 649, 657, 665, 673, 681) now include "where reason contains 'hyphen'" or "where reason contains 'underscore'" clauses. Verified by grep. ✓

#### MAJOR-B (Integer edge case type_name wildcards) — RESOLVED

All 7 integer edge case scenarios in Section 3.18 (lines 1700, 1708, 1716, 1724, 1732, 1740, 1748) now specify `<exact_type_name>` with "asserted per rstest case: 'SequenceNumber', 'EventVersion', 'AttemptNumber', 'TimeoutMs', 'DurationMs', 'TimestampMs', 'FireAtMs', 'MaxAttempts'". No `type_name: _` wildcards remain in any BDD scenario. Verified by grep (zero hits). ✓

#### MAJOR-C (InstanceId InvalidFormat reason wildcards) — RESOLVED

Both scenarios (lines 393, 401) now include "where reason contains 'character' or 'invalid'" and "where reason contains 'ULID' or 'validation'" respectively. ✓

#### MAJOR-D (WorkflowName BoundaryViolation reason wildcards) — RESOLVED

Both scenarios (lines 509, 517) now include "where reason contains 'hyphen'" and "where reason contains 'underscore'" respectively. ✓

#### MAJOR-E (BinaryHash InvalidFormat reason wildcard) — RESOLVED

Scenario at line 805 now includes "where reason contains '8' or 'minimum'". ✓

#### MINOR-1 through MINOR-8 (missing boundary BDDs) — ALL RESOLVED

| # | Missing Boundary | Status | Evidence |
|---|---|---|---|
| MINOR-1 | WorkflowName trailing whitespace | RESOLVED | test-plan.md:561 `#### Behavior: WorkflowName rejects trailing whitespace` |
| MINOR-2 | NodeName trailing whitespace | RESOLVED | test-plan.md:725 `#### Behavior: NodeName rejects trailing whitespace` |
| MINOR-3 | BinaryHash trailing whitespace | RESOLVED | test-plan.md:833 `#### Behavior: BinaryHash rejects trailing whitespace` |
| MINOR-4 | InstanceId trailing whitespace | RESOLVED | test-plan.md:421 `#### Behavior: InstanceId rejects trailing whitespace` |
| MINOR-5 | TimeoutMs u64::MAX | RESOLVED | test-plan.md:1161 `#### Behavior: TimeoutMs accepts u64::MAX` |
| MINOR-6 | TimestampMs u64::MAX | RESOLVED | test-plan.md:1297 `#### Behavior: TimestampMs accepts u64::MAX` |
| MINOR-7 | FireAtMs u64::MAX | RESOLVED | test-plan.md:1373 `#### Behavior: FireAtMs accepts u64::MAX` |
| MINOR-8 | MaxAttempts u64::MAX | RESOLVED | test-plan.md:1465 `#### Behavior: MaxAttempts accepts u64::MAX` |

---

### Fresh Audit (6 Axes)

### Axis 1 — Contract Parity

**[PASS]** All 41 public items in `contract.md` have ≥1 BDD scenario in `test-plan.md`.

Public function inventory verified:
- 14 `parse()` methods → Sections 3.2-3.15
- 6 `as_str()` methods → Asserted in every parse() acceptance test
- 8 `as_u64()` methods → Asserted in every parse() acceptance test
- 5 `new_unchecked()` methods → Section 3.16 (10 scenarios: valid + panic per type)
- 2 `to_duration()` methods → Sections 3.11, 3.12
- 2 `to_system_time()` methods → Sections 3.13, 3.14
- 1 `now()` method → Section 3.13 (2 scenarios: parseable + approximately current)
- 1 `has_elapsed()` method → Section 3.14 (3 scenarios: <, >, ==)
- 1 `is_exhausted()` method → Section 3.15 (5 scenarios: <, max-1, ==, >, 1==1)
- 1 `From<SequenceNumber> for NonZeroU64` → Section 3.20

**[PASS]** All 8 `ParseError` variants have scenarios asserting the exact variant:
- `Empty` → 6 string type empty tests
- `InvalidCharacters` → WorkflowName/NodeName/BinaryHash invalid char tests
- `InvalidFormat` → InstanceId/BinaryHash format tests
- `ExceedsMaxLength` → 4 type boundary tests
- `BoundaryViolation` → WorkflowName/NodeName leading/trailing tests
- `NotAnInteger` → 8 integer type tests + shared edge case tests
- `ZeroValue` → 5 NonZeroU64 type tests
- `OutOfRange` → Section 3.1 Display test (variant exists but no type produces it)

### Axis 2 — Assertion Sharpness

**[PASS]** No `is_ok()` or `is_err()` assertions found in any BDD scenario. Zero grep hits.

**[PASS]** All `reason: _` wildcards in BDD Section 3 have accompanying "where reason contains 'X'" substring constraints. 21 `reason: _` instances found, all with substring checks. The 3 remaining `reason: _` hits (lines 2206-2208) are in Section 8 (Combinatorial Coverage Matrix reference table), not in the BDD specification.

**[PASS]** No `type_name: _` wildcards remain in any BDD scenario. Zero grep hits. All 7 integer edge case scenarios use `<exact_type_name>` with per-case assertions.

**[PASS]** `invalid_chars: _` wildcards (2 instances at lines 581, 821) both have constraints: "where invalid_chars is non-empty" and "where invalid_chars contains at least one uppercase letter". These are appropriate for unicode/mixed-case inputs where the exact character set is implementation-dependent.

**[PASS]** `Err(_)` wildcards (3 instances at lines 1796, 1805, 1814) are in serde rejection tests. `serde_json::Error` is not our type — the error message string is the only accessible assertion surface. This was accepted as correct in the previous review.

### Axis 3 — Trophy Allocation

**[PASS]** 209 total scenarios (183 BDD + 26 proptest) > 205 (5× 41 public functions). Ratio: 5.10x. ✓

**[PASS]** All 14 `parse()` functions have fuzz targets (Section 5.1: 14 parse targets + 14 serde targets).

**[PASS]** All 14 newtypes have proptest round-trip invariants (Section 4.1).

**[PASS]** Additional proptest invariants cover: Display consistency (4.2), Hash/Eq (4.3), Copy (4.4), PartialOrd/Ord (4.5), Serde round-trip (4.6), conversion methods (4.7).

**[PASS]** Trophy ratio justification is sound: ~68% unit / ~27% integration (serde) / ~5% static is appropriate for a pure Calc layer crate with zero I/O.

### Axis 4 — Boundary Completeness

**[PASS]** All boundaries explicitly named for every function:

| Type | Empty | Min Valid | Max Valid | One-Below-Min | One-Above-Max | Invalid Chars | Whitespace | Overflow | Special |
|---|---|---|---|---|---|---|---|---|---|
| InstanceId | ✓ | 26 chars | 26 chars | 10 chars | 29 chars | ✓ | ✓ (lead/trail) | — | ULID validation |
| WorkflowName | ✓ | 1 char | 128 chars | — | 129 chars | ✓ (space/null/unicode/ws-only) | ✓ (lead/trail) | — | Hyphen/underscore boundaries |
| NodeName | ✓ | 1 char | 128 chars | — | 129 chars | ✓ (space/null) | ✓ (lead/trail) | — | Hyphen/underscore boundaries |
| BinaryHash | ✓ | 8 chars | No max | 6 chars | — | ✓ (upper/non-hex/mixed) | ✓ (lead/trail) | — | Odd length, all-zeros |
| SequenceNumber | — | 1 | u64::MAX | 0 (ZeroValue) | — | — | ✓ | ✓ | Leading zeros, hex/octal/binary prefix, float |
| EventVersion | — | 1 | u64::MAX | 0 (ZeroValue) | — | — | ✓ | ✓ | Same shared integer edge cases |
| AttemptNumber | — | 1 | u64::MAX | 0 (ZeroValue) | — | — | ✓ | ✓ | Same |
| TimerId | ✓ | 1 char | 256 chars | — | 257 chars | — | ✓ (trailing preserved) | — | Unicode, any chars |
| IdempotencyKey | ✓ | 1 char | 1024 chars | — | 1025 chars | — | ✓ (trailing preserved) | — | Unicode, any chars |
| TimeoutMs | — | 1 | u64::MAX | 0 (ZeroValue) | — | — | ✓ | ✓ | Same shared integer edge cases |
| DurationMs | — | 0 | u64::MAX | — | — | — | ✓ | ✓ | Same |
| TimestampMs | — | 0 | u64::MAX | — | — | — | ✓ | ✓ | Same |
| FireAtMs | — | 0 | u64::MAX | — | — | — | ✓ | ✓ | Same |
| MaxAttempts | — | 1 | u64::MAX | 0 (ZeroValue) | — | — | ✓ | ✓ | Same |

**[PASS]** `MaxAttempts::is_exhausted`: Covers attempt < max, attempt == max-1, attempt == max, attempt > max, max==1 && attempt==1. ✓
**[PASS]** `FireAtMs::has_elapsed`: Covers fire_at < now, fire_at > now, fire_at == now (determinism). ✓

### Axis 5 — Mutation Survivability

**[PASS]** All critical mutations caught:

| Mutation | Catching Test(s) |
|---|---|
| Remove Empty check | 6 `*_rejects_empty_*` tests |
| Remove ZeroValue check | 5 `*_rejects_zero_*` tests |
| Swap Empty/InvalidCharacters priority | `workflow_name_rejects_leading_whitespace_*` |
| Remove ExceedsMaxLength check | 4 `*_rejects_exceeds_max_length_*` + 4 `*_accepts_exactly_N_chars_*` |
| Remove BoundaryViolation check | 12 `*_rejects_leading/trailing_hyphen/underscore_*` |
| `>` to `>=` in max-length | `*_accepts_exactly_128/256/1024_chars_*` (4 tests) |
| `<` to `<=` in min-length (BinaryHash) | `binary_hash_accepts_8_char_hex_*` |
| `>=` to `>` in is_exhausted | `max_attempts_is_exhausted_returns_true_when_attempt_equals_max` |
| `<` to `<=` in has_elapsed | `fire_at_ms_has_elapsed_returns_false_when_fire_at_is_after_now` |
| Allow uppercase in BinaryHash | `binary_hash_rejects_uppercase_hex_*` + `binary_hash_rejects_mixed_case_*` |
| Remove odd-length check | `binary_hash_rejects_odd_length_*` |
| Remove min-length check | `binary_hash_rejects_6_chars_*` + `binary_hash_rejects_too_short_*` |
| Deserialize bypasses parse() | 3 `serde_deserialize_rejects_*` scenarios (parameterized over 14 types) |
| new_unchecked doesn't panic on zero | 5 `#[should_panic] *_new_unchecked_panics_*` tests |
| to_duration wrong factor | `*_to_duration_returns_correct_duration_*` (4 scenarios) |
| Strip whitespace | 4 trailing whitespace rejection tests |
| Cap at lower than u64::MAX | 8 `*_accepts_u64_max_*` tests |
| Allow single hyphen/underscore as valid name | 4 `*_rejects_hyphen/underscore_only_*` tests |

No uncaught mutations identified.

### Axis 6 — Holzmann Plan Audit

**[PASS]** Rule 5 (State Assumptions): Every BDD scenario has explicit `Given:` block. Proptest invariants state strategies. `TimestampMs::now()` names system clock dependency.

**[PASS]** Rule 2 (Bound Every Loop): No loops in any planned test. Parameterization via `rstest` (explicit cases). Property testing via `proptest` (strategies with bounds).

**[PASS]** Rule 8 (Surface Side Effects): Only `TimestampMs::now()` has a side effect (system clock), explicitly named in Given block.

**[PASS]** Rule 4 (One Function, One Job): Each BDD scenario tests one behavior. Test names follow `fn type_behavior_when_condition()`.

**[PASS]** Rule 10 (Warnings Are Errors): Plan specifies `cargo clippy --tests --all-features -- -D warnings`.

---

### Summary Table

| Check | Result |
|---|---|
| Contract parity (pub fn coverage) | **PASS** — 209 ≥ 205 (5.10x) |
| Error variant completeness | **PASS** — All 8 variants tested |
| Assertion sharpness | **PASS** — No is_ok/is_err, all wildcards have constraints |
| Trophy allocation | **PASS** — 5.10x ratio, 14 fuzz targets, 26 proptest invariants |
| Boundary completeness | **PASS** — All boundaries named for all 14 types |
| Mutation survivability | **PASS** — All critical mutations caught |
| Holzmann rules | **PASS** — Rules 2, 4, 5, 8, 10 all satisfied |

---

### LETHAL FINDINGS

None.

### MAJOR FINDINGS (0)

None.

### MINOR FINDINGS (2/5 threshold — does not block APPROVAL)

- **MINOR-1** — test-plan.md:289: Trophy allocation arithmetic says "228 scenarios (209 distinct BDD + 26 proptest invariants)" but 209 + 26 = 235, not 228. The 228 is the inflated parameterized count (167+58+3), and 209 already includes the 26 proptest invariants (183 BDD + 26 proptest = 209). The parenthetical is misleading but the coverage numbers are correct.
- **MINOR-2** — test-plan.md:2206-2231: Section 8 Combinatorial Coverage Matrix still uses bare `reason: _` in several cells (InstanceId lines 2206-2208, WorkflowName lines 2226-2231) without substring checks. The corresponding BDD scenarios in Section 3 all have concrete assertions, so this is a documentation inconsistency in the reference table, not a test coverage gap.

---

### Previous Review Findings — Final Disposition

| Finding | Status | Verification |
|---|---|---|
| LETHAL-1: Density < 5x | **RESOLVED** | 209 ≥ 205 ✓ |
| MAJOR-A: NodeName reason wildcards | **RESOLVED** | All 6 have substring checks ✓ |
| MAJOR-B: Integer type_name wildcards | **RESOLVED** | All 7 use `<exact_type_name>` ✓ |
| MAJOR-C: InstanceId reason wildcards | **RESOLVED** | Both have substring checks ✓ |
| MAJOR-D: WorkflowName reason wildcards | **RESOLVED** | Both have substring checks ✓ |
| MAJOR-E: BinaryHash reason wildcard | **RESOLVED** | Has substring check ✓ |
| MINOR-1: WorkflowName trailing whitespace | **RESOLVED** | BDD at line 561 ✓ |
| MINOR-2: NodeName trailing whitespace | **RESOLVED** | BDD at line 725 ✓ |
| MINOR-3: BinaryHash trailing whitespace | **RESOLVED** | BDD at line 833 ✓ |
| MINOR-4: InstanceId trailing whitespace | **RESOLVED** | BDD at line 421 ✓ |
| MINOR-5: TimeoutMs u64::MAX | **RESOLVED** | BDD at line 1161 ✓ |
| MINOR-6: TimestampMs u64::MAX | **RESOLVED** | BDD at line 1297 ✓ |
| MINOR-7: FireAtMs u64::MAX | **RESOLVED** | BDD at line 1373 ✓ |
| MINOR-8: MaxAttempts u64::MAX | **RESOLVED** | BDD at line 1465 ✓ |

### MANDATE

None. Plan is approved for implementation. Proceed to Mode 2 (Suite Inquisition) after implementation is complete.
