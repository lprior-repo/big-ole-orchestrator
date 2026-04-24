# Test Plan: Blob Publication (ADR-040)

## Summary
- Behaviors identified: 42
- Trophy allocation: 28 unit / 14 integration / 6 proptest (Total 48 tests)
- Proptest invariants: 6
- Fuzz targets: 2
- Kani harnesses: 2
- Target Mutation Kill Rate: ≥90%

## 1. Behavior Inventory

### BlobRef Construction & Invariants (9)
1. `BlobRef::new` creates a reference with valid `blob_id` (26-char ULID), `size_bytes > 0`, and valid lowercase hex `content_hash`.
2. `BlobRef::new` rejects empty `blob_id` with `ParseError::Empty`.
3. `BlobRef::new` rejects `blob_id` that is not a valid ULID with `ParseError::InvalidFormat`.
4. `BlobRef::new` rejects `blob_id` with wrong length (not 26 chars) with `ParseError::InvalidFormat`.
5. `BlobRef::new` rejects `size_bytes == 0` with `ParseError::ZeroValue`.
6. `BlobRef::new` rejects empty `content_hash` with `ParseError::Empty`.
7. `BlobRef::new` rejects non-lowercase-hex `content_hash` with `ParseError::InvalidCharacters`.
8. `BlobRef::new` rejects odd-length `content_hash` with `ParseError::InvalidFormat`.
9. `BlobRef::new` rejects `content_hash` shorter than 8 chars with `ParseError::InvalidFormat`.

### BlobRef Accessors (3)
10. `BlobRef::blob_id()` returns the stored blob_id string.
11. `BlobRef::size_bytes()` returns the stored size.
12. `BlobRef::content_hash()` returns the stored hash string.

### BlobStatus State Machine (8)
13. `BlobStatus::Pending` is the initial state for new blobs.
14. `BlobStatus::Pending` can transition to `DurablyStored`.
15. `BlobStatus::Pending` can transition to `Failed`.
16. `BlobStatus::Pending` cannot skip to `Published` (invalid transition).
17. `BlobStatus::Pending` cannot transition to itself.
18. `BlobStatus::DurablyStored` can transition to `Published`.
19. `BlobStatus::DurablyStored` cannot revert to `Pending`.
20. `BlobStatus::DurablyStored` cannot transition to `Failed`.
21. `BlobStatus::Published` is terminal — cannot transition to any other state.
22. `BlobStatus::Failed` is terminal — cannot transition to any other state.

### OutputRef Dual Representation (6)
23. `OutputRef::inline(data)` succeeds when `data.len() <= INLINED_MAX_BYTES` (4096).
24. `OutputRef::inline(data)` rejects when `data.len() > INLINED_MAX_BYTES` with `ParseError::ExceedsMaxLength`.
25. `OutputRef::inline(vec![])` (empty) is valid.
26. `OutputRef::blob_ref(blob)` creates a `BlobRef` variant.
27. `OutputRef::is_inline()` returns `true` for `Inline` variant, `false` for `BlobRef`.
28. `OutputRef::is_blob_ref()` returns `true` for `BlobRef` variant, `false` for `Inline`.
29. `OutputRef::as_inline()` returns `Some(&[u8])` for `Inline`, `None` for `BlobRef`.
30. `OutputRef::as_blob_ref()` returns `Some(&BlobRef)` for `BlobRef`, `None` for `Inline`.
31. `OutputRef::classify(data)` is equivalent to `OutputRef::inline(data)`.

### OutputPolicy Failure Semantics (6)
32. `OutputPolicy::Required.blob_failure_action(Failed)` returns `BlockStep`.
33. `OutputPolicy::Optional.blob_failure_action(Failed)` returns `CompleteWithInline`.
34. `OutputPolicy::Required` blocks step completion for any non-`Failed` status.
35. `OutputPolicy::Optional` blocks step completion for any non-`Failed` status.
36. `OutputPolicy::Optional.permits_completion_on_blob_failure()` returns `true`.
37. `OutputPolicy::Required.permits_completion_on_blob_failure()` returns `false`.
38. `OutputPolicy::Required.is_required_for_replay()` returns `true`.
39. `OutputPolicy::Optional.is_required_for_replay()` returns `false`.
40. ADR-040 §3 Invariant: Replay never requires an optional blob (verified by failure action).

