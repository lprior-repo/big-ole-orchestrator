# Test Plan: Content-Addressed Storage (BlobStore)

## Summary

- Behaviors identified: 47
- Trophy allocation: 35 unit / 25 integration / 5 proptest / 2 fuzz (Total 67 tests)
- Proptest invariants: 8
- Fuzz targets: 2
- Target Mutation Kill Rate: ≥90%

## 1. Behavior Inventory

### Data Layer — ContentAddress

1. `ContentAddress::new` accepts valid 64-char lowercase hex SHA-256
2. `ContentAddress::new` rejects wrong length (< 64 chars)
3. `ContentAddress::new` rejects wrong length (> 64 chars)
4. `ContentAddress::new` rejects uppercase hex characters
5. `ContentAddress::new` rejects non-hex characters
6. `ContentAddress::new` rejects empty string
7. `ContentAddress::from_bytes` constructs from valid 32-byte slice
8. `ContentAddress::from_bytes` produces correct hex string
9. `ContentAddress::as_str` returns the inner string
10. `ContentAddress::as_bytes` returns correct 32-byte representation
11. `ContentAddress` roundtrip: `new` → `as_bytes` → `from_bytes` → `as_str` matches original

### Data Layer — PackFileId

12. `PackFileId::new` accepts non-empty string
13. `PackFileId::new` rejects empty string
14. `PackFileId::as_str` returns the inner string

### Data Layer — PackIndexEntry

15. `PackIndexEntry::new` constructs with valid fields
16. `PackIndexEntry::new` returns correct `content_addr` accessor
17. `PackIndexEntry::new` returns correct `pack_file_id` accessor
18. `PackIndexEntry::new` returns correct `offset_bytes` accessor
19. `PackIndexEntry::new` returns correct `size_bytes` accessor

### Data Layer — BlobRecord

20. `BlobRecord::new` constructs with valid fields (`reference_count > 0`, `created_at_ms > 0`)
21. `BlobRecord::new` rejects zero `reference_count`
22. `BlobRecord::new` rejects zero `created_at_ms`
23. `BlobRecord::is_expired` returns false when `now_ms < expires_at_ms`
24. `BlobRecord::is_expired` returns true when `now_ms >= expires_at_ms`
25. `BlobRecord::is_expired` returns false when `expires_at_ms` is `None` (never expires)
26. `BlobRecord::increment_ref_count` saturates at `u64::MAX`
27. `BlobRecord::decrement_ref_count` saturates at 0

### Data Layer — BlobStoreError

28. `BlobStoreError::ContentNotFound` Display formats correctly
29. `BlobStoreError::PackFileNotFound` Display formats correctly
30. `BlobStoreError::DuplicateContent` Display formats correctly
31. `BlobStoreError::CorruptPackIndex` Display formats correctly
32. `BlobStoreError::CorruptPackFile` Display formats correctly
33. `BlobStoreError::ChecksumMismatch` Display formats correctly
34. `BlobStoreError::SerializationFailed` Display formats correctly
35. `BlobStoreError::DeserializationFailed` Display formats correctly
36. `BlobStoreError::Storage` Display formats correctly
37. `BlobStoreError::InvalidArgument` Display formats correctly
38. `BlobStoreError::GcCycleInProgress` Display formats correctly
39. `BlobStoreError::PackFileFull` Display formats correctly
40. All `BlobStoreError` variants implement `std::error::Error` correctly

### Calc Layer — Encoding/Decoding

41. `encode_content_address` produces valid UTF-8 bytes
42. `decode_content_address` roundtrips `encode_content_address` output
43. `decode_content_address` rejects invalid UTF-8
44. `decode_content_address` rejects invalid content address format
45. `validate_content_address` accepts valid SHA-256 hex
46. `validate_content_address` rejects invalid formats
47. `encode_pack_index_entry` and `decode_pack_index_entry` roundtrip correctly
48. `encode_blob_record` and `decode_blob_record` roundtrip correctly

## 2. Trophy Allocation

### Unit Tests (35)

Cover all pure data type constructors, accessors, invariants, and calc encoding/decoding functions. Includes exhaustive coverage of every `BlobStoreError` Display match arm.

### Integration Tests (25)

Cover `BlobStore` trait behavior using a mock in-memory implementation:

- `store()` happy path with dedup detection
- `retrieve()` happy path and `ContentNotFound` case
- `contains()` true/false cases
- increment/decrement `ref_count` behavior
- `get_metadata` returns correct `BlobRecord`
- `list_gc_candidates` filters correctly (`ref_count=0` AND expired)
- `run_gc` collects candidates and returns count
- `run_gc` rejects concurrent GC (`GcCycleInProgress`)
- `store_streaming` computes SHA-256 incrementally
- `retrieve_streaming` streams without buffering full blob

### Proptest (5)

Property-based testing for `ContentAddress` invariants:

- `from_bytes` → `as_bytes` is identity
- `as_str` always produces valid 64-char lowercase hex
- `ContentAddress` equality is reflexive/symmetric/transitive

### Fuzz Targets (2)

- Fuzz: `decode_content_address` with random bytes (panic resistance)
- Fuzz: `validate_content_address` with random strings (panic resistance)

## 3. BDD Scenarios

### ContentAddress Construction

**Given:** A valid 64-char lowercase hex string
**When:** `ContentAddress::new` is called
**Then:** Returns `Ok(ContentAddress)` with correct `as_str`

**Given:** A string with uppercase hex characters
**When:** `ContentAddress::new` is called
**Then:** Returns `Err(BlobStoreError::InvalidArgument)`

**Given:** A string with wrong length
**When:** `ContentAddress::new` is called
**Then:** Returns `Err(BlobStoreError::InvalidArgument)`

### BlobStore Store and Retrieve

**Given:** An empty `BlobStore`
**When:** `store(data)` is called with new content
**Then:** Returns `Ok(ContentAddress)` computed via SHA-256

**Given:** A `BlobStore` with existing content
**When:** `store(data)` is called with duplicate content
**Then:** Returns `Err(BlobStoreError::DuplicateContent)`

**Given:** A `BlobStore` with stored content
**When:** `retrieve(addr)` is called with valid address
**Then:** Returns `Ok(original_data)`

**Given:** A `BlobStore` with stored content
**When:** `retrieve(addr)` is called with unknown address
**Then:** Returns `Err(BlobStoreError::ContentNotFound)`

### Reference Counting

**Given:** A `BlobStore` with stored content at `ref_count=1`
**When:** `increment_ref_count` is called
**Then:** Returns 2 and `ref_count` is updated

**Given:** A `BlobStore` with stored content at `ref_count=1`
**When:** `decrement_ref_count` is called
**Then:** Returns 0 and blob becomes GC candidate

### Garbage Collection

**Given:** A `BlobStore` with unreferenced expired blobs
**When:** `list_gc_candidates(now_ms)` is called
**Then:** Returns only blobs where `ref_count=0` AND `is_expired(now_ms)=true`

**Given:** A `BlobStore` with unreferenced expired blobs
**When:** `run_gc(now_ms)` is called
**Then:** Returns count of collected blobs and removes them from store

**Given:** A GC cycle already in progress
**When:** `run_gc` is called again
**Then:** Returns `Err(BlobStoreError::GcCycleInProgress)`

## 4. Proptest Invariants

### ContentAddress Byte Roundtrip

**Invariant:** For any 32-byte array, `ContentAddress::from_bytes(&bytes).as_bytes() == bytes`
**Strategy:** `any::<[u8; 32]>()`
**Anti-invariant:** Modifying any byte breaks the invariant

### ContentAddress String Validity

**Invariant:** `ContentAddress::new(s).ok().map(|a| a.as_str().len() == 64)` is always true for valid input
**Strategy:** `prop::string::saturated_with(|s| s.chars().filter(|c| c.is_ascii_hexdigit()).take(64).collect())`
**Anti-invariant:** Taking only 63 chars produces `Err`

### PackFileId Non-Empty

**Invariant:** `PackFileId::new(s).is_err()` when `s.is_empty()`
**Strategy:** `any::<String>()`
**Anti-invariant:** Empty string produces `Err`

### BlobRecord Reference Count

**Invariant:** `BlobRecord::new(.., 0, ..)` always returns `Err`
**Strategy:** arbitrary valid fields with zero_ref variant
**Anti-invariant:** Zero reference_count produces `Err`

### BlobRecord Expiry Boundary

**Invariant:** `record.is_expired(t)` is monotonic — if `is_expired(t)` true, then `is_expired(t+n)` true for n>0
**Strategy:** arbitrary `BlobRecord` with `expires_at_ms` and various `now_ms` values

### Content Address Encoding Roundtrip

**Invariant:** `decode_content_address(encode_content_address(&addr)) == addr`
**Strategy:** arbitrary valid `ContentAddress` values
**Anti-invariant:** Corrupting encoded bytes breaks roundtrip

### PackIndexEntry Roundtrip

**Invariant:** `decode_pack_index_entry(encode_pack_index_entry(&entry)) == entry`
**Strategy:** arbitrary valid `PackIndexEntry`
**Anti-invariant:** JSON corruption breaks roundtrip

### BlobRecord Roundtrip

**Invariant:** `decode_blob_record(encode_blob_record(&record)) == record`
**Strategy:** arbitrary valid `BlobRecord`
**Anti-invariant:** JSON corruption breaks roundtrip

## 5. Fuzz Targets

### Fuzz Target: decode_content_address

**Input type:** random bytes
**Risk:** Panic on invalid UTF-8, OOM on huge inputs
**Corpus seeds:** valid hex strings, empty bytes, UTF-8 surrogates, 1MB of random bytes

### Fuzz Target: validate_content_address

**Input type:** random strings
**Risk:** Panic on edge cases, unexpected rejection of valid input
**Corpus seeds:** valid SHA-256, empty string, single char, 64 uppercase, 64 chars with symbols

## 6. Mutation Checkpoints

Critical mutations to survive:

- Changing `len()` check in `ContentAddress::new` from `!= 64` to `< 64` must be caught
- Removing the lowercase hex check must be caught by `content_address_rejects_uppercase_hex`
- Changing `is_expired` boundary from `>=` to `>` must be caught
- Removing `ref_count == 0` check in `decrement` must be caught
- Changing GC candidate filter from `ref_count=0 AND expired` to `OR` must be caught
- Adding duplicate content to store without checking must be caught by `DuplicateContent` error

**Threshold:** 90% mutation kill rate minimum
**Coverage:** 90% line coverage minimum

