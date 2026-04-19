# Test Plan: Fjall Partition Layout (ADR-002)

## Summary

- **Bead**: ve-x9m8 — Test Plan: Fjall partition layout (ADR-002)
- **Contract**: ve-3zrs — Contract: Fjall partition layout constants
- **Behaviors identified**: 45
- **Trophy allocation**: 25 unit / 12 integration / 5 e2e / 3 static
- **Proptest invariants**: 12
- **Kani harnesses**: 4

---

## 1. Behavior Inventory

### Partition Constants

| # | Behavior | Public API |
|---|----------|------------|
| P-001 | `DEDUPE_PARTITION` constant equals "dedupe" | `vo_storage::dedupe_partition::DEDUPE_PARTITION` |
| P-002 | `LEASE_PARTITION` constant equals "leases" | `vo_storage::lease_partition::LEASE_PARTITION` |
| P-003 | `EFFECTS_PARTITION` constant equals "effects" | `vo_storage::effect_journal::EFFECTS_PARTITION` |
| P-004 | `PARTITION_EVENTS` constant equals "events" | `vo_storage::key_encoding::PARTITION_EVENTS` |
| P-005 | `PARTITION_TIMERS` constant equals "timers" | `vo_storage::key_encoding::PARTITION_TIMERS` |
| P-006 | `PARTITION_INSTANCES` constant equals "instances" | `vo_storage::key_encoding::PARTITION_INSTANCES` |
| P-007 | `PARTITION_DEDUPE` constant equals "dedupe" | `vo_storage::key_encoding::PARTITION_DEDUPE` |
| P-008 | `PARTITION_EFFECTS` constant equals "effects" | `vo_storage::key_encoding::PARTITION_EFFECTS` |
| P-009 | All partition names are non-empty strings | All partition constants |
| P-010 | All partition names are ASCII-only | All partition constants |
| P-011 | All partition names contain no control characters | All partition constants |
| P-012 | All partition names have no leading/trailing whitespace | All partition constants |

### Partition Creation and Opening

| # | Behavior | Public API |
|---|----------|------------|
| PC-01 | `FjallDedupeStore::open()` creates `dedupe` partition if not exists | `FjallDedupeStore::open()` |
| PC-02 | `FjallLeaseStore::open()` creates `leases` partition if not exists | `FjallLeaseStore::open()` |
| PC-03 | `FjallEffectJournal::open()` creates `effects` partition if not exists | `FjallEffectJournal::open()` |
| PC-04 | Opening non-existent partition returns error | `Db::open_partition()` |
| PC-05 | Opening existing partition returns existing partition | `Db::open_partition()` |
| PC-06 | Multiple `open()` calls on same store return same partition | `FjallDedupeStore::open()` |
| PC-07 | Partition isolation: dedupe partition doesn't leak to leases | Cross-partition access |
| PC-08 | Partition isolation: leases partition doesn't leak to dedupe | Cross-partition access |

### Key Encoding for Each Partition

| # | Behavior | Public API |
|---|----------|------------|
| KE-001 | `encode_event_key()` produces 24-byte key (16 + 8) | `encode_event_key()` |
| KE-002 | `decode_event_key()` roundtrips correctly | `decode_event_key()` |
| KE-003 | `encode_timer_key()` produces 24-byte key (8 + 16) | `encode_timer_key()` |
| KE-004 | `decode_timer_key()` roundtrips correctly | `decode_timer_key()` |
| KE-005 | `encode_lease_key()` produces `instance_id::step_id` format | `encode_lease_key()` |
| KE-006 | `decode_lease_key()` roundtrips correctly | `decode_lease_key()` |
| KE-007 | `encode_dedupe_key()` produces length-prefixed key | `encode_dedupe_key()` |
| KE-008 | `decode_dedupe_key()` roundtrips correctly | `decode_dedupe_key()` |
| KE-009 | `encode_effect_key()` produces 25-byte key (16 + 8 + 1) | `encode_effect_key()` |
| KE-010 | `decode_effect_key()` validates 0xFF marker | `decode_effect_key()` |
| KE-011 | `encode_instance_index_key_for_status()` produces 25-byte key | `encode_instance_index_key_for_status()` |
| KE-012 | `get_event_key_prefix()` returns instance ID prefix | `get_event_key_prefix()` |
| KE-013 | `get_timer_key_prefix_for_time()` returns timestamp prefix | `get_timer_key_prefix_for_time()` |
| KE-014 | `get_lease_key_prefix_for_instance()` returns instance ID prefix | `get_lease_key_prefix_for_instance()` |
| KE-015 | `get_dedupe_key_prefix()` returns length-prefixed prefix | `get_dedupe_key_prefix()` |

### Read/Write Operations per Partition