### BlobRecord Lifecycle (5)
41. `BlobRecord::new` creates a record with `status = Pending`, `reference_count >= 1`, `created_at_ms > 0`.
42. `BlobRecord::new` rejects `reference_count == 0` with `InvalidArgument`.
43. `BlobRecord::new` rejects `created_at_ms == 0` with `InvalidArgument`.
44. `BlobRecord::with_status` allows explicit status on construction.
45. `BlobRecord::can_transition_to(target)` returns true only for valid transitions per ADR-040 §2.
46. `BlobRecord::is_expired(now_ms)` returns `true` when `now_ms >= expires_at_ms`.
47. `BlobRecord::is_expired(now_ms)` returns `false` when `expires_at_ms = None`.
48. `BlobRecord::increment_ref_count()` saturates at `u64::MAX`.
49. `BlobRecord::decrement_ref_count()` saturates at `0`.

### ContentAddress (4)
50. `ContentAddress::new` accepts exactly 64 lowercase hex characters.
51. `ContentAddress::new` rejects uppercase, wrong length, non-hex.
52. `ContentAddress::from_bytes(bytes)` produces correct hex string via SHA-256 digest.
53. `ContentAddress::as_bytes()` roundtrips correctly.

### BlobStore Trait & GC (4)
54. `BlobStore::store(data)` returns `ContentAddress` computed from SHA-256.
55. `BlobStore::store` returns `DuplicateContent` if content already exists.
56. `BlobStore::increment_ref_count` increases ref_count, saturating at MAX.
57. `BlobStore::decrement_ref_count` decreases ref_count, saturating at 0.
58. `BlobStore::list_gc_candidates` returns blobs where `ref_count == 0` AND `is_expired(now_ms)`.
59. `BlobStore::run_gc` collects candidates, returns count of collected blobs.
60. `BlobStore::run_gc` returns `GcCycleInProgress` if already running.
61. `BlobStore::contains(addr)` returns `true` if blob exists.
62. `BlobStore::retrieve(addr)` returns `ContentNotFound` for unknown addresses.
63. `BlobStore::get_metadata(addr)` returns blob metadata or `ContentNotFound`.

### Publication Protocol (ADR-040 §2) (4)
64. Engine may only publish `output_ref` after blob is `DurablyStored` or staged with atomic visibility.
65. If neither durable-write nor atomic-staged is possible, Engine must NOT publish the ref.
66. If blob persistence fails before publication and output is `Required`: step stays incomplete.
67. If blob persistence fails and output is `Optional`: step may complete with `routing_projection` only.

## 2. Trophy Allocation

*   **Unit Tests (28)**: Cover all pure types (`BlobRef`, `BlobStatus`, `OutputRef`, `OutputPolicy`, `BlobFailureAction`, `BlobRecord`, `ContentAddress`), their construction errors, state transitions, accessor correctness, serde roundtrips, and display formatting.
*   **Integration Tests (14)**: Cover `BlobStore` trait implementation lifecycle (store → retrieve → ref_count → GC), concurrent deduplication, concurrent ref_count operations, streaming upload/download, and the publication protocol ordering (store-before-publish invariant).
*   **Proptest (6)**: Property-based testing for `ContentAddress` byte roundtrip, `BlobRecord` ref_count saturation, expiry monotonicity, `PackIndexEntry` encoding roundtrip, and `BlobRef` construction validation.
*   **Fuzz (2)**: Content address from arbitrary bytes, blob store streaming upload with chunk boundaries.
*   **Kani (2)**: `BlobStatus::can_transition_to` state machine invariant, `OutputPolicy::blob_failure_action` correctness.

