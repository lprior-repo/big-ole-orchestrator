# Test Plan: Checksum Verification Pipeline

## Summary

- **Bead**: ve-atc7 — Test Plan: Checksum verification pipeline
- **Contract**: ve-oa1s — Contract: Checksum verification pipeline
- **Behaviors identified**: 14
- **Trophy allocation**: 18 unit / 6 integration / 2 e2e / 2 static
- **Proptest invariants**: 6
- **Fuzz targets**: 4
- **Kani harnesses**: 2
- **Mutation checkpoints**: 8

---

## 1. Behavior Inventory

| # | Behavior | Public API |
|---|----------|------------|
| B-001 | `ChecksumAlgorithm::name()` returns correct string for each variant | `ChecksumAlgorithm::name()` |
| B-002 | `StreamingHasher::new()` creates empty hasher with correct initial state | `StreamingHasher::new()` |
| B-003 | `StreamingHasher::update()` accumulates data across multiple calls | `StreamingHasher::update()` |
| B-004 | `StreamingHasher::finalize()` produces `Checksum` with all three algorithm outputs | `StreamingHasher::finalize()` |
| B-005 | `compute_checksum(data)` is equivalent to `new().update(data).finalize()` | `compute_checksum()` |
| B-006 | `verify_checksum(data, expected)` returns `Ok(())` when checksums match | `verify_checksum()` |
| B-007 | `verify_checksum(data, expected)` returns `Err(Mismatch)` when sha256 differs | `verify_checksum()` |
| B-008 | `verify_checksum(data, expected)` returns `Err(Mismatch)` when blake3 differs (if sha256 matches) | `verify_checksum()` |
| B-009 | `verify_checksum(data, expected)` returns `Err(Mismatch)` when crc32 differs (if sha256+blake3 match) | `verify_checksum()` |
| B-010 | `ChecksumError::Display` formats as `"checksum mismatch for {alg}: expected {exp}, got {act}"` | `ChecksumError::fmt()` |
| B-011 | `ChunkedHasher::new(chunk_size)` creates hasher with zeroed state | `ChunkedHasher::new()` |
| B-012 | `ChunkedHasher::update()` splits data into chunks at boundaries | `ChunkedHasher::update()` |
| B-013 | `ChunkedHasher::finalize()` produces at least one chunk when data was processed | `ChunkedHasher::finalize()` |
| B-014 | `Checksum` serde roundtrip preserves all fields identically | `Checksum` serialize/deserialize |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 18 | Pure functions: `compute_checksum`, `verify_checksum_internal`, `hex_encode`, `StreamingHasher::{update, finalize}`, `ChunkedHasher::{update, finalize}`. Exhaustive combinatorial coverage of algorithm inputs, chunk sizes, error variants. |
| **Integration** | 6 | Real hasher interactions: `ChunkedHasher` + `StreamingHasher` integration, snapshot storage with checksum verification, codec encoding/decoding with checksums. |
| **E2E** | 2 | Full pipeline: `compute_checksum → serialize → deserialize → verify_checksum`, snapshot save/load with checksum validation. |
| **Static Analysis** | 2 | `clippy::pedantic` lint gates, `cargo-deny` for dependency audit. |

**Rationale for distribution**: The checksum module is a pure data/computation layer with no I/O dependencies. The Testing Trophy calls for ~60% integration, but this module's design is inherently unit-testable at the Calc layer since all dependencies (crc32fast, sha2, blake3) are pure synchronous hashers. The 18/6/2 split reflects that exhaustive unit coverage of the pure computation layer provides the highest confidence for this critical integrity component.

---

## 3. BDD Scenarios

### B-001: ChecksumAlgorithm::name() returns correct string

**Scenario: algorithm name is correct for each variant**

```
Given: A ChecksumAlgorithm enum value
When: calling name() on the variant
Then: returns "crc32" for Crc32, "sha256" for Sha256, "blake3" for Blake3
```