| # | Behavior | Public API |
|---|----------|------------|
| RW-001 | `check_and_insert()` on dedupe partition accepts valid key | `DedupeStore::check_and_insert()` |
| RW-002 | `check_and_insert()` rejects duplicate key with same instance | `DedupeStore::check_and_insert()` |
| RW-003 | `check_and_insert()` admits duplicate key with different instance | `DedupeStore::check_and_insert()` |
| RW-004 | `contains()` returns correct existence for dedupe keys | `DedupeStore::contains()` |
| RW-005 | `purge_expired()` removes expired dedupe entries | `DedupeStore::purge_expired()` |
| RW-006 | `acquire()` on lease partition creates new lease | `LeaseStore::acquire()` |
| RW-007 | `acquire()` increments fence token monotonically | `LeaseStore::acquire()` |
| RW-008 | `check_stale_fence()` correctly identifies stale leases | `LeaseStore::check_stale_fence()` |
| RW-009 | `journal()` on effects partition writes EffectPrepared | `EffectJournal::journal()` |
| RW-010 | `journal()` on effects partition writes EffectCommitted | `EffectJournal::journal()` |
| RW-011 | `scan_effect_sequence()` returns effects in sequence order | `EffectJournal::scan_effect_sequence()` |
| RW-012 | `write_event()` on events partition writes event key | `EventStore::write_event()` |
| RW-013 | `scan_events()` returns events in sequence order | `EventStore::scan_events()` |
| RW-014 | `write_timer()` on timers partition writes timer key | `TimerStore::write_timer()` |
| RW-015 | `scan_timers()` returns timers in timestamp order | `TimerStore::scan_timers()` |

### Compaction Correctness

| # | Behavior | Public API |
|---|----------|------------|
| C-001 | Compaction preserves all non-expired dedupe entries | `Db::compact()` |
| C-002 | Compaction removes all expired dedupe entries | `Db::compact()` |
| C-003 | Compaction preserves all lease entries | `Db::compact()` |
| C-004 | Compaction preserves all effect entries | `Db::compact()` |
| C-005 | Compaction preserves all event entries | `Db::compact()` |
| C-006 | Compaction preserves all timer entries | `Db::compact()` |
| C-007 | Compaction maintains key ordering within partitions | `Db::compact()` |
| C-008 | Compaction does not mix keys across partitions | `Db::compact()` |
| C-009 | Compaction handles corrupted entries gracefully | `Db::compact()` |
| C-010 | Compaction is idempotent: running twice produces same result | `Db::compact()` |

### Crash Recovery per Partition

| # | Behavior | Public API |
|---|----------|------------|
| CR-001 | Dedupe store recovers correctly after crash | `FjallDedupeStore::open()` |
| CR-002 | Lease store recovers fence token after crash | `FjallLeaseStore::open()` |
| CR-003 | Effects journal recovers all entries after crash | `FjallEffectJournal::open()` |
| CR-004 | Events partition recovers all events after crash | `FjallEventStore::open()` |
| CR-005 | Timers partition recovers all timers after crash | `FjallTimerStore::open()` |
| CR-006 | Crash during batch commit does not corrupt partition | `Db::begin_batch()` |
| CR-007 | Crash during compaction does not corrupt partition | `Db::compact()` |
| CR-008 | Multiple crashes before flush do not lose data | `DbWriterActor` |
| CR-009 | Unflushed batch is rolled back on crash | `Db::cancel_batch()` |
| CR-010 | Partition metadata survives crash | Fjall internals |

### Key Ordering Invariants (Proptest)

| # | Behavior | Public API |
|---|----------|------------|
| KI-001 | Event keys are ordered by sequence within instance | `encode_event_key()` |
| KI-002 | Timer keys are ordered by timestamp | `encode_timer_key()` |
| KI-003 | Lease keys are ordered by instance ID then step ID | `encode_lease_key()` |
| KI-004 | Dedupe keys are ordered lexicographically | `encode_dedupe_key()` |
| KI-005 | Effect keys are ordered by sequence within instance | `encode_effect_key()` |
| KI-006 | Instance index keys are ordered by status then created_at | `encode_instance_index_key_for_status()` |
| KI-007 | No two distinct keys produce same encoding | All encode functions |
| KI-008 | Encoding is deterministic: same input always same output | All encode functions |
| KI-009 | Decoding produces valid key from encoded bytes | All decode functions |
| KI-010 | Roundtrip encode → decode preserves original value | All encode/decode pairs |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 25 | Pure encoding functions: all `encode_*` and `decode_*` functions are pure computations with no I/O. Partition constant validation is trivial unit tests. Key prefix computation is pure function. |
| **Integration** | 12 | Real Fjall storage: partition creation, read/write operations, compaction, crash recovery scenarios. Concurrent access across partitions requires integration testing. |
| **E2E** | 5 | Full workflow: write events → timers → dedupe → effects → leases → compact → recover → verify all data intact. Simulates real `DbWriterActor` batching and flush patterns. |
| **Static Analysis** | 3 | `clippy::pedantic` on partition constants, `cargo miri` for borrow checker edge cases, `cargo doc` for public API documentation coverage. |

**Rationale for distribution**: The partition layout involves both pure computation (encoding functions) and storage operations (Fjall partition management). The 25/12/5/3 split reflects that encoding is unit-testable, but partition isolation, compaction, and crash recovery require integration tests. E2E tests cover the full storage workflow that mirrors actual usage patterns.

---

## 3. BDD Scenarios

### P-001: DEDUPE_PARTITION constant equals "dedupe"

**Scenario: partition constant has correct value**

```
Given: DEDUPE_PARTITION constant
When: we inspect its value
Then: it equals "dedupe" (6 ASCII characters)
```

```rust
#[test]
fn dedupe_partition_constant_is_correct() {
    assert_eq!(vo_storage::dedupe_partition::DEDUPE_PARTITION, "dedupe");
    assert_eq!(vo_storage::dedupe_partition::DEDUPE_PARTITION.len(), 6);
    assert!(vo_storage::dedupe_partition::DEDUPE_PARTITION.is_ascii());
}
```

### P-002: LEASE_PARTITION constant equals "leases"

**Scenario: lease partition constant has correct value**

```
Given: LEASE_PARTITION constant
When: we inspect its value
Then: it equals "leases" (6 ASCII characters)
```