## 3. BDD Scenarios

### Behavior: BlobRef Valid Construction
Given: Valid 26-char ULID `blob_id`, `size_bytes = 1024`, valid 64-char lowercase hex `content_hash`
When: `BlobRef::new` is called
Then: Returns `Ok(BlobRef)` with all fields set correctly

### Behavior: BlobRef Rejects Empty blob_id
Given: Empty string as `blob_id`, valid `size_bytes` and `content_hash`
When: `BlobRef::new` is called
Then: Returns `Err(ParseError::Empty { type_name: "BlobRef.blob_id" })`

### Behavior: BlobRef Rejects Invalid ULID
Given: `blob_id = "not-a-ulid"`, valid size and hash
When: `BlobRef::new` is called
Then: Returns `Err(ParseError::InvalidFormat { type_name: "BlobRef.blob_id", reason: "not a valid ULID" })`

### Behavior: BlobRef Rejects Zero Size
Given: Valid ULID `blob_id`, `size_bytes = 0`, valid `content_hash`
When: `BlobRef::new` is called
Then: Returns `Err(ParseError::ZeroValue { type_name: "BlobRef.size_bytes" })`

### Behavior: BlobRef Rejects Non-Hex Content Hash
Given: Valid ULID and size, `content_hash = "ghijklmnopqrstuv"`
When: `BlobRef::new` is called
Then: Returns `Err(ParseError::InvalidCharacters { type_name: "BlobRef.content_hash", invalid_chars: "ghijklmnopqrstuv" })`

### Behavior: BlobStatus Pending to DurablyStored Transition
Given: A blob with `status = Pending`
When: `can_transition_to(DurablyStored)` is called
Then: Returns `true`

### Behavior: BlobStatus Pending Cannot Skip to Published
Given: A blob with `status = Pending`
When: `can_transition_to(Published)` is called
Then: Returns `false`

### Behavior: BlobStatus Published Is Terminal
Given: A blob with `status = Published`
When: `can_transition_to` is called with any variant
Then: Returns `false` for all variants

### Behavior: OutputRef Inline Within Max Bytes
Given: `data` with length exactly `INLINED_MAX_BYTES` (4096)
When: `OutputRef::inline(data)` is called
Then: Returns `Ok(OutputRef::Inline(data))`

### Behavior: OutputRef Inline Exceeds Max
Given: `data` with length `INLINED_MAX_BYTES + 1`
When: `OutputRef::inline(data)` is called
Then: Returns `Err(ParseError::ExceedsMaxLength { type_name: "OutputRef.inline", max: 4096, actual: 4097 })`

### Behavior: OutputRef BlobRef Variant
Given: A valid `BlobRef`
When: `OutputRef::blob_ref(blob)` is called
Then: Returns `OutputRef::BlobRef(blob)`, `is_blob_ref() == true`, `as_blob_ref() == Some(&blob)`

### Behavior: OutputPolicy Required Blocks on Blob Failure
Given: `OutputPolicy::Required` and `BlobStatus::Failed`
When: `blob_failure_action(Failed)` is called
Then: Returns `BlobFailureAction::BlockStep`

### Behavior: OutputPolicy Optional Allows Inline on Blob Failure
Given: `OutputPolicy::Optional` and `BlobStatus::Failed`
When: `blob_failure_action(Failed)` is called
Then: Returns `BlobFailureAction::CompleteWithInline`

### Behavior: OutputPolicy Non-Failed Status Always Blocks
Given: Each non-`Failed` `BlobStatus` (`Pending`, `DurablyStored`, `Published`) and both `OutputPolicy` variants
When: `blob_failure_action(status)` is called
Then: Returns `BlobFailureAction::BlockStep` regardless of policy

### Behavior: BlobRecord New Rejects Zero Reference Count
Given: `reference_count = 0`, valid other fields
When: `BlobRecord::new` is called
Then: Returns `Err(BlobStoreError::InvalidArgument { reason: "reference_count must be non-zero" })`