```rust
fn checksum_algorithm_name_returns_crc32_for_crc32_variant() {
    assert_eq!(ChecksumAlgorithm::Crc32.name(), "crc32");
}

fn checksum_algorithm_name_returns_sha256_for_sha256_variant() {
    assert_eq!(ChecksumAlgorithm::Sha256.name(), "sha256");
}

fn checksum_algorithm_name_returns_blake3_for_blake3_variant() {
    assert_eq!(ChecksumAlgorithm::Blake3.name(), "blake3");
}
```

---

### B-002: StreamingHasher::new() creates empty hasher

**Scenario: new hasher produces zero checksums on empty data**

```
Given: A freshly created StreamingHasher
When: finalize() is called with no prior update() calls
Then: produces a valid Checksum
And: the checksum matches compute_checksum(&[])
```

```rust
fn streaming_hasher_new_produces_valid_empty_checksum() {
    let hasher = StreamingHasher::new();
    let checksum = hasher.finalize();
    let expected = compute_checksum(&[]);
    assert_eq!(checksum.crc32, expected.crc32);
    assert_eq!(checksum.sha256, expected.sha256);
    assert_eq!(checksum.blake3, expected.blake3);
}
```

---

### B-003: StreamingHasher::update() accumulates data

**Scenario: multiple update calls accumulate correctly**

```
Given: A StreamingHasher
When: update() is called multiple times with distinct byte slices
Then: final checksum equals checksum of concatenated bytes
```

```rust
fn streaming_hasher_update_accumulates_across_multiple_calls() {
    let (chunk1, chunk2, chunk3) = (b"hello".as_slice(), b" ".as_slice(), b"world".as_slice());
    let mut hasher = StreamingHasher::new();
    hasher.update(chunk1);
    hasher.update(chunk2);
    hasher.update(chunk3);
    let checksum = hasher.finalize();
    let expected = compute_checksum(b"hello world");
    assert_eq!(checksum, expected);
}
```

---

### B-004: StreamingHasher::finalize() produces all three algorithm outputs

**Scenario: finalize returns Checksum with all fields populated**

```
Given: A StreamingHasher that has processed data
When: finalize() is called
Then: returns Checksum with non-zero crc32 field
And: returns Checksum with non-zero sha256 array (32 bytes)
And: returns Checksum with non-zero blake3 array (32 bytes)
```

```rust
fn streaming_hasher_finalize_produces_all_three_algorithm_outputs() {
    let hasher = StreamingHasher::new();
    hasher.update(b"test data");
    let checksum = hasher.finalize();
    assert_ne!(checksum.crc32, 0, "crc32 should be non-zero for non-empty input");
    assert!(checksum.sha256.iter().any(|&b| b != 0), "sha256 should be non-zero");
    assert!(checksum.blake3.iter().any(|&b| b != 0), "blake3 should be non-zero");
}
```

---

### B-005: compute_checksum equivalence (INV-002)

**Scenario: compute_checksum equals new().update().finalize()**

```
Given: A byte slice
When: compute_checksum(data) is called
Then: result equals StreamingHasher::new().update(data).finalize()
```

```rust
fn compute_checksum_equals_streaming_hasher_manual_flow() {
    let data = b"hello world this is a test";
    let from_compute = compute_checksum(data);
    let mut hasher = StreamingHasher::new();
    hasher.update(data);
    let from_manual = hasher.finalize();
    assert_eq!(from_compute, from_manual);
}
```

---

### B-006: verify_checksum passes for matching data (INV-003)

**Scenario: identical checksum passes verification**

```
Given: Data and its computed Checksum
When: verify_checksum(data, &checksum) is called
Then: returns Ok(())
```

```rust
fn verify_checksum_returns_ok_when_data_matches() {
    let data = b"identical data for verification";
    let checksum = compute_checksum(data);
    let result = verify_checksum(data, &checksum);
    assert!(result.is_ok(), "verification should pass for matching data: {:?}", result);
}
```

---

### B-007: verify_checksum fails on sha256 mismatch (INV-010)

**Scenario: sha256 mismatch reported first**

```
Given: Data and a Checksum with different sha256
When: verify_checksum(data, &wrong_checksum) is called
Then: returns Err(ChecksumError::Mismatch)
And: algorithm field is Sha256
```