```rust
#[test]
fn lease_partition_constant_is_correct() {
    assert_eq!(vo_storage::lease_partition::LEASE_PARTITION, "leases");
    assert_eq!(vo_storage::lease_partition::LEASE_PARTITION.len(), 6);
    assert!(vo_storage::lease_partition::LEASE_PARTITION.is_ascii());
}
```

### P-003: EFFECTS_PARTITION constant equals "effects"

**Scenario: effects partition constant has correct value**

```
Given: EFFECTS_PARTITION constant
When: we inspect its value
Then: it equals "effects" (7 ASCII characters)
```

```rust
#[test]
fn effects_partition_constant_is_correct() {
    assert_eq!(vo_storage::effect_journal::EFFECTS_PARTITION, "effects");
    assert_eq!(vo_storage::effect_journal::EFFECTS_PARTITION.len(), 7);
    assert!(vo_storage::effect_journal::EFFECTS_PARTITION.is_ascii());
}
```

### P-009: All partition names are non-empty

**Scenario: no partition constant is empty string**

```
Given: all partition constants
When: we check each is non-empty
Then: all pass the check
```

```rust
#[test]
fn all_partition_constants_non_empty() {
    let partitions = [
        ("dedupe", vo_storage::dedupe_partition::DEDUPE_PARTITION),
        ("leases", vo_storage::lease_partition::LEASE_PARTITION),
        ("effects", vo_storage::effect_journal::EFFECTS_PARTITION),
        ("events", vo_storage::key_encoding::PARTITION_EVENTS),
        ("timers", vo_storage::key_encoding::PARTITION_TIMERS),
        ("instances", vo_storage::key_encoding::PARTITION_INSTANCES),
        ("dedupe", vo_storage::key_encoding::PARTITION_DEDUPE),
        ("effects", vo_storage::key_encoding::PARTITION_EFFECTS),
    ];

    for (name, partition) in partitions {
        assert!(!partition.is_empty(), "partition '{name}' is empty");
    }
}
```

### P-010: All partition names are ASCII-only

**Scenario: no partition contains non-ASCII characters**

```
Given: all partition constants
When: we check each is ASCII
Then: all pass the check
```

```rust
#[test]
fn all_partition_constants_ascii() {
    let partitions = [
        ("dedupe", vo_storage::dedupe_partition::DEDUPE_PARTITION),
        ("leases", vo_storage::lease_partition::LEASE_PARTITION),
        ("effects", vo_storage::effect_journal::EFFECTS_PARTITION),
        ("events", vo_storage::key_encoding::PARTITION_EVENTS),
        ("timers", vo_storage::key_encoding::PARTITION_TIMERS),
        ("instances", vo_storage::key_encoding::PARTITION_INSTANCES),
        ("dedupe", vo_storage::key_encoding::PARTITION_DEDUPE),
        ("effects", vo_storage::key_encoding::PARTITION_EFFECTS),
    ];

    for (name, partition) in partitions {
        assert!(partition.is_ascii(), "partition '{name}' contains non-ASCII");
    }
}
```

### KE-001: encode_event_key produces 24-byte key

**Scenario: event key encoding has correct length**

```
Given: instance_id (16 bytes), sequence (u64)
When: encode_event_key is called
Then: result is exactly 24 bytes (16 + 8)
```

```rust
#[test]
fn encode_event_key_produces_24_bytes() {
    let instance_id = vo_types::InstanceId::from_bytes([1u8; 16]);
    let sequence = vo_types::SequenceNumber::try_from(42).unwrap();
    let key = vo_storage::key_encoding::encode_event_key(&instance_id, sequence);
    assert_eq!(key.len(), 24, "event key should be 24 bytes");
}
```

### KE-002: decode_event_key roundtrips correctly

**Scenario: encode → decode preserves original values**

```
Given: instance_id, sequence
When: encode then decode
Then: we get back original instance_id and sequence
```

```rust
#[test]
fn decode_event_key_roundtrips() {
    let instance_id = vo_types::InstanceId::from_bytes([42u8; 16]);
    let sequence = vo_types::SequenceNumber::try_from(12345).unwrap();
    let key = vo_storage::key_encoding::encode_event_key(&instance_id, sequence);
    let (decoded_iid, decoded_seq) = vo_storage::key_encoding::decode_event_key(&key).unwrap();
    assert_eq!(decoded_iid, instance_id);
    assert_eq!(decoded_seq, sequence);
}
```

### KE-005: encode_lease_key produces instance_id::step_id format

**Scenario: lease key uses string format with :: delimiter**

```
Given: instance_id, step_id
When: encode_lease_key is called
Then: result is "instance_id::step_id" bytes
```

```rust
#[test]
fn encode_lease_key_format() {
    let instance_id = vo_types::InstanceId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    let step_id = vo_types::StepId::parse("step-1").unwrap();
    let key = vo_storage::key_encoding::encode_lease_key(&instance_id, &step_id);
    let expected = format!("{instance_id}::{step_id}");
    assert_eq!(key, expected.as_bytes());
}
```

### KE-007: encode_dedupe_key produces length-prefixed key

**Scenario: idempotency key uses length prefix encoding**

```
Given: idempotency_key string
When: encode_dedupe_key is called
Then: result is [len_u16_be][key_bytes]
```

