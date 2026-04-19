## Contract: Segment Tree

### 1. Purpose

Defines the contract for a segment tree data structure in veloxide. This contract establishes types, invariants, and error taxonomy for efficient range queries and range updates on arrays. The segment tree enables O(log n) range aggregation (sum, min, max) and point/range updates, supporting time-series monitoring of event counts, resource usage metrics, and workflow state statistics.

### 2. Source ADRs

- `docs/adr/v2/ADR-002-v2-fjall-storage.md` (key encoding and tree structures)
- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (batch operations)

### 3. Segment Tree Types

#### 3.1 SegmentTreeConfig

Configuration for constructing a segment tree.

```rust
struct SegmentTreeConfig {
    /// Initial capacity (number of leaves).
    len: usize,
}
```

**Constraints:**
- `len >= 1`

#### 3.2 SegmentTree<T>

A segment tree parameterized by value type `T` and merge operation.

```rust
pub struct SegmentTree<T> {
    tree: Vec<T>,
    len: usize,
    merge: fn(&T, &T) -> T,
    identity: T,
}
```

Uses a flat array representation (1-indexed internally):
- Root at index 1
- Left child of `i` at `2*i`
- Right child of `i` at `2*i + 1`
- Leaves at indices `[n, 2n)`

#### 3.3 LazySegmentTree<T, U>

A segment tree with lazy propagation for range updates.

```rust
pub struct LazySegmentTree<T, U> {
    tree: Vec<T>,
    lazy: Vec<Option<U>>,
    len: usize,
    merge: fn(&T, &T) -> T,
    identity: T,
    apply: fn(&T, &U, usize) -> T,
    compose: fn(&U, &U) -> U,
}
```

Where:
- `merge`: combines two segment values
- `apply`: applies a lazy update `U` to a segment value `T` with given segment length
- `compose`: composes two lazy updates (newer into older)

### 4. Core Operations

#### 4.1 SegmentTree Operations

```rust
impl<T: Clone> SegmentTree<T> {
    /// Build a segment tree from a slice with the given merge function and identity.
    fn from_slice(data: &[T], merge: fn(&T, &T) -> T, identity: T) -> Self

    /// Query the aggregate value over range [left, right).
    fn query(&self, left: usize, right: usize) -> T

    /// Update the value at a single position.
    fn update(&mut self, index: usize, value: T)

    /// Get the value at a single position.
    fn get(&self, index: usize) -> T

    /// Number of elements in the underlying array.
    fn len(&self) -> usize
}
```

#### 4.2 LazySegmentTree Operations

```rust
impl<T: Clone, U: Clone> LazySegmentTree<T, U> {
    /// Build from a slice with merge, identity, apply, and compose functions.
    fn from_slice(
        data: &[T],
        merge: fn(&T, &T) -> T,
        identity: T,
        apply: fn(&T, &U, usize) -> T,
        compose: fn(&U, &U) -> U,
    ) -> Self

    /// Query the aggregate value over range [left, right).
    fn query(&self, left: usize, right: usize) -> T

    /// Update a single position.
    fn update_point(&mut self, index: usize, value: T)

    /// Apply an update to range [left, right).
    fn update_range(&mut self, left: usize, right: usize, update: U)

    /// Number of elements.
    fn len(&self) -> usize
}
```

### 5. Invariants (INV-*)

- **INV-ST001**: `query(0, len)` returns the aggregate over the entire array
- **INV-ST002**: `query(i, i+1)` returns the value at position `i`
- **INV-ST003**: After `update(i, v)`, `get(i)` returns `v`
- **INV-ST004**: Range query `[l, r)` uses half-open interval: includes `l`, excludes `r`
- **INV-ST005**: Panics on out-of-bounds indices (`l >= len` or `r > len` or `l > r`)
- **INV-ST006**: Merge operation is associative: `merge(a, merge(b, c)) == merge(merge(a, b), c)`
- **INV-ST007**: Identity is the neutral element: `merge(x, identity) == x == merge(identity, x)`

### 6. Error Taxonomy

Panics on invalid inputs:
- Out-of-bounds index
- Empty range (l > r)
- Empty input data

### 7. Test Coverage Requirements

| Test ID | Description |
|---------|-------------|
| ST-01 | Build from slice and query full range |
| ST-02 | Point update changes query result |
| ST-03 | Range query returns correct partial sum |
| ST-04 | Out-of-bounds panics |
| ST-05 | Single element tree |
| ST-06 | Identity property |
| ST-07 | Lazy range update correctness |
| ST-08 | Overlapping lazy updates compose correctly |
| ST-09 | Point update on lazy tree |
| ST-10 | Multiple range updates then query |

### 8. Constraints

- **No Async**: All operations are synchronous.
- **No Persistence**: In-memory only.
- **Generic**: Works with any `T: Clone` and any binary merge operation.
- **Panics on Invalid Input**: No `Result` type — invalid indices panic (programmer error).

### 9. Acceptance Criteria

- [ ] `SegmentTree<T>` builds from slice with merge and identity
- [ ] `query` returns correct aggregate for any valid range
- [ ] `update` correctly modifies single elements
- [ ] `LazySegmentTree<T, U>` supports range updates with lazy propagation
- [ ] All invariants hold
- [ ] Tests cover all ST-* test cases

### 10. Relevant Files

- `crates/vo-core/src/segment_tree.rs` (primary implementation)