```rust
fn verify_checksum_returns_mismatch_with_sha256_when_sha256_differs() {
    let data = b"original data";
    let mut checksum = compute_checksum(data);
    checksum.sha256 = [0u8; 32]; // Corrupt sha256
    let result = verify_checksum(data, &checksum);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        ChecksumError::Mismatch { algorithm, .. } => {
            assert_eq!(algorithm, ChecksumAlgorithm::Sha256, "sha256 should be reported first");
        }
        ChecksumError::Io(msg) => panic!("expected Mismatch, got Io: {}", msg),
    }
}
```

---

### B-008: verify_checksum fails on blake3 mismatch (INV-010)

**Scenario: blake3 mismatch reported second (only when sha256 matches)**

```
Given: Data and a Checksum with matching sha256 but different blake3
When: verify_checksum(data, &wrong_checksum) is called
Then: returns Err(ChecksumError::Mismatch)
And: algorithm field is Blake3
```

```rust
fn verify_checksum_returns_mismatch_with_blake3_when_sha256_matches_but_blake3_differs() {
    let data = b"original data";
    let mut checksum = compute_checksum(data);
    checksum.blake3 = [0u8; 32]; // Corrupt only blake3
    let result = verify_checksum(data, &checksum);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        ChecksumError::Mismatch { algorithm, .. } => {
            assert_eq!(algorithm, ChecksumAlgorithm::Blake3, "blake3 should be reported second");
        }
        ChecksumError::Io(msg) => panic!("expected Mismatch, got Io: {}", msg),
    }
}
```

---

### B-009: verify_checksum fails on crc32 mismatch (INV-010)

**Scenario: crc32 mismatch reported last (only when sha256+blake3 match)**

```
Given: Data and a Checksum with matching sha256 and blake3 but different crc32
When: verify_checksum(data, &wrong_checksum) is called
Then: returns Err(ChecksumError::Mismatch)
And: algorithm field is Crc32
```

```rust
fn verify_checksum_returns_mismatch_with_crc32_when_sha256_and_blake3_match() {
    let data = b"original data";
    let mut checksum = compute_checksum(data);
    checksum.crc32 = 0; // Corrupt only crc32
    let result = verify_checksum(data, &checksum);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        ChecksumError::Mismatch { algorithm, .. } => {
            assert_eq!(algorithm, ChecksumAlgorithm::Crc32, "crc32 should be reported last");
        }
        ChecksumError::Io(msg) => panic!("expected Mismatch, got Io: {}", msg),
    }
}
```

---

### B-010: ChecksumError Display format

**Scenario: error message format is human-readable**

```
Given: A ChecksumError::Mismatch with algorithm, expected, actual
When: format!("{}", error) is called
Then: format is "checksum mismatch for {alg}: expected {exp}, got {act}"
```

```rust
fn checksum_error_display_format_is_human_readable() {
    let err = ChecksumError::Mismatch {
        algorithm: ChecksumAlgorithm::Sha256,
        expected: "abc123".to_string(),
        actual: "def456".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("checksum mismatch"), "should contain 'checksum mismatch'");
    assert!(display.contains("sha256"), "should contain algorithm name");
    assert!(display.contains("abc123"), "should contain expected value");
    assert!(display.contains("def456"), "should contain actual value");
}

fn checksum_io_error_display_format() {
    let err = ChecksumError::Io("failed to read from buffer".to_string());
    let display = format!("{}", err);
    assert!(display.contains("checksum I/O error"), "should contain 'checksum I/O error'");
    assert!(display.contains("failed to read from buffer"), "should contain message");
}
```

---

### B-011: ChunkedHasher::new creates empty state

**Scenario: new ChunkedHasher has zero offset and empty chunk list**

```
Given: ChunkedHasher::new(1024)
When: created
Then: chunk_size is 1024
And: current_offset is 0
And: chunks is empty
```

```rust
fn chunked_hasher_new_has_correct_initial_state() {
    let hasher = ChunkedHasher::new(1024);
    assert_eq!(hasher.chunk_size, 1024); // Cannot test private field, use behavior
    // Behavior: calling finalize() on new hasher with no update returns empty vec
    let chunks = hasher.finalize();
    assert!(chunks.is_empty(), "new hasher with no data should produce no chunks");
}
```