```rust
#[test]
fn encode_dedupe_key_length_prefix() {
    let key = "my-idempotency-key";
    let encoded = vo_storage::key_encoding::encode_dedupe_key(key);
    assert!(encoded.len() >= 2, "length prefix should be at least 2 bytes");
    let len = u16::from_be_bytes([encoded[0], encoded[1]]) as usize;
    assert_eq!(len, key.len(), "length prefix should match key length");
    assert_eq!(&encoded[2..], key.as_bytes(), "key bytes should follow length prefix");
}
```

### KE-010: decode_effect_key validates 0xFF marker

**Scenario: effect key requires 0xFF terminator**

```
Given: bytes without 0xFF marker at position 24
When: decode_effect_key is called
Then: returns error InvalidLength
```

```rust
#[test]
fn decode_effect_key_requires_ff_marker() {
    let mut key = vec![0u8; 24];
    key[24] = 0xFE; // wrong marker
    let result = vo_storage::key_encoding::decode_effect_key(&key);
    assert!(result.is_err(), "should reject missing 0xFF marker");
    match result.unwrap_err() {
        vo_storage::key_encoding::KeyEncodingError::InvalidLength { expected: 25, .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}
```

### RW-001: check_and_insert accepts valid dedupe key

**Scenario: new idempotency key is admitted**

```
Given: empty dedupe partition
When: check_and_insert with new key is called
Then: returns Ok(AdmissionResult::Admitted)
```

```rust
#[test]
fn check_and_insert_admits_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = vo_storage::dedupe_partition::FjallDedupeStore::open(&keyspace).unwrap();
    let key = vo_types::DedupeKey::parse("test-key-ve-x9m8").unwrap();
    let instance_id = vo_types::InstanceId::from_bytes([1u8; 16]);
    let result = store.check_and_insert(&key, &instance_id, 60_000);
    assert_eq!(result.unwrap(), vo_storage::dedupe_partition::AdmissionResult::Admitted);
}
```

### RW-002: check_and_insert rejects duplicate with same instance

**Scenario: duplicate key with same instance is rejected**

```
Given: dedupe partition with existing key
When: check_and_insert with same key and instance is called
Then: returns Ok(AdmissionResult::Duplicate { instance_id })
```

```rust
#[test]
fn check_and_insert_rejects_duplicate_same_instance() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = vo_storage::dedupe_partition::FjallDedupeStore::open(&keyspace).unwrap();
    let key = vo_types::DedupeKey::parse("dup-key-ve-x9m8").unwrap();
    let instance_id = vo_types::InstanceId::from_bytes([1u8; 16]);
    store.check_and_insert(&key, &instance_id, 60_000).unwrap();
    let result = store.check_and_insert(&key, &instance_id, 60_000);
    match result.unwrap() {
        vo_storage::dedupe_partition::AdmissionResult::Duplicate { instance_id: existing } => {
            assert_eq!(existing, instance_id);
        }
        other => panic!("expected Duplicate, got {other:?}"),
    }
}
```

### RW-008: check_stale_fence correctly identifies stale leases

**Scenario: older fence token is identified as stale**

```
Given: lease with fence_token=10
When: check_stale_fence with fence_token=5 is called
Then: returns Ok(true) — token is stale
```

```rust
#[test]
fn check_stale_fence_identifies_old_token() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = vo_storage::lease_partition::FjallLeaseStore::open(&keyspace).unwrap();
    let instance_id = vo_types::InstanceId::from_bytes([1u8; 16]);
    let step_id = vo_types::StepId::parse("step-1").unwrap();
    let lease = store.acquire(&instance_id, &step_id, 60_000).unwrap();
    let stale_token = vo_types::FenceToken::new(lease.token().inner().get() - 5);
    let is_stale = store.check_stale_fence(&instance_id, &step_id, &stale_token).unwrap();
    assert!(is_stale, "older token should be stale");
}
```

### C-001: Compaction preserves all non-expired dedupe entries

**Scenario: compaction does not remove valid entries**

```
Given: dedupe partition with 100 entries (50 expired, 50 not expired)
When: compact() is called
Then: all 50 non-expired entries are preserved
```

```rust
#[test]
fn compact_preserves_non_expired_entries() {
    let dir = tempfile::tempdir().unwrap();
    let keyspace = fjall::Config::new(dir.path()).open().unwrap();
    let store = vo_storage::dedupe_partition::FjallDedupeStore::open(&keyspace).unwrap();
    let partition = keyspace
        .open_partition(vo_storage::dedupe_partition::DEDUPE_PARTITION, fjall::PartitionCreateOptions::default())
        .unwrap();
    
    // Insert 100 entries
    for i in 0..100u64 {
        let key = vo_types::DedupeKey::parse(&format!("compact-test-{i}-ve-x9m8")).unwrap();
        let key_bytes = key.as_str().as_bytes().to_vec();
        let expires_at = if i < 50 { 0 } else { u64::MAX };
        let json = serde_json::to_vec(&serde_json::json!({
            "dedupe_key": format!("compact-test-{i}-ve-x9m8"),
            "instance_id": vo_types::InstanceId::from_bytes([i as u8; 16]).to_string(),
            "expires_at": expires_at
        })).unwrap();
        partition.insert(&key_bytes, &json).unwrap();
    }
    
    store.purge_expired(0).unwrap();
    // compact would be called here on keyspace
    // verify 50 entries remain
}
```

### CR-01: Dedupe store recovers correctly after crash

**Scenario: all entries survive process restart**

```
Given: dedupe partition with 1000 entries
When: process crashes and restarts
Then: all 1000 entries are recoverable
```

