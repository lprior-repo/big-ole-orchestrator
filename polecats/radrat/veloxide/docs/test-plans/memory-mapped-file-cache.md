# Test Plan: Memory-Mapped File Cache

**Contract**: `docs/contracts/memory-mapped-file-cache.md`
**Issue**: ve-vsb3
**Target crate**: `crates/vo-storage/src/mmap_cache.rs` (implementation + unit tests)

## Scope

This plan covers exhaustive testing for `MmapCache`, `MmapCacheBuilder`, `MmapCacheError`, all 16 invariants (INV-001 through INV-016), the full error taxonomy, edge cases, and property-based invariants. Tests are organized by the Testing Trophy: unit tests (majority), targeted property tests via proptest, and edge-case stress tests.

---

## 1. Construction & Builder Tests

### 1.1 MmapCache::new

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CN-001 | Create cache in non-existent directory | INV-001 | Ok, directory created |
| CN-002 | Create cache in existing directory | Happy path | Ok |
| CN-003 | Create cache with zero max_memory_bytes | INV-002 | Ok (zero accepted at construction; rejected at insert) |
| CN-004 | Create cache with very large max_memory_bytes | Boundary | Ok (usize::MAX) |
| CN-005 | Create cache with max_memory_bytes = 1 | Boundary | Ok |
| CN-006 | New cache has zero current_memory_usage | Initial state | 0 |
| CN-007 | New cache has zero len | Initial state | 0 |
| CN-008 | New cache reports is_empty | Initial state | true |

### 1.2 MmapCacheBuilder

| ID | Test | Category | Expected |
|----|------|----------|----------|
| BB-001 | Builder new with path | Happy path | Builder created |
| BB-002 | Builder default max_memory_bytes is 100 MiB | Default | 100 * 1024 * 1024 |
| BB-003 | Builder max_memory_bytes override | Configuration | Custom value respected |
| BB-004 | Builder build creates valid cache | Happy path | Ok, cache usable |
| BB-005 | Builder build with non-existent path | INV-001 | Ok, directory created |
| BB-006 | Builder is const-constructible | API | Compiles in const context |

---

## 2. Insert Operation Tests

### 2.1 Basic Insert

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IN-001 | Insert single key-value pair | Happy path | Ok |
| IN-002 | Insert empty data slice `&[]` | Boundary | Ok |
| IN-003 | Insert data exactly at max_memory_bytes | Boundary | Ok |
| IN-004 | Insert data exceeding max_memory_bytes with empty cache | INV-003 | Eviction loop runs, CacheFull if single item > max |
| IN-005 | Insert same key twice (overwrite) | Edge case | Ok, data replaced |
| IN-006 | Insert multiple distinct keys | Happy path | All retrievable |
| IN-007 | Insert increments len | INV-004 | len == number of unique keys |
| IN-008 | Insert updates current_memory_usage | INV-003 | Accurate byte count |

### 2.2 Insert with LRU Eviction

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IE-001 | Eviction triggered when cache full | INV-006, INV-007 | Ok, LRU entry evicted |
| IE-002 | Eviction removes least-recently-used first | INV-005 | First inserted key evicted |
| IE-003 | Eviction removes multiple entries if needed | INV-007 | Enough entries evicted for space |
| IE-004 | Eviction deletes region file from disk | INV-007 | File no longer exists on filesystem |
| IE-005 | Eviction updates current_memory_usage | INV-003 | Accurate after eviction |
| IE-006 | Eviction updates lru_queue | INV-004 | Queue length matches entries |
| IE-007 | Insert single item larger than max_memory_bytes | INV-007 | CacheFull (nothing to evict helps) |
| IE-008 | Insert at exact capacity boundary | Boundary | No eviction triggered |
| IE-009 | Eviction of all entries for oversized insert | INV-007 | Cache empty after failed insert |
| IE-010 | Insert after eviction — new entry retrievable | INV-008 | get returns correct data |

### 2.3 Insert Atomicity (INV-006)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| IA-001 | Failed insert does not leave partial entry | INV-006 | On error, key not in cache |
| IA-002 | Failed insert does not corrupt memory tracking | INV-003 | current_memory_usage consistent |
| IA-003 | Failed insert does not corrupt LRU queue | INV-004 | Queue synchronized with entries |

---

## 3. Get Operation Tests

### 3.1 Basic Get