### Behavior: BlobRecord Expires At Boundary
Given: `BlobRecord` with `expires_at_ms = Some(1500)`
When: `is_expired(1499)` is called
Then: Returns `false`
When: `is_expired(1500)` is called
Then: Returns `true`

### Behavior: BlobRecord Never Expires Without TTL
Given: `BlobRecord` with `expires_at_ms = None`
When: `is_expired(u64::MAX)` is called
Then: Returns `false`

### Behavior: BlobRecord Ref Count Increment Saturates
Given: `BlobRecord` with `reference_count = u64::MAX`
When: `increment_ref_count()` is called
Then: Returns `u64::MAX`

### Behavior: BlobRecord Ref Count Decrement Saturates
Given: `BlobRecord` with `reference_count = 1`
When: `decrement_ref_count()` is called
Then: Returns `0`

### Behavior: ContentAddress Valid SHA-256
Given: 64-char lowercase hex string
When: `ContentAddress::new` is called
Then: Returns `Ok(ContentAddress)`

### Behavior: ContentAddress Rejects Uppercase
Given: String with uppercase hex characters
When: `ContentAddress::new` is called
Then: Returns `Err(BlobStoreError::InvalidArgument { reason: "content address must be lowercase hex" })`

### Behavior: ContentAddress Rejects Wrong Length
Given: String that is not exactly 64 characters
When: `ContentAddress::new` is called
Then: Returns `Err(BlobStoreError::InvalidArgument { reason: "content address must be 64 chars" })`

### Behavior: Blob Store Deduplication
Given: Store already contains content with SHA-256 `X`
When: `store(data)` is called where `sha256(data) = X`
Then: Returns `Err(BlobStoreError::DuplicateContent { content_addr: X })`

### Behavior: Blob Store GC Candidates
Given: Blobs A (ref_count=0, expired), B (ref_count=1, expired), C (ref_count=0, not expired)
When: `list_gc_candidates(now)` is called
Then: Returns only A

### Behavior: Blob Store GC Only Collects Expired Unreferenced
Given: Blob with ref_count=0 but expires_at_ms=None (no TTL)
When: `list_gc_candidates(now)` is called
Then: Returns empty (blob never expires, not eligible for GC without TTL)

### Behavior: Blob Store GC Rejects Concurrent Run
Given: GC is already in progress
When: `run_gc(now)` is called
Then: Returns `Err(BlobStoreError::GcCycleInProgress)`

### Behavior: Publication Rule — OutputRef Only After Durable
Given: Blob with `status = Pending`
When: Engine attempts to publish `output_ref` referencing this blob
Then: Must NOT publish (violates ADR-040 §2)

### Behavior: Publication Rule — DurablyStored Allows Publish
Given: Blob with `status = DurablyStored`
When: Engine publishes `output_ref` referencing this blob
Then: Publication is valid per ADR-040 §2

### Behavior: Optional Blob Failure Allows Inline Completion
Given: Step with `OutputPolicy::Optional` output, blob persistence fails
When: Engine processes the failure
Then: Step may complete with `routing_projection` only (no `output_ref`)

### Behavior: Required Blob Failure Blocks Step
Given: Step with `OutputPolicy::Required` output, blob persistence fails
When: Engine processes the failure
Then: Step stays incomplete (retry or fail per policy)

## 4. Proptest Invariants

### Proptest: ContentAddress Byte Roundtrip
Invariant: For any `[u8; 32]`, `ContentAddress::from_bytes(bytes).as_bytes() == bytes`.
Strategy: `any::<[u8; 32]>()`

### Proptest: ContentAddress Hex Validity
Invariant: `ContentAddress::new(hex).is_ok()` implies the result has `as_str().len() == 64` and all chars are lowercase hex.
Strategy: `hex in "[a-f0-9]{64}"`