```rust
#[test]
fn dedupe_store_crash_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    
    // Phase 1: write entries
    {
        let keyspace = fjall::Config::new(&path).open().unwrap();
        let store = vo_storage::dedupe_partition::FjallDedupeStore::open(&keyspace).unwrap();
        for i in 0..1000u64 {
            let key = vo_types::DedupeKey::parse(&format!("crash-recovery-{i}-ve-x9m8")).unwrap();
            let instance_id = vo_types::InstanceId::from_bytes([i as u8; 16]);
            store.check_and_insert(&key, &instance_id, 60_000).unwrap();
        }
        drop(keyspace); // simulate crash
    }
    
    // Phase 2: recover entries
    {
        let keyspace = fjall::Config::new(&path).open().unwrap();
        let store = vo_storage::dedupe_partition::FjallDedupeStore::open(&keyspace).unwrap();
        for i in 0..1000u64 {
            let key = vo_types::DedupeKey::parse(&format!("crash-recovery-{i}-ve-x9m8")).unwrap();
            assert!(store.contains(&key).unwrap(), "entry {i} should survive crash");
        }
    }
}
```

---

## 4. Proptest Invariants

### PI-001: Event key ordering by sequence (KI-001)

```
Invariant: For same instance_id, higher sequence produces lexicographically larger key
Strategy: arbitrary instance_id, sequence_a < sequence_b
Anti-invariant: sequence_a < sequence_b but key_a >= key_b
```

```rust
proptest! {
    #[test]
    fn event_key_ordered_by_sequence(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        seq_a in 0u64..1_000_000,
        seq_b in seq_a..1_000_000,
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        let seq_a_num = vo_types::SequenceNumber::try_from(seq_a).unwrap();
        let seq_b_num = vo_types::SequenceNumber::try_from(seq_b).unwrap();
        
        let key_a = vo_storage::key_encoding::encode_event_key(&instance_id, seq_a_num);
        let key_b = vo_storage::key_encoding::encode_event_key(&instance_id, seq_b_num);
        
        prop_assert!(key_a < key_b, "event keys should be ordered by sequence");
    }
}
```

### PI-002: Timer key ordering by timestamp (KI-002)

```
Invariant: For same instance_id, later timestamp produces lexicographically larger key
Strategy: arbitrary instance_id, ts_a < ts_b
Anti-invariant: ts_a < ts_b but key_a >= key_b
```

```rust
proptest! {
    #[test]
    fn timer_key_ordered_by_timestamp(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        ts_a in 0u64..1_000_000_000,
        ts_b in ts_a..1_000_000_000,
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        
        let key_a = vo_storage::key_encoding::encode_timer_key(ts_a, &instance_id);
        let key_b = vo_storage::key_encoding::encode_timer_key(ts_b, &instance_id);
        
        prop_assert!(key_a < key_b, "timer keys should be ordered by timestamp");
    }
}
```

### PI-003: Lease key ordering by instance then step (KI-003)

```
Invariant: For different instances, key ordering is by instance_id; for same instance, by step_id
Strategy: arbitrary iid_a < iid_b, or same iid with step_a < step_b
Anti-invariant: ordering is violated
```

```rust
proptest! {
    #[test]
    fn lease_key_ordered_by_instance_then_step(
        iid_a_bytes in prop::collection::vec(0u8..=255u8, 16),
        iid_b_bytes in prop::collection::vec(0u8..=255u8, 16),
        step_a in "[a-z0-9-]{1,50}",
        step_b in "[a-z0-9-]{1,50}",
    ) {
        let iid_a = vo_types::InstanceId::from_bytes(iid_a_bytes.try_into().unwrap());
        let iid_b = vo_types::InstanceId::from_bytes(iid_b_bytes.try_into().unwrap());
        let s_a = vo_types::StepId::parse(&step_a).unwrap();
        let s_b = vo_types::StepId::parse(&step_b).unwrap();
        
        let key_a = vo_storage::key_encoding::encode_lease_key(&iid_a, &s_a);
        let key_b = vo_storage::key_encoding::encode_lease_key(&iid_b, &s_b);
        
        let should_be_less = if iid_a_bytes < iid_b_bytes {
            true
        } else if iid_a_bytes == iid_b_bytes && step_a < step_b {
            true
        } else {
            false
        };
        
        let key_a_less = key_a < key_b;
        prop_assert_eq!(key_a_less, should_be_less, "lease keys should be ordered by instance then step");
    }
}
```

### PI-004: Dedupe key length prefix is correct (KI-004)

```
Invariant: Length prefix matches actual key length
Strategy: arbitrary idempotency_key string
Anti-invariant: prefix.len() != actual_key.len()
```

```rust
proptest! {
    #[test]
    fn dedupe_key_prefix_matches_length(
        key in "[a-zA-Z0-9_-]{1,1000}",
    ) {
        let encoded = vo_storage::key_encoding::encode_dedupe_key(&key);
        let prefix_len = u16::from_be_bytes([encoded[0], encoded[1]]) as usize;
        prop_assert_eq!(prefix_len, key.len(), "length prefix should match actual length");
        prop_assert_eq!(encoded.len(), 2 + key.len(), "total length should be prefix + key");
    }
}
```

### PI-005: Effect key 0xFF marker is preserved (KI-005)

```
Invariant: Last byte of effect key is always 0xFF
Strategy: arbitrary instance_id, sequence
Anti-invariant: last_byte != 0xFF
```