| ID | Test | Category | Expected |
|----|------|----------|----------|
| GT-001 | Get existing key returns correct data | INV-008 | Ok(data) |
| GT-002 | Get non-existent key returns RegionNotFound | Error | Err(RegionNotFound(key)) |
| GT-003 | Get after eviction returns RegionNotFound | INV-007 | Err(RegionNotFound) |
| GT-004 | Get empty data returns empty vec | Boundary | Ok(vec![]) |
| GT-005 | Get large data (multi-page) | Boundary | Ok, full data intact |
| GT-006 | Get does not update LRU ordering | INV-016 | LRU order unchanged after get |

### 3.2 Get Error Paths

| ID | Test | Category | Expected |
|----|------|----------|----------|
| GE-001 | Get after region file deleted externally | Error | Err(IoError) or Err(MmapError) |
| GE-002 | Get after region file corrupted (zero-length) | Error | Err variant (data mismatch) |
| GE-003 | Get on cache with many entries | Stress | Correct data for each |

---

## 4. Contains Key Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CK-001 | contains_key returns false for missing key | Happy path | false |
| CK-002 | contains_key returns true after insert | Happy path | true |
| CK-003 | contains_key returns false after remove | Happy path | false |
| CK-004 | contains_key returns false after eviction | INV-007 | false |
| CK-005 | contains_key does not update LRU | INV-015 | LRU order unchanged |

---

## 5. Remove Operation Tests

### 5.1 Basic Remove

| ID | Test | Category | Expected |
|----|------|----------|----------|
| RM-001 | Remove existing key | INV-009 | Ok, key gone |
| RM-002 | Remove non-existent key | Happy path | Ok (no-op, no error) |
| RM-003 | Remove decrements len | INV-004 | len decreased |
| RM-004 | Remove updates current_memory_usage | INV-003 | Bytes subtracted |
| RM-005 | Remove deletes region file from disk | INV-009 | File no longer exists |
| RM-006 | Remove removes from both entries and lru_queue | INV-004 | Synchronized |

### 5.2 Remove Edge Cases

| ID | Test | Category | Expected |
|----|------|----------|----------|
| RE-001 | Remove the only entry | Edge case | Cache empty |
| RE-002 | Remove then re-insert same key | Edge case | Ok, new data stored |
| RE-003 | Remove entry that is LRU front | INV-005 | Queue updated correctly |
| RE-004 | Remove entry that is LRU back (MRU) | INV-005 | Queue updated correctly |
| RE-005 | Remove entry in middle of LRU queue | INV-005 | Queue updated correctly |

---

## 6. Prefetch & Read-Ahead Tests

### 6.1 Prefetch

| ID | Test | Category | Expected |
|----|------|----------|----------|
| PF-001 | Prefetch existing key | INV-013 | Ok |
| PF-002 | Prefetch non-existent key (no entry) | INV-013 | Ok (silently skipped) |
| PF-003 | Prefetch after region file deleted externally | Error | Err(IoError) or Err(MmapError) |
| PF-004 | Prefetch does not update LRU | Happy path | LRU order unchanged |
| PF-005 | Prefetch returns data into OS page cache | INV-013 | Subsequent get is fast (no assertion, coverage) |

### 6.2 Read-Ahead

| ID | Test | Category | Expected |
|----|------|----------|----------|
| RA-001 | Read-ahead multiple existing keys | INV-014 | Ok |
| RA-002 | Read-ahead with mix of existing and missing keys | INV-014 | Continues on individual errors (see note) |
| RA-003 | Read-ahead empty key list | Edge case | Ok |
| RA-004 | Read-ahead single key | Edge case | Ok |
| RA-005 | Read-ahead all missing keys | INV-014 | Ok or Err (depends on INV-014 interpretation) |

> **Note on INV-014**: The contract states read_ahead "continues on individual errors", but the current implementation uses `?` which returns on the first error. Tests RA-002 and RA-005 should document the expected behavior. If INV-014 is authoritative, the implementation has a bug that the TDD red phase (ve-5i3c) should expose.

---

## 7. Clear & Drop Tests

### 7.1 Clear

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CL-001 | Clear removes all entries | INV-010 | len == 0 |
| CL-002 | Clear resets current_memory_usage to zero | INV-010 | 0 |
| CL-003 | Clear deletes all region files from disk | INV-010 | Base path empty (except dir itself) |
| CL-004 | Clear on already-empty cache | Edge case | Ok, no error |
| CL-005 | Clear resets lru_queue | INV-004 | Queue empty |
| CL-006 | Clear resets entries map | INV-004 | Map empty |
| CL-007 | Insert after clear works normally | Happy path | Ok, data stored |

### 7.2 Drop

| ID | Test | Category | Expected |
|----|------|----------|----------|
| DR-001 | Drop cleans up all region files | INV-011 | No region files remain |
| DR-002 | Drop on cache with many entries | INV-011 | All files removed |
| DR-003 | Drop after clear is safe | INV-011 | No double-free, no error |
| DR-004 | Drop when cache is empty | INV-011 | No error |

---

## 8. Key Sanitization Tests (INV-012)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| KS-001 | Key with `/` sanitized to `_` | INV-012 | File created with `_` |
| KS-002 | Key with `\` sanitized to `_` | INV-012 | File created with `_` |
| KS-003 | Key with `:` sanitized to `_` | INV-012 | File created with `_` |
| KS-004 | Key with all three special chars | INV-012 | All replaced |
| KS-005 | Key with no special chars unchanged | INV-012 | File name matches key |
| KS-006 | Sanitized key round-trips (insert + get) | INV-012 | Data retrievable |
| KS-007 | Keys that sanitize to same name collide | Edge case | Last write wins |

---

## 9. Memory Tracking Tests (INV-003)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| MT-001 | current_memory_usage starts at 0 | Initial state | 0 |
| MT-002 | Insert adds data.len() to usage | INV-003 | Exact |
| MT-003 | Remove subtracts region.size from usage | INV-003 | Exact |
| MT-004 | Clear resets to 0 | INV-003 | 0 |
| MT-005 | current_memory_usage never exceeds max_memory_bytes | INV-003 | Always true |
| MT-006 | Overwrite (insert same key) updates usage correctly | Edge case | Old size removed, new size added |
| MT-007 | Eviction reduces usage by evicted entry sizes | INV-003 | Exact |
| MT-008 | Usage after insert-evict-insert cycle is correct | INV-003 | Consistent |

---

## 10. LRU Ordering Tests (INV-005)

| ID | Test | Category | Expected |
|----|------|----------|----------|
| LR-001 | Single entry is both LRU and MRU | Happy path | Queue length 1 |
| LR-002 | First inserted is evicted first | INV-005 | Correct eviction order |
| LR-003 | Three entries evicted in insertion order | INV-005 | FIFO for non-accessed entries |
| LR-004 | lru_queue.len() == entries.len() after every operation | INV-004 | Always synchronized |
| LR-005 | Queue synchronized after insert | INV-004 | Match |
| LR-006 | Queue synchronized after remove | INV-004 | Match |
| LR-007 | Queue synchronized after clear | INV-004 | Both empty |
| LR-008 | Queue synchronized after eviction | INV-004 | Match |

> **Note on INV-015/INV-016**: The contract states get and contains_key do NOT update LRU. Since the current implementation never updates LRU ordering on access (insert order is the only ordering), all accesses effectively don't update LRU. Tests LR-* verify this by checking queue state doesn't change on read operations.

---

## 11. Invariant Verification Tests

These tests explicitly verify each invariant holds after specific operations.

| ID | Invariant | Test Strategy |
|----|-----------|---------------|
| IV-001 | INV-001 | Create cache with non-existent path; verify directory exists; verify cache is usable |
| IV-002 | INV-002 | Create cache with max=0; attempt insert; verify error or rejection |
| IV-003 | INV-003 | Run 100 random insert/remove/evict cycles; assert current_memory <= max after each |
| IV-004 | INV-004 | After each mutation (insert/remove/clear/evict), assert lru_queue.len() == entries.len() |
| IV-005 | INV-005 | Insert 5 entries with total > max; trigger eviction; verify eviction order matches insertion order |
| IV-006 | INV-006 | Simulate insert failure (e.g., read-only dir); verify no partial entry in cache |
| IV-007 | INV-007 | Fill cache to max; insert entry requiring eviction of 3 entries; verify exactly 3 evicted and space available |
| IV-008 | INV-008 | Insert data; get it back; verify returned Vec<u8> matches original slice exactly |
| IV-009 | INV-009 | Insert entry; remove it; verify file deleted, entry gone from both map and queue |
| IV-010 | INV-010 | Insert 10 entries; clear; verify len=0, memory=0, all files deleted |
| IV-011 | INV-011 | Insert entries; drop cache; verify base_path has no region files |
| IV-012 | INV-012 | Insert with key "a/b:c\\d"; verify file named "a_b_c_d"; get returns correct data |
| IV-013 | INV-013 | Prefetch existing key; verify Ok; prefetch non-existent key; verify behavior documented |
| IV-014 | INV-014 | Insert 5 keys; read_ahead [valid, invalid, valid]; verify behavior per contract |
| IV-015 | INV-015 | Insert 3 keys; call contains_key on each; verify lru_queue order unchanged |
| IV-016 | INV-016 | Insert 3 keys; call get on middle key; verify lru_queue order unchanged |

---

## 12. Error Taxonomy Tests

Each error variant must be producible by at least one test.

| ID | Error Variant | Trigger Strategy |
|----|---------------|-----------------|
| ET-001 | IoError | Create cache in read-only directory (insert fails) |
| ET-002 | MmapError | Delete region file after insert, then get (file gone, mmap fails) |
| ET-003 | RegionNotFound | Get key that was never inserted |
| ET-004 | InvalidRegion | Corrupt region file to zero-length after insert, then get |
| ET-005 | CacheFull | Set max_memory_bytes=1, insert data of size > 1 (eviction can't help) |
| ET-006 | SerializationError | Not directly triggerable in current impl (no serialization); document as future-proofing |

### Error Display Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| ED-001 | Display for IoError | Formatting | Contains "IO error" |
| ED-002 | Display for MmapError | Formatting | Contains "Mmap error" |
| ED-003 | Display for RegionNotFound | Formatting | Contains "region not found" + key |
| ED-004 | Display for InvalidRegion | Formatting | Contains "invalid region" |
| ED-005 | Display for CacheFull | Formatting | Contains "cache full" |
| ED-006 | Display for SerializationError | Formatting | Contains "serialization error" |
| ED-007 | Debug format for MmapCacheError | Formatting | Compiles, shows variant |

---

## 13. Property-Based Tests (proptest)

| ID | Property | Strategy |
|----|----------|----------|
| PP-001 | **Insert preserves INV-003** | Arbitrary insert sequences; after each, assert current_memory <= max |
| PP-002 | **Insert preserves INV-004** | Arbitrary insert sequences; after each, assert queue.len() == entries.len() |
| PP-003 | **Get returns inserted data** | Arbitrary key/value pairs; insert then get; assert equality |
| PP-004 | **Remove preserves invariants** | Build cache; arbitrary remove sequence; verify INV-003, INV-004 |
| PP-005 | **Clear resets all state** | Arbitrary cache state; clear; verify len=0, memory=0 |
| PP-006 | **Memory usage accuracy** | Arbitrary insert/remove sequence; verify usage matches sum of remaining entry sizes |
| PP-007 | **Key sanitization is injective for safe keys** | Arbitrary alphanumeric keys; insert + get round-trips |
| PP-008 | **Eviction ordering is FIFO** | Fill cache; verify eviction order matches insertion order |
| PP-009 | **contains_key is pure (no side effects)** | Arbitrary cache state; contains_key; verify no state change |
| PP-010 | **Overwrite updates memory correctly** | Insert key with size A; insert same key with size B; verify usage = B not A+B |

---

## 14. Filesystem Integration Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| FS-001 | Region file created on insert | Filesystem | File exists at base_path/sanitized_key |
| FS-002 | Region file deleted on remove | Filesystem | File no longer exists |
| FS-003 | Region file deleted on eviction | Filesystem | File no longer exists |
| FS-004 | All region files deleted on clear | Filesystem | Base path contains no files |
| FS-005 | Region file size matches data length | Filesystem | File size == data.len() |
| FS-006 | Region file content matches data | Filesystem | File bytes == data bytes |
| FS-007 | Multiple caches with different base_paths don't interfere | Isolation | Independent operations |
| FS-008 | Cache works with absolute path | Path handling | Ok |
| FS-009 | Cache works with relative path | Path handling | Ok |

---

## 15. Concurrency & Safety Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| CS-001 | MmapCache is Send | Trait bound | Compiles |
| CS-002 | MmapCache is NOT Sync | Trait bound | Does not compile (negative test) |
| CS-003 | MmapCacheError is Send | Trait bound | Compiles |
| CS-004 | MmapCacheError is Sync | Trait bound | Compiles |
| CS-005 | MmapCacheBuilder is Send | Trait bound | Compiles |
| CS-006 | MmapCacheBuilder is Sync | Trait bound | Compiles |

---

## 16. Multi-Operation Sequence Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| SO-001 | Insert A, Insert B, Insert C, evict (small cache) | Sequence | A evicted, B and C remain |
| SO-002 | Insert A, Get A, Insert B (cache size=1 entry) | Sequence | A evicted (not accessed for LRU update per INV-016) |
| SO-003 | Insert A, Remove A, Insert A | Sequence | A present with latest data |
| SO-004 | Insert A, Clear, Insert A | Sequence | Works, single entry |
| SO-005 | Fill cache to max, remove all, fill again | Sequence | Second fill works identically |
| SO-006 | Insert 100 small entries in tiny cache | Stress | Cache stays within memory bound |
| SO-007 | Insert, get, remove, re-insert, get same key | Sequence | Data correct at each step |
| SO-008 | Alternating insert and remove of same key | Sequence | No memory leak, usage oscillates correctly |

---

## 17. Edge Case Tests

| ID | Test | Category | Expected |
|----|------|----------|----------|
| EC-001 | Insert with zero-length data | Boundary | Ok, get returns empty vec |
| EC-002 | Insert with single byte | Boundary | Ok |
| EC-003 | Insert with key length 0 (empty string) | Edge case | Behavior documented (may create file named "") |
| EC-004 | Insert with very long key (255 chars) | Boundary | File created, get works |
| EC-005 | Insert with key containing only special chars `///` | INV-012 | Sanitized to `___` |
| EC-006 | max_memory_bytes exactly equals one entry size | Boundary | One entry fits, second triggers eviction |
| EC-007 | max_memory_bytes = 1 byte, insert 1-byte data | Boundary | Ok |
| EC-008 | max_memory_bytes = 1 byte, insert 2-byte data | Boundary | Eviction can't help (single entry > max) |
| EC-009 | Insert then immediately get without any other ops | Happy path | Data correct |
| EC-010 | Builder with max=0, insert any data | INV-002 | Error or rejection |
| EC-011 | Drop cache while holding mmap reference from get | Edge case | No panic (mmap outlives get call) |
| EC-012 | Multiple inserts of same key with different sizes | Edge case | Memory usage reflects latest size |

---

## Test File Organization

```
crates/vo-storage/src/
  mmap_cache.rs                     # Implementation + existing basic tests
  mmap_cache_tests/
    mod.rs                          # Test module root
    construction.rs                 # CN-*, BB-* tests
    insert.rs                       # IN-*, IE-*, IA-* tests
    get.rs                          # GT-*, GE-* tests
    contains_key.rs                 # CK-* tests
    remove.rs                       # RM-*, RE-* tests
    prefetch.rs                     # PF-*, RA-* tests
    clear_drop.rs                   # CL-*, DR-* tests
    key_sanitization.rs             # KS-* tests
    memory_tracking.rs              # MT-* tests
    lru_ordering.rs                 # LR-* tests
    invariants.rs                   # IV-* tests
    error_taxonomy.rs               # ET-*, ED-* tests
    proptest.rs                     # PP-* tests
    filesystem.rs                   # FS-* tests
    concurrency_safety.rs           # CS-* tests
    sequences.rs                    # SO-* tests
    edge_cases.rs                   # EC-* tests
```

---

## Contract Deviations Found

| ID | Invariant | Issue | Severity |
|----|-----------|-------|----------|
| CD-001 | INV-014 | `read_ahead` uses `?` which returns on first error, but contract says "continues on individual errors" | HIGH — implementation bug |
| CD-002 | INV-002 | Contract says "zero limit returns error during insert" but `evict_until_space_available` always returns Ok even when no entries to evict | MEDIUM — CacheFull never returned |
| CD-003 | INV-005 | Contract says "ordered by ascending last_access" but implementation uses insertion order only (no access-time updates). This means INV-015/INV-016 are vacuously true since access time never changes. | LOW — design simplification, not a bug |

---

## Test Count Summary

| Category | Count |
|----------|-------|
| Construction & Builder | 14 |
| Insert operations | 21 |
| Get operations | 8 |
| Contains key | 5 |
| Remove operations | 10 |
| Prefetch & read-ahead | 10 |
| Clear & Drop | 11 |
| Key sanitization | 7 |
| Memory tracking | 8 |
| LRU ordering | 8 |
| Invariant verification | 16 |
| Error taxonomy | 13 |
| Property-based tests | 10 |
| Filesystem integration | 9 |
| Concurrency & safety | 6 |
| Multi-operation sequences | 8 |
| Edge cases | 12 |
| **Total** | **176** |