## 7. Combinatorial Coverage Matrix

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| ContentAddress Happy Path | 64 lowercase hex | `Ok(ContentAddress)` | Unit |
| ContentAddress Wrong Length Short | 63 chars | `Err(InvalidArgument)` | Unit |
| ContentAddress Wrong Length Long | 65 chars | `Err(InvalidArgument)` | Unit |
| ContentAddress Uppercase | 64 uppercase hex | `Err(InvalidArgument)` | Unit |
| ContentAddress Non-Hex | string with 'g'-'z' | `Err(InvalidArgument)` | Unit |
| ContentAddress Empty | "" | `Err(InvalidArgument)` | Unit |
| PackFileId Happy Path | "pack-001" | `Ok(PackFileId)` | Unit |
| PackFileId Empty | "" | `Err(InvalidArgument)` | Unit |
| BlobRecord Happy Path | valid fields | `Ok(BlobRecord)` | Unit |
| BlobRecord Zero RefCount | ref_count=0 | `Err(InvalidArgument)` | Unit |
| BlobRecord Zero CreatedAt | created_at_ms=0 | `Err(InvalidArgument)` | Unit |
| BlobRecord Expired True | now >= expires_at | true | Unit |
| BlobRecord Expired False | now < expires_at | false | Unit |
| BlobRecord Never Expires | expires_at=None | false (any now) | Unit |
| BlobRecord Increment Saturate | ref_count=u64::MAX | u64::MAX | Unit |
| BlobRecord Decrement Saturate | ref_count=0 | 0 | Unit |
| Error Display: ContentNotFound | addr="abc" | contains "abc" | Unit |
| Error Display: PackFileNotFound | id="p1" | contains "p1" | Unit |
| Error Display: DuplicateContent | addr="abc" | contains "abc" | Unit |
| Error Display: ChecksumMismatch | expected/actual | contains both | Unit |
| BlobStore Store New | new data | `Ok(content_addr)` | Integration |
| BlobStore Store Duplicate | existing data | `Err(DuplicateContent)` | Integration |
| BlobStore Retrieve Found | stored addr | `Ok(original_data)` | Integration |
| BlobStore Retrieve NotFound | unknown addr | `Err(ContentNotFound)` | Integration |
| BlobStore Contains True | stored addr | true | Integration |
| BlobStore Contains False | unknown addr | false | Integration |
| BlobStore Increment Ref | stored addr | new_count | Integration |
| BlobStore Decrement to Zero | ref_count=1 | 0 | Integration |
| BlobStore Get Metadata | stored addr | `Ok(BlobRecord)` | Integration |
| BlobStore GC Candidates | ref=0+expired | in candidates | Integration |
| BlobStore GC Candidates | ref=1 | not in candidates | Integration |
| BlobStore GC Run | expired unreferenced | collected count | Integration |
| BlobStore GC Concurrent | while GC running | `Err(GcCycleInProgress)` | Integration |
| Encoding Roundtrip: ContentAddr | ContentAddress | `decode(encode) == original` | Unit |
| Encoding Roundtrip: PackIndex | PackIndexEntry | `decode(encode) == original` | Unit |
| Encoding Roundtrip: BlobRecord | BlobRecord | `decode(encode) == original` | Unit |
| Proptest: Byte Roundtrip | [u8; 32] | `from_bytes(as_bytes) == original` | Proptest |
| Proptest: String Validity | 64 hex chars | `len == 64 && lowercase` | Proptest |
| Fuzz: Decode Invalid UTF-8 | random bytes | no panic, `Err` | Fuzz |
| Fuzz: Validate Random String | random str | no panic, `Err/Ok` | Fuzz |

## 8. Implementation Notes

Since no concrete `BlobStore` implementation exists, tests should:

1. First create a mock in-memory `BlobStore` implementation for integration tests
2. Focus unit tests on pure functions (data types, calc layer)
3. The mock should support: tempdir storage, in-memory pack index, configurable GC behavior
4. Integration tests require `tokio` runtime for async trait methods

## 9. Test File Locations

| Test Type | Location |
|-----------|----------|
| Unit tests (data types) | `crates/vo-storage/src/blob_store.rs` (existing, extend) |
| Unit tests (encoding) | `crates/vo-storage/src/blob_store.rs` (existing, extend) |
| Integration tests | `crates/vo-storage/tests/blob_store_impl.rs` (new) |
| Proptest | `crates/vo-storage/proptest/blob_store.rs` (new) |
| Fuzz | `crates/vo-storage/fuzz/` (new targets) |

## 10. Dependencies

- `BlobStore` trait implementation (pending ve-dafv)
- `tokio` for async runtime in integration tests
- `proptest` for property-based tests
- `cargo-fuzz` or `libfuzzer-sys` for fuzzing
- `tempfile` for temporary test directories