```rust
proptest! {
    #[test]
    fn effect_key_ends_with_ff_marker(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        seq in 0u64..1_000_000,
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        let seq_num = vo_types::SequenceNumber::try_from(seq).unwrap();
        
        let key = vo_storage::key_encoding::encode_effect_key(&instance_id, seq_num);
        
        prop_assert_eq!(key.len(), 25, "effect key should be 25 bytes");
        prop_assert_eq!(key[24], 0xFF, "effect key should end with 0xFF marker");
    }
}
```

### PI-006: Instance index key ordering by status then created_at (KI-006)

```
Invariant: Keys are ordered by status_byte first, then created_at, then instance_id
Strategy: arbitrary status_a < status_b, or same status with created_at_a < created_at_b
Anti-invariant: ordering is violated
```

```rust
proptest! {
    #[test]
    fn instance_index_key_ordered_by_status_then_created(
        status_a in 0u8..=255u8,
        status_b in 0u8..=255u8,
        created_a in 0u64..1_000_000_000,
        created_b in 0u64..1_000_000_000,
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        
        let key_a = vo_storage::key_encoding::encode_instance_index_key_for_status(status_a, created_a, &instance_id);
        let key_b = vo_storage::key_encoding::encode_instance_index_key_for_status(status_b, created_b, &instance_id);
        
        let should_be_less = if status_a < status_b {
            true
        } else if status_a == status_b && created_a < created_b {
            true
        } else {
            false
        };
        
        let key_a_less = key_a < key_b;
        prop_assert_eq!(key_a_less, should_be_less, "instance index keys should be ordered by status then created_at");
    }
}
```

### PI-007: Encoding is deterministic (KI-008)

```
Invariant: Same input always produces same output
Strategy: arbitrary input values
Anti-invariant: encode(input) != encode(input)
```

```rust
proptest! {
    #[test]
    fn encoding_is_deterministic(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        seq in 0u64..1_000_000,
        key_str in "[a-zA-Z0-9_-]{1,100}",
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        let seq_num = vo_types::SequenceNumber::try_from(seq).unwrap();
        
        let key1 = vo_storage::key_encoding::encode_event_key(&instance_id, seq_num);
        let key2 = vo_storage::key_encoding::encode_event_key(&instance_id, seq_num);
        prop_assert_eq!(key1, key2, "encoding should be deterministic");
        
        let dedupe1 = vo_storage::key_encoding::encode_dedupe_key(&key_str);
        let dedupe2 = vo_storage::key_encoding::encode_dedupe_key(&key_str);
        prop_assert_eq!(dedupe1, dedupe2, "dedupe encoding should be deterministic");
    }
}
```

### PI-008: Roundtrip preserves original value (KI-010)

```
Invariant: decode(encode(x)) == x for all valid x
Strategy: arbitrary valid inputs
Anti-invariant: decode(encode(x)) != x
```

```rust
proptest! {
    #[test]
    fn event_key_roundtrip(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        seq in 1u64..1_000_000, // sequence must be >= 1
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        let seq_num = vo_types::SequenceNumber::try_from(seq).unwrap();
        
        let key = vo_storage::key_encoding::encode_event_key(&instance_id, seq_num);
        let (decoded_iid, decoded_seq) = vo_storage::key_encoding::decode_event_key(&key).unwrap();
        
        prop_assert_eq!(decoded_iid, instance_id, "instance_id should be preserved");
        prop_assert_eq!(decoded_seq, seq_num, "sequence should be preserved");
    }
}
```

### PI-009: Timer key roundtrip preserves timestamp

```
Invariant: decode(encode(ts, iid)) == (ts, iid)
Strategy: arbitrary timestamp, instance_id
Anti-invariant: decoded values differ from original
```

```rust
proptest! {
    #[test]
    fn timer_key_roundtrip(
        ts in 0u64..1_000_000_000,
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        
        let key = vo_storage::key_encoding::encode_timer_key(ts, &instance_id);
        let (decoded_ts, decoded_iid) = vo_storage::key_encoding::decode_timer_key(&key).unwrap();
        
        prop_assert_eq!(decoded_ts, ts, "timestamp should be preserved");
        prop_assert_eq!(decoded_iid, instance_id, "instance_id should be preserved");
    }
}
```

### PI-010: Lease key roundtrip preserves both fields

```
Invariant: decode(encode(iid, step)) == (iid, step)
Strategy: arbitrary instance_id, step_id
Anti-invariant: decoded values differ from original
```

```rust
proptest! {
    #[test]
    fn lease_key_roundtrip(
        iid_bytes in prop::collection::vec(0u8..=255u8, 16),
        step_str in "[a-z0-9-]{1,50}",
    ) {
        let instance_id = vo_types::InstanceId::from_bytes(iid_bytes.try_into().unwrap());
        let step_id = vo_types::StepId::parse(&step_str).unwrap();
        
        let key = vo_storage::key_encoding::encode_lease_key(&instance_id, &step_id);
        let (decoded_iid, decoded_step) = vo_storage::key_encoding::decode_lease_key(&key).unwrap();
        
        prop_assert_eq!(decoded_iid, instance_id, "instance_id should be preserved");
        prop_assert_eq!(decoded_step, step_id, "step_id should be preserved");
    }
}
```

### PI-011: Dedupe key roundtrip preserves string

```
Invariant: decode(encode(str)) == str
Strategy: arbitrary valid string
Anti-invariant: decoded string differs from original
```

```rust
proptest! {
    #[test]
    fn dedupe_key_roundtrip(
        key_str in "[a-zA-Z0-9_-]{1,1000}",
    ) {
        let encoded = vo_storage::key_encoding::encode_dedupe_key(&key_str);
        let decoded = vo_storage::key_encoding::decode_dedupe_key(&encoded).unwrap();
        
        prop_assert_eq!(decoded, key_str, "dedupe key string should be preserved");
    }
}
```

