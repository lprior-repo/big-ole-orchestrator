## Black Hat Review: vo-storage

### PHASE 1: Contract & Bead Parity

**Status: PASS with MINOR CONCERNS**

Reviewed: lib.rs, blob_store.rs, append.rs, dedupe_partition/mod.rs, instance_index/mod.rs, key_partition/mod.rs

**Contract Parity:**
- Data/Calc/Actions layering observed in blob_store.rs - CORRECT
- ContentAddress validates SHA-256 format at construction - CORRECT
- Error taxonomy is comprehensive with is_transient() / is_fatal() methods - GOOD
- ContentAddress::new() returns Result<Self, BlobStoreError> for parse-not-validate - CORRECT

**Concerns:**
- lib.rs:76-83 append_event is a stub that returns Ok(()) for any input - VIOLATES parse-dont-validate
- blob_store.rs:64 uses #[expect(clippy::unsafe_derive_deserialize)] - UNSAFE_EXPECT

---

### PHASE 2: Farley Engineering Rigor

**Status: FAIL**

**Function Length Violations:**
1. BudgetQueues::try_enqueue (append.rs:638-729) - 91 lines - EXCEEDS 25 line limit
2. BudgetQueues::dequeue (append.rs:734-777) - 43 lines - EXCEEDS 25 line limit
3. decode_dedupe_entry (dedupe_partition/mod.rs:270-317) - 47 lines - EXCEEDS 25 line limit

**Separation of Concerns:**
- BudgetQueues mixes budget tracking (pure calc) with queue operations (impure I/O) - VIOLATION
- CommitLatencyTracker uses Mutex internally but exposes stateful API - MIXED

---

### PHASE 3: NASA-Level Functional Rust (The Big 6)

**Status: MIXED**

**PASS:**
- ContentAddress is a newtype wrapper around String with validation in constructor - CORRECT
- BlobStoreError is non_exhaustive with detailed variants - CORRECT
- DedupeEntry uses parse-not-validate pattern - CORRECT
- State transitions are explicit via can_transition_to method - CORRECT

**Issues:**
- ProjectionWrite (append.rs:875) uses raw String for projection_id - NEWTYPE VIOLATION
- BlobWrite (append.rs:902) uses raw String for blob_id - NEWTYPE VIOLATION
- hex_nibble (blob_store.rs:438) returns 0 for invalid input - SILENT FAILURE

**The Big 6 Checklist:**
1. Immutability: ContentAddress is immutable newtype - PASS
2. Purity: CommitLatencyTracker has side effects (metrics emission) - FAIL
3. Error types: BlobStoreError, DedupeStoreError, DekStoreError all non_exhaustive - PASS
4. No unwrap: hex_nibble returns 0 for invalid input - FAIL
5. Type-state: BlobStatus transitions are checked, but state machine is implicit - PARTIAL
6. Newtypes: String primitives used in ProjectionWrite, BlobWrite - FAIL

---

### PHASE 4: Ruthless Simplicity & DDD (Scott Wlaschin)

**Status: FAIL**

**Panic Vector:**
- blob_store.rs: Multiple #[expect(clippy::unwrap_used)] around mutex locks
- append.rs: Multiple #[expect(clippy::unwrap_used)] around mutex locks
- Total suppressions: 15+ unwrap_used expects indicate mutex poisoning as error handling

**CUPID Violations:**
- BudgetQueues has multiple responsibilities - NOT COMPOSABLE
- BackpressureSignal uses Mutex for last_event - IMPURE

---

### PHASE 5: Bitter Truth (Velocity & Legibility)

**Status: PASS**

**What Passes:**
- blob_store.rs documentation is excellent - GOOD
- Error messages are descriptive and actionable
- Module organization follows Data/Calc/Actions consistently
- No TODO-driven development observed

**Concerns:**
- lib.rs:39 has blanket #[allow(unsafe_code)] - lazy
- #[cfg_attr(not(test), deny(clippy::unwrap_used))] contradicted by heavy expect usage

---

### CRITICAL FINDINGS

1. BudgetQueues::try_enqueue is 91 lines - MUST refactor into smaller functions
2. BudgetQueues::dequeue is 43 lines - SHOULD refactor
3. decode_dedupe_entry is 47 lines - borderline
4. ProjectionWrite/BlobWrite use raw String instead of newtypes - DEK boundary violation
5. hex_nibble returns 0 silently on invalid input - dangerous
6. 15+ unwrap_used expects indicate mutex poisoning design smell

---

### VERDICT

**REJECT** - The code demonstrates solid foundational architecture but violates Farley constraints on function length and uses raw primitives where newtypes should be used.

Required actions before approval:
1. Refactor BudgetQueues::try_enqueue into <25 line functions
2. Add newtype wrappers for projection_id and blob_id
3. Fix hex_nibble to handle invalid input explicitly
4. Reduce unwrap_used expect count by addressing mutex poisoning design
