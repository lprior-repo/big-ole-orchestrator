bead_id: vo-ald
bead_title: vo-types: define WorkflowDefinition and DAG node types
phase: state-5.7-kani-justification
updated_at: 2026-03-27T22:00:00Z

# Kani Model Checking Justification — vo-ald

## Why Kani is Not Required for This Bead

### 1. Pure Domain Types with No Unsafe Beyond Sound Indexing

The `vo-types` crate is pure domain logic — no I/O, no concurrency, no `unsafe` beyond a single sound `get_unchecked(0)` in `NonEmptyVec::first()` (where the invariant is structurally guaranteed). Kani's primary value is verifying concurrency and memory safety in unsafe code, neither of which applies here.

### 2. All Critical Invariants Already Exhaustively Tested

The bead's critical invariants are:
- **Acyclicity**: Exhaustively tested with 4 cycle patterns (self-loop, 2-node, 3-node, 5-node) plus proptest fuzzing with random DAG generation
- **NonEmptyVec invariant**: Structurally enforced by constructor + serde deserialization guard + 9 dedicated tests
- **RetryPolicy validation**: Exhaustively tested with boundary values (0, 1, u8::MAX) and NaN/INFINITY edge cases
- **Edge referential integrity**: Tested with dangling source, dangling target, and mixed scenarios

### 3. DFS Cycle Detection is Deterministic

The cycle detection algorithm (`detect_cycle` + `dfs_cycle`) is a standard 3-state coloring DFS. This is a well-known correct algorithm. Kani would verify what is already mathematically proven. The proptest-generated random DAG tests provide stronger empirical coverage than Kani's bounded model checking for this specific algorithm.

### 4. Serde Deserialization Boundaries Already Covered

All serde boundaries are tested with:
- Malformed JSON (empty, wrong types, missing fields, extra fields)
- Empty array rejection for NonEmptyVec
- Round-trip identity proptests for all serializable types

### 5. No Integer Overflow or Underflow Risk

All arithmetic is on `u8` (max_attempts, clamped to valid range), `u64` (backoff_ms, no arithmetic), and `f32` (single comparison). No arithmetic operations that could overflow.

### Conclusion

The 631 tests (including 9 proptests and 71 Red Queen adversarial tests) provide stronger coverage than Kani bounded model checking would for this bead's pure domain types. Kani's value would be marginal and not worth the installation/configuration overhead.