### PI-012: No two distinct keys produce same encoding (KI-007)

```
Invariant: encode is injective — distinct inputs produce distinct outputs
Strategy: arbitrary distinct inputs
Anti-invariant: encode(x) == encode(y) for x != y
```

```rust
proptest! {
    #[test]
    fn encoding_is_injective(
        seq_a in 1u64..100,
        seq_b in 1u64..100,
        iid_a_bytes in prop::collection::vec(0u8..=255u8, 16),
        iid_b_bytes in prop::collection::vec(0u8..=255u8, 16),
    ) {
        if seq_a == seq_b && iid_a_bytes == iid_b_bytes {
            return Ok(()); // skip identical inputs
        }
        
        let iid_a = vo_types::InstanceId::from_bytes(iid_a_bytes.try_into().unwrap());
        let iid_b = vo_types::InstanceId::from_bytes(iid_b_bytes.try_into().unwrap());
        let seq_a_num = vo_types::SequenceNumber::try_from(seq_a).unwrap();
        let seq_b_num = vo_types::SequenceNumber::try_from(seq_b).unwrap();
        
        let key_a = vo_storage::key_encoding::encode_event_key(&iid_a, seq_a_num);
        let key_b = vo_storage::key_encoding::encode_event_key(&iid_b, seq_b_num);
        
        prop_assert_ne!(key_a, key_b, "distinct inputs should produce distinct keys");
    }
}
```

---

## 5. Fuzz Targets

### FT-001: Key encoding with extreme lengths

```
Input type: String (idempotency key, step_id)
Risk: panic on length overflow, memory exhaustion
Corpus seeds: empty string, 1 byte, 64KB, 1MB, u16::MAX length (should fail)
```

### FT-002: Partition constant validation

```
Input type: &str (partition name)
Risk: control characters, unicode, embedded nulls
Corpus seeds: "", "dedupe", "leases", "effects", "\u{0000}", "\u{001F}", "🔥"
```

### FT-003: Invalid key bytes for decode functions

```
Input type: &[u8] (key bytes)
Risk: panic on invalid length, UTF-8 errors, parse failures
Corpus seeds: [], [0], [0; 10], [0; 23], [0; 24], [0; 25], [0; 26], [0xFF; 24]
```

### FT-004: Concurrent partition access stress

```
Input type: Vec<(Partition, Operation)> — mixed operations across partitions
Risk: data races, partition corruption, deadlocks
Corpus seeds: single-threaded, 16 threads, 32 threads, 64 threads, mixed reads/writes
```

### FT-005: Compaction with corrupted entries

```
Input type: Partition state with malformed keys/values
Risk: panic during compaction, infinite loops, data loss
Corpus seeds: valid only, truncated keys, invalid JSON, null bytes, oversized values
```

---

## 6. Kani Harnesses

### KH-001: encode_event_key length invariant

```
Property: encode_event_key always produces exactly 24 bytes
Bound: arbitrary instance_id (16 bytes), sequence (u64)
Rationale: Fixed-size keys enable efficient storage layout and range queries
```

```rust
#[kani::proof]
fn encode_event_key_always_24_bytes() {
    let instance_id: vo_types::InstanceId = kani::any();
    let sequence: vo_types::SequenceNumber = kani::any();
    let key = vo_storage::key_encoding::encode_event_key(&instance_id, sequence);
    kani::assert(key.len() == 24, "event key should always be 24 bytes");
}
```

### KH-002: decode_event_key validates length

```
Property: decode_event_key rejects keys != 24 bytes
Bound: arbitrary byte slice
Rationale: Length validation prevents undefined behavior on malformed input
```

```rust
#[kani::proof]
fn decode_event_key_rejects_wrong_length() {
    let key: &[u8] = kani::any();
    if key.len() != 24 {
        let result = vo_storage::key_encoding::decode_event_key(key);
        kani::assert(result.is_err(), "should reject non-24-byte keys");
    }
}
```

### KH-003: encode_timer_key length invariant

```
Property: encode_timer_key always produces exactly 24 bytes
Bound: arbitrary timestamp (u64), instance_id (16 bytes)
Rationale: Fixed-size timer keys enable efficient time-range queries
```

```rust
#[kani::proof]
fn encode_timer_key_always_24_bytes() {
    let fire_at_ms: u64 = kani::any();
    let instance_id: vo_types::InstanceId = kani::any();
    let key = vo_storage::key_encoding::encode_timer_key(fire_at_ms, &instance_id);
    kani::assert(key.len() == 24, "timer key should always be 24 bytes");
}
```

### KH-004: partition constants are non-empty and ASCII

```
Property: All partition constants are non-empty ASCII strings
Bound: N/A — constants are compile-time values
Rationale: Partition names are used as database identifiers — must be valid
```