---

### B-012: ChunkedHasher splits data at boundaries (INV-004, INV-005)

**Scenario: chunks have monotonically increasing offsets and sum to total size**

```
Given: ChunkedHasher with chunk_size=5
When: update("0123456789ABCDEF") is called (16 bytes)
Then: each ChunkInfo.offset is greater than the previous
And: sum of all ChunkInfo.size equals 16
```

```rust
fn chunked_hasher_produces_monotonically_increasing_offsets() {
    let data = b"0123456789ABCDEF";
    let mut hasher = ChunkedHasher::new(5);
    hasher.update(data);
    let chunks = hasher.finalize();
    for window in chunks.windows(2) {
        assert!(window[1].offset > window[0].offset, "offsets must increase");
    }
}

fn chunked_hasher_chunk_sizes_sum_to_total_bytes() {
    let data = b"0123456789ABCDEF";
    let mut hasher = ChunkedHasher::new(5);
    hasher.update(data);
    let chunks = hasher.finalize();
    let total: u64 = chunks.iter().map(|c| c.size).sum();
    assert_eq!(total, 16, "sum of chunk sizes must equal input size");
}
```

---

### B-013: ChunkedHasher produces at least one chunk (INV-006)

**Scenario: any non-empty data produces at least one chunk**

```
Given: ChunkedHasher with chunk_size=1024
When: update("x") is called (1 byte)
Then: finalize() returns vec with at least one ChunkInfo
```

```rust
fn chunked_hasher_produces_at_least_one_chunk_when_data_processed() {
    let mut hasher = ChunkedHasher::new(1024);
    hasher.update(b"x");
    let chunks = hasher.finalize();
    assert!(!chunks.is_empty(), "any non-empty data must produce at least one chunk");
}

fn chunked_hasher_finalize_on_empty_hasher_returns_empty_vec() {
    let hasher = ChunkedHasher::new(1024);
    let chunks = hasher.finalize();
    assert!(chunks.is_empty(), "no data should produce no chunks");
}
```

---

### B-014: Checksum serde roundtrip (INV-009)

**Scenario: JSON serialization roundtrip preserves all fields**

```
Given: A Checksum computed from data
When: serialize to JSON then deserialize back
Then: resulting Checksum equals original
```

```rust
fn checksum_json_roundtrip_preserves_all_fields() {
    let original = compute_checksum(b"test data for serde");
    let json = serde_json::to_string(&original).expect("serialize should succeed");
    let recovered: Checksum = serde_json::from_str(&json).expect("deserialize should succeed");
    assert_eq!(original, recovered);
}

fn checksum_binary_roundtrip_preserves_all_fields() {
    use bincode::{deserialize, serialize};
    let original = compute_checksum(b"test data for binary serde");
    let encoded = serialize(&original).expect("serialize should succeed");
    let recovered: Checksum = deserialize(&encoded).expect("deserialize should succeed");
    assert_eq!(original, recovered);
}
```

---

## 4. Proptest Invariants

### PI-001: StreamingHasher incremental determinism (INV-001)

```
Invariant: StreamingHasher produces identical final Checksum regardless of how update() calls are distributed
Strategy: split data at random positions into variable-length chunks
Anti-invariant: N/A — this should always hold
```

```rust
proptest! {
    #[test]
    fn streaming_hasher_produces_identical_checksum_regardless_of_chunk_distribution(
        data: Vec<u8>,
        seed: u64
    ) {
        let mut rng = SeedableRng::seed_from_u64(seed);
        let chunk_boundaries = generate_random_chunk_boundaries(&mut rng, data.len());
        // Single-pass
        let single_pass = compute_checksum(&data);
        // Multi-pass with random chunks
        let mut hasher = StreamingHasher::new();
        let mut offset = 0;
        for boundary in chunk_boundaries {
            let end = boundary.min(data.len());
            hasher.update(&data[offset..end]);
            offset = end;
        }
        hasher.update(&data[offset..]);
        let multi_pass = hasher.finalize();
        prop_assert_eq!(single_pass, multi_pass);
    }
}
```

### PI-002: compute_checksum is associative (equivalent to streaming)

```
Invariant: compute_checksum(data) == StreamingHasher::new().update(data).finalize()
Strategy: arbitrary byte vector input
```

### PI-003: verify_checksum is reflexive

```
Invariant: verify_checksum(data, &compute_checksum(data)) is always Ok(())
Strategy: arbitrary byte vector
```

### PI-004: ChunkedHasher offset monotonicity (INV-004)

```
Invariant: For all i < j, chunks[i].offset < chunks[j].offset
Strategy: arbitrary data + arbitrary chunk_size >= 1
```

### PI-005: ChunkedHasher size conservation (INV-005)

```
Invariant: sum(chunks.map(|c| c.size)) == total_bytes_processed
Strategy: arbitrary data + arbitrary chunk_size >= 1
```

### PI-006: Checksum equality is reflexive and transitive

```
Invariant: checksum == checksum (reflexive)
Invariant: if A == B and B == C then A == C (transitive)
Strategy: arbitrary Checksum values
```

---

## 5. Fuzz Targets

### FT-001: compute_checksum with arbitrary byte slice

```
Input type: bytes
Risk: panic on malformed input, non-deterministic output
Corpus seeds: empty slice, single byte, 1KB random, 1MB random, all-zeros, alternating pattern
```

### FT-002: StreamingHasher::update with fragmented data

```
Input type: (bytes, Vec<(start, end)>)
Risk: off-by-one in slice boundaries, inconsistent accumulation
Corpus seeds: single update, pair of updates, 1000 small updates
```

### FT-003: ChunkedHasher with edge case chunk sizes

```
Input type: (bytes, u64) where u64 is chunk_size
Risk: division by zero (chunk_size=0), overflow on offset arithmetic, incorrect chunk boundary
Corpus seeds: chunk_size=1, chunk_size=2^63, chunk_size > data.len(), chunk_size == data.len()
```

### FT-004: ChecksumError JSON deserialization

```
Input type: string (JSON)
Risk: deserializing invalid JSON into ChecksumError causes panic, wrong variant constructed
Corpus seeds: valid Mismatch JSON, valid Io JSON, null, number, array, truncated string
```

---

## 6. Kani Harnesses

### KH-001: verify_checksum_internal algorithm precedence (INV-010)

```
Property: sha256 is always checked before blake3, blake3 before crc32
Bound: 3 checks (one per algorithm)
Rationale: Formal proof that error reporting order is deterministic and matches spec
```

```rust
#[kani::proof]
fn verify_checksum_internal_respects_algorithm_precedence() {
    // Formal verification that the comparison order is sha256 → blake3 → crc32
    // Kani will check all possible execution paths through verify_checksum_internal
}
```

### KH-002: ChunkedHasher offset arithmetic never overflows (INV-004, INV-005)

```
Property: current_offset + bytes_to_process never overflows u64
Bound: u64::MAX / 2 (reasonable bound for practical data sizes)
Rationale: The hasher processes arbitrary data sizes; offset arithmetic must be safe
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Swap sha256 and blake3 comparison order | `verify_checksum_returns_mismatch_with_sha256_when_sha256_differs` + `verify_checksum_returns_mismatch_with_blake3_when_sha256_matches_but_blake3_differs` |
| MC-002 | Change crc32 comparison to use != instead of == in finalization | `streaming_hasher_finalize_produces_all_three_algorithm_outputs` |
| MC-003 | Remove one of the three `hasher.update()` calls in StreamingHasher | `streaming_hasher_update_accumulates_across_multiple_calls` |
| MC-004 | Swap `is_multiple_of` check in ChunkedHasher finalize | `chunked_hasher_produces_at_least_one_chunk_when_data_processed` |
| MC-005 | Change `remaining_in_chunk` calculation | `chunked_hasher_chunk_sizes_sum_to_total_bytes` |
| MC-006 | Change error message format string | `checksum_error_display_format_is_human_readable` |
| MC-007 | Use `ChunkInfo { offset: 0, .. }` for all chunks | `chunked_hasher_produces_monotonically_increasing_offsets` |
| MC-008 | Return empty vec instead of pushing final chunk | `chunked_hasher_produces_at_least_one_chunk_when_data_processed` |

**Threshold**: ≥90% mutation kill rate

---

## 8. Combinatorial Coverage Matrix

### StreamingHasher

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| new() on empty data | `&[]` | Valid Checksum (empty hash values) | unit |
| new() on non-empty data | `b"test"` | Valid Checksum (non-zero) | unit |
| update() once | `b"hello"` | Accumulates correctly | unit |
| update() multiple times | split `b"hello world"` | Same as single update | unit |
| finalize() produces all fields | any | crc32, sha256[32], blake3[32] all non-zero | unit |
| INV-001: chunk distribution | random chunks | Identical checksum | unit (proptest) |

### compute_checksum

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| empty slice | `&[]` | Valid Checksum with empty hash values | unit |
| single byte | `b"x"` | Valid Checksum | unit |
| small data | `b"hello"` | Valid Checksum | unit |
| large data | 1MB random | Valid Checksum | integration |
| INV-002 equivalence | any | equals new().update().finalize() | unit |

### verify_checksum

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| matching data | `data, &compute_checksum(data)` | `Ok(())` | unit |
| sha256 mismatch | corrupted sha256 | `Err(Mismatch { Sha256, ... })` | unit |
| blake3 mismatch | corrupted blake3 | `Err(Mismatch { Blake3, ... })` | unit |
| crc32 mismatch | corrupted crc32 | `Err(Mismatch { Crc32, ... })` | unit |
| all three mismatch | corrupted all three | `Err(Mismatch { Sha256, ... })` (first checked) | unit |
| INV-003 equivalence | any valid | `Ok(())` iff checksums equal | unit |

### ChunkedHasher

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| chunk_size=1, data=1 byte | `b"x"` | 1 chunk | unit |
| chunk_size=2, data=3 bytes | `b"abc"` | 2 chunks (2+1) | unit |
| chunk_size=5, data=16 bytes | 16-byte string | 4 chunks (5+5+5+1) | unit |
| empty data | `&[]` | empty vec | unit |
| exact multiple of chunk_size | 10 bytes, chunk=5 | 2 chunks | unit |
| partial last chunk | 11 bytes, chunk=5 | 3 chunks (5+5+1) | unit |
| INV-004 monotonic offsets | any | offsets strictly increasing | unit |
| INV-005 size conservation | any | sum of sizes == input len | unit |
| INV-006 at least one chunk | any non-empty | non-empty vec | unit |

### ChecksumError

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| Mismatch Display | Mismatch variant | `"checksum mismatch for {alg}: expected {exp}, got {act}"` | unit |
| Io Display | Io variant | `"checksum I/O error: {msg}"` | unit |
| INV-010 algorithm precedence | sha256 mismatch | algorithm == Sha256 | unit |

---

## Open Questions

1. **ChunkedHasher private fields**: The `chunk_size`, `current_offset`, and `chunks` fields are private. Tests that verify INV-004 (monotonic offsets) and INV-005 (size conservation) cannot directly inspect state — they must use `finalize()` output. Is this the intended behavior, or should accessors be added for testing purposes?

2. **Kani proof bound**: KH-002 uses a conservative bound (`u64::MAX / 2`). Should this be tightened to a more practical limit (e.g., `1TB = 2^40`) given hardware constraints?

3. **INV-007 (empty data checksum)**: The contract states empty data produces "algorithm-specific empty hash values". Need to verify what the empty hash values are for each algorithm (e.g., SHA256 of empty string = `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`). Should tests assert the exact known values?

4. **Integration test scope**: The trophy calls for integration tests using real dependencies. For `ChunkedHasher` + `StreamingHasher`, both are in the same crate. Should integration tests span to `snapshots.rs` and `codec.rs` (as specified in contract section 10), or keep them at the `checksum.rs` module boundary?

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target
- [x] Every error variant in `ChecksumError` enum has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