### Proptest: BlobRecord Ref Count Saturation
Invariant: `increment_ref_count` never exceeds `u64::MAX`; `decrement_ref_count` never goes below `0`.
Strategy: Arbitrary `u64` ref_count values including `u64::MAX` and `1`.

### Proptest: BlobRecord Expiry Monotonicity
Invariant: If `expires_at_ms = Some(t)`, then `is_expired(t-1) == false` and `is_expired(t) == true` and `is_expired(t+1) == true`.
Strategy: Arbitrary `created_at`, `expires_offset`.

### Proptest: BlobStatus Transition Validity
Invariant: For all `BlobStatus` values, `can_transition_to` returns `true` only for the defined valid transitions.
Strategy: All combinations of source × target from `BlobStatus::all_variants()`.

### Proptest: OutputPolicy Failure Action Determinism
Invariant: `OutputPolicy::blob_failure_action(status)` is deterministic for all combinations of policy and status.
Strategy: All `OutputPolicy` × all `BlobStatus`.

## 5. Fuzz Targets

### Fuzz Target: ContentAddress from Arbitrary Bytes
Input type: `&[u8]` — arbitrary byte slice
Risk: Panic in `from_utf8_unchecked` or `from_str_radix` on invalid hex-like bytes.
Corpus seeds: Valid 32-byte SHA-256, all-zeros, all-0xFF, alternating patterns.

### Fuzz Target: Blob Store Streaming Upload
Input type: `Vec<u8>` with chunk boundaries — simulate streaming with various chunk sizes
Risk: SHA-256 incrementally computed correctly across chunk boundaries; no buffer overflow.
Corpus seeds: Small blob (< 1KB), medium (4KB = INLINED_MAX_BYTES), large (> 1MB), empty.

## 6. Kani Harnesses

### Kani Harness: BlobStatus State Machine Transitions
Property: For any `BlobStatus` source and target, `can_transition_to(target)` is `true` if and only if the transition is valid per ADR-040 §2 state machine.
Bound: Depth 3.
Rationale: State machine is the core correctness invariant — prevents illegal transitions.

### Kani Harness: OutputPolicy Failure Action Correctness
Property: For all `OutputPolicy` × `BlobStatus` combinations, `blob_failure_action` returns the correct `BlobFailureAction` per ADR-040 §3 rules.
Bound: Depth 4.
Rationale: Failure semantics are critical for replay correctness.

## 7. Mutation Checkpoints

Critical mutations to survive:
- Changing `Pending → DurablyStored` to `Pending → Published` must be caught by invalid transition test.
- Changing `OutputPolicy::Optional` failure action from `CompleteWithInline` to `BlockStep` must be caught by failure action test.
- Changing `size_bytes == 0` check to `size_bytes < 0` (impossible but illustrative) must be caught by zero-size test.
- Changing `content_hash.len() < 8` to `< 7` must be caught by 7-char hash rejection test.
- Changing `reference_count == 0` check to allow zero must be caught by `BlobRecord::new` rejection test.
- Changing `is_expired` comparison from `>=` to `>` must be caught by boundary test at `now_ms == expires_at_ms`.
- Removing deduplication check in `store` must be caught by duplicate store integration test.
- Changing `can_transition_to` to allow `DurablyStored → Failed` must be caught by invalid transition test.

Threshold: 90% mutation kill rate minimum.
Coverage: 90% line coverage minimum.

## 8. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| BlobRef new: valid | ULID, >0 size, valid hash | Ok(BlobRef) | Unit |
| BlobRef new: empty blob_id | empty string | Err(Empty) | Unit |
| BlobRef new: invalid ULID | "not-a-ulid" | Err(InvalidFormat) | Unit |
| BlobRef new: wrong length id | 10-char string | Err(InvalidFormat) | Unit |
| BlobRef new: size=0 | 0 size_bytes | Err(ZeroValue) | Unit |
| BlobRef new: empty hash | "" | Err(Empty) | Unit |
| BlobRef new: non-hex hash | "ghijklmnop" | Err(InvalidChars) | Unit |
| BlobRef new: odd-length hash | "abcde" | Err(InvalidFormat) | Unit |
| BlobRef new: short hash | "ab" | Err(InvalidFormat) | Unit |
| BlobStatus: Pending→DurablyStored | Pending status | true | Unit |
| BlobStatus: Pending→Failed | Pending status | true | Unit |
| BlobStatus: Pending→Published | Pending status | false | Unit |
| BlobStatus: DurablyStored→Published | DurablyStored status | true | Unit |
| BlobStatus: DurablyStored→Pending | DurablyStored status | false | Unit |
| BlobStatus: DurablyStored→Failed | DurablyStored status | false | Unit |
| BlobStatus: Published→* | Published status | false for all | Unit |
| BlobStatus: Failed→* | Failed status | false for all | Unit |
| OutputRef: inline within max | 4096 bytes | Ok(Inline) | Unit |
| OutputRef: inline at max | 4096 bytes | Ok(Inline) | Unit |
| OutputRef: inline exceeds max | 4097 bytes | Err(ExceedsMaxLength) | Unit |
| OutputRef: empty inline | vec![] | Ok(Inline) | Unit |
| OutputRef: blob_ref from valid | BlobRef | Ok(BlobRef) | Unit |
| OutputPolicy: Required+Failed | Required, Failed | BlockStep | Unit |
| OutputPolicy: Optional+Failed | Optional, Failed | CompleteWithInline | Unit |
| OutputPolicy: Required+Pending | Required, Pending | BlockStep | Unit |
| OutputPolicy: Optional+DurablyStored | Optional, DurablyStored | BlockStep | Unit |
| BlobRecord: new valid | ref_count≥1, created>0 | Ok(Pending) | Unit |
| BlobRecord: ref_count=0 | 0 | Err(InvalidArg) | Unit |
| BlobRecord: created_at=0 | 0 | Err(InvalidArg) | Unit |
| BlobRecord: is_expired boundary | expires=1500 | false at 1499, true at 1500 | Unit |
| BlobRecord: no expiry | None | always false | Unit |
| BlobRecord: ref_count increment saturates | u64::MAX | u64::MAX | Unit |
| BlobRecord: ref_count decrement saturates | 1 | 0 | Unit |
| ContentAddress: valid 64-char hex | "[a-f0-9]{64}" | Ok | Unit |
| ContentAddress: uppercase | "[A-F0-9]{64}" | Err | Unit |
| ContentAddress: wrong length | "[a-f0-9]{32}" | Err | Unit |
| ContentAddress: non-hex | "[g-z]{64}" | Err | Unit |
| ContentAddress: byte roundtrip | [u8; 32] | original bytes | Unit |
| BlobStore: store+retrieve | new data | correct data | Integration |
| BlobStore: duplicate store | same data twice | DuplicateContent | Integration |
| BlobStore: contains true/false | stored vs unstored | true / false | Integration |
| BlobStore: increment ref_count | stored blob | count+1 | Integration |
| BlobStore: decrement ref_count | stored blob | count-1 | Integration |
| BlobStore: GC candidates | mixed ref_count/expiry | only eligible | Integration |
| BlobStore: GC concurrent | two simultaneous calls | GcCycleInProgress | Integration |
| BlobStore: get_metadata | stored blob | correct BlobRecord | Integration |
| BlobStore: streaming store | chunked data | correct content_addr | Integration |
| BlobStore: streaming retrieve | stored blob, sink | correct data | Integration |
| ContentAddress: from_bytes SHA-256 | sha256 digest bytes | correct hex | Proptest |
| BlobStatus: all variants | each variant | 4 total | Unit |
| OutputRef: classify small | 100 bytes | Inline | Unit |
| OutputRef: classify huge | 1MB | Err(ExceedsMaxLength) | Proptest |
| Publication: pending blob not publishable | Pending status | must not publish | Integration |
| Publication: durably stored blob publishable | DurablyStored | may publish | Integration |
| ADR-040: optional failure inline | Optional+Failed | CompleteWithInline | Unit |
| ADR-040: required failure blocks | Required+Failed | BlockStep | Unit |

## 9. Contract Invariant Coverage (ADR-040)

| Contract Invariant | Test Coverage |
|---|---|
| INV-1: BlobStatus state machine (Pending→DurablyStored→Published, Pending→Failed) | Unit: invalid transition tests, proptest |
| INV-2: output_ref only published after durable blob | Integration: publication order test |
| INV-3: Optional blob failure → inline completion | Unit: OutputPolicy failure action tests |
| INV-4: Required blob failure → step blocked | Unit: OutputPolicy failure action tests |
| INV-5: Replay never requires optional blob | Unit: `is_required_for_replay` + failure action tests |
| INV-6: BlobRef invariant (ULID, size>0, hex hash) | Unit: all BlobRef construction error tests |
| INV-7: ContentAddress is valid SHA-256 64-char lowercase hex | Unit + Proptest: construction + byte roundtrip |
| INV-8: GC only collects ref_count=0 AND expired | Integration: list_gc_candidates + run_gc tests |
| INV-9: Ref count saturation (no overflow/underflow) | Unit + Proptest: increment/decrement saturation |
| INV-10: Streaming SHA-256 incremental correctness | Fuzz: chunk boundary testing |
| INV-11: Deduplication by content address | Integration: duplicate store test |
| INV-12: Dual representation serde preserves variant | Unit: OutputRef serde roundtrip tests |

## 10. Observability Test Coverage

| Metric | Increment Trigger | Test |
|---|---|---|
| blobs_stored | Successful `store()` | Integration: store+retrieve |
| blobs_retrieved | Successful `retrieve()` | Integration: store+retrieve |
| blobs_gc_collected | Successful `run_gc()` | Integration: GC tests |
| duplicate_rejected | Duplicate `store()` call | Integration: deduplication test |
| streaming_bytes | Chunk processed in streaming ops | Integration: streaming tests |

## 11. Implementation Gaps Noted

These gaps between ADR-040 contract and current implementation should be tracked:

1. **`BlobStoreError::NotDurablyStored` is defined but no `publish` method exists**: The ADR-040 §2 publication protocol (checking blob is durably stored before publishing output_ref) is not yet enforced by a dedicated `publish` method. The error variant exists but is not returned from any active code path.
2. **`BlobStore::publish` method is absent**: There is no `BlobStore::publish(addr: &ContentAddress)` method that transitions a blob from `DurablyStored` to `Published`, which is the core ADR-040 §2 enforcement point.
3. **No atomic storage primitive for staged+published together**: ADR-040 §2 allows "blob is staged and the same atomic storage primitive guarantees visibility of both the blob and the published ref together" — this atomic path is not yet modeled.
4. **`BlobRecord` does not track which step/output owns the reference**: The ref_count is aggregate; there is no per-output accounting to know which step would be affected if a blob reference count drops to zero.
5. **`BlobStore::store_streaming` is not implemented in the integration test harness**: The streaming upload path is declared in the trait but the in-memory test store returns `Storage { reason: "not implemented" }`.

## 12. Test File Location

Tests should be placed in:
- **Unit tests**: `crates/vo-types/src/blob.rs` (existing `#[cfg(test)] mod tests`)
- **Unit tests**: `crates/vo-storage/src/blob_store.rs` (existing `#[cfg(test)] mod tests`)
- **Integration tests**: `crates/vo-storage/tests/blob_store_integration.rs` (existing, extend)
- **Red Queen tests**: `crates/vo-storage/tests/blob_store_red_queen.rs` (existing, extend)
- **Proptest**: `crates/vo-storage/src/blob_store.rs` (existing `#[cfg(feature = "proptest")] mod proptests`)