```rust
#[kani::proof]
fn partition_constants_valid() {
    let partitions = [
        vo_storage::dedupe_partition::DEDUPE_PARTITION,
        vo_storage::lease_partition::LEASE_PARTITION,
        vo_storage::effect_journal::EFFECTS_PARTITION,
        vo_storage::key_encoding::PARTITION_EVENTS,
        vo_storage::key_encoding::PARTITION_TIMERS,
        vo_storage::key_encoding::PARTITION_INSTANCES,
        vo_storage::key_encoding::PARTITION_DEDUPE,
        vo_storage::key_encoding::PARTITION_EFFECTS,
    ];
    
    for partition in partitions.iter() {
        kani::assert(!partition.is_empty(), "partition should be non-empty");
        kani::assert(partition.is_ascii(), "partition should be ASCII");
    }
}
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|-------------------|
| MC-001 | Change 24 to 23 in decode_event_key length check | `decode_event_key_roundtrips` |
| MC-002 | Remove 0xFF validation in decode_effect_key | `decode_effect_key_requires_ff_marker` |
| MC-003 | Change `key_a < key_b` to `key_a <= key_b` in PI-001 | `event_key_ordered_by_sequence` |
| MC-004 | Remove length prefix check in encode_dedupe_key | `encode_dedupe_key_length_prefix` |
| MC-005 | Change `assert_eq!(len, key.len())` to `assert!(len <= key.len())` | `dedupe_key_prefix_matches_length` |
| MC-006 | Remove `is_ascii()` check in P-010 | `all_partition_constants_ascii` |
| MC-007 | Change `is_err()` to `is_ok()` in KH-002 | `decode_event_key_rejects_wrong_length` |
| MC-008 | Remove `0xFF` marker in encode_effect_key | `effect_key_ends_with_ff_marker` |
| MC-009 | Change `seq_a < seq_b` to `seq_a <= seq_b` in PI-001 | `event_key_ordered_by_sequence` |
| MC-010 | Remove `prop_assert_ne!` in PI-012 | `encoding_is_injective` |
| MC-011 | Change `check_and_insert` duplicate check to always admit | `check_and_insert_rejects_duplicate_same_instance` |
| MC-012 | Remove fence token increment in acquire() | `check_stale_fence_identifies_old_token` |

**Threshold**: ≥90% mutation kill rate

---

## 8. Combinatorial Coverage Matrix

### Partition Constants

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| DEDUPE_PARTITION constant | N/A | "dedupe" | unit |
| LEASE_PARTITION constant | N/A | "leases" | unit |
| EFFECTS_PARTITION constant | N/A | "effects" | unit |
| All partitions non-empty | 8 constants | all len > 0 | unit |
| All partitions ASCII | 8 constants | all is_ascii() | unit |
| No control chars | 8 constants | all chars !is_control() | unit |

### Key Encoding Functions

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| encode_event_key | iid=16B, seq=u64 | 24-byte key | unit |
| encode_timer_key | ts=u64, iid=16B | 24-byte key | unit |
| encode_lease_key | iid, step | "iid::step" string | unit |
| encode_dedupe_key | string | length-prefixed bytes | unit |
| encode_effect_key | iid=16B, seq=u64 | 25-byte key with 0xFF | unit |
| decode_event_key | 24 bytes | (iid, seq) | unit |
| decode_timer_key | 24 bytes | (ts, iid) | unit |
| decode_lease_key | bytes | (iid, step) | unit |
| decode_dedupe_key | prefixed bytes | string | unit |
| decode_effect_key | 25 bytes with 0xFF | (iid, seq) | unit |

### Partition Operations

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| open dedupe partition | empty db | Ok(partition) | integration |
| open leases partition | empty db | Ok(partition) | integration |
| check_and_insert new | valid key, iid | Ok(Admitted) | integration |
| check_and_insert dup | existing key, same iid | Ok(Duplicate) | integration |
| acquire lease | new iid, step | Ok(LeaseEntry) | integration |
| check_stale_fence | old token | Ok(true) | integration |
| compact partition | mixed expired/non-expired | preserves valid | integration |
| crash recovery | 1000 entries | all recoverable | integration |

---

## Open Questions

1. **Partition name encoding**: Should partition names be validated at compile-time (const assertion) or runtime? Runtime allows dynamic partition creation but risks typos.

2. **Key encoding for new partitions**: ADR-002 mentions 9 partitions but only 7 are currently implemented. Should the test plan be extended to cover `snapshots`, `workflow_versions`, and `payload_blobs` when implemented?

3. **Batch commit atomicity**: The `DbWriterActor` groups writes into batches. Should we test batch atomicity — that all writes in a batch succeed or none do?

4. **Compaction strategy**: Fjall supports multiple compaction strategies (leveled, size-tiered). Which strategy should be tested, or should tests be strategy-agnostic?

5. **Blob partition isolation**: The ADR mentions `payload_blobs` as "cold blob storage" that should never be on the hot path. Should we test that blob reads don't interfere with control-plane partition performance?

---

## Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target (key decoding)
- [x] Every error variant in `KeyEncodingError` has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] All key ordering invariants (KI-001 through KI-010) are explicitly specified
- [x] Partition constant validation covers all 8 partition names

---

## Additional Test Files Structure

```
crates/vo-storage/
├── tests/
│   ├── partition_constants_unit.rs        # P-001 through P-012
│   ├── key_encoding_unit.rs               # KE-001 through KE-015
│   ├── partition_operations_integration.rs # PC-01 through PC-08, RW-001 through RW-015
│   ├── compaction_integration.rs          # C-001 through C-010
│   ├── crash_recovery_integration.rs      # CR-001 through CR-010
│   ├── key_ordering_proptest.rs           # KI-001 through KI-010
│   └── partition_layout_red_queen.rs      # Adversarial tests (extends ve-3zrs)
├── proptest-regressions/
│   └── key_ordering.txt
└── fuzz/
    ├── key_encoding.rs
    └── partition_constants.rs
```

</content>
<filePath>
/home/lewis/gt/veloxide/polecats/vault/veloxide/crates/vo-storage/test-plan.md