## Contract: Memory-Mapped File Cache

### 1. Purpose

Defines the contract for a memory-mapped file cache that uses mmap for efficient data access with LRU eviction. This contract establishes types, invariants, and error taxonomy for a cache that stores binary data on disk using memory-mapped files with bounded memory usage.

### 2. Source ADRs

- `docs/adr/v2/ADR-012-v2-execution-boundary-hardening.md` (execution boundary)
- `docs/adr/v2/ADR-015-v2-actor-invariants-backpressure.md` (actor health semantics)

### 3. Types

#### 3.1 MmapCacheError

```
enum MmapCacheError {
  IoError(std::io::Error),        // File system error
  MmapError(std::io::Error),       // Memory mapping error
  RegionNotFound(String),          // Key not present in cache
  InvalidRegion,                   // Region data corrupted or invalid
  CacheFull,                       // Cannot evict enough to make space
  SerializationError,              // Data serialization/deserialization failed
}
```

#### 3.2 MmapCache

```
struct MmapCache {
  base_path: PathBuf,              // Directory for cache region files
  max_memory_bytes: usize,         // Maximum memory threshold
  current_memory_bytes: usize,     // Current memory usage
  access_counter: u64,             // Monotonic access counter for LRU
  lru_queue: VecDeque<String>,     // LRU ordering queue
  entries: HashMap<String, LruEntry>, // Key -> entry mapping
}
```

#### 3.3 MmapCacheBuilder

```
struct MmapCacheBuilder {
  base_path: PathBuf,
  max_memory_bytes: usize,         // Default: 100 MiB
}

impl MmapCacheBuilder {
  fn new(base_path: PathBuf) -> Self
  fn max_memory_bytes(mut self, bytes: usize) -> Self
  fn build(self) -> Result<MmapCache, MmapCacheError>
}
```

#### 3.4 Internal Types

```
struct CacheRegion {
  _offset: u64,                    // File offset (unused, always 0)
  size: u64,                       // Region size in bytes
  file_path: PathBuf,              // Path to region file
}

struct LruEntry {
  _key: String,                    // Cache key
  region: CacheRegion,             // Region metadata
  _last_access: u64,               // Access counter value
}
```

### 4. Invariants (INV-*)

- **INV-001**: `MmapCache::new` creates the `base_path` directory if it does not exist
- **INV-002**: `max_memory_bytes > 0`; zero limit returns error during insert
- **INV-003**: `current_memory_bytes <= max_memory_bytes` at all times
- **INV-004**: `lru_queue.len() == entries.len()`; queue and map stay synchronized
- **INV-005**: The LRU queue is ordered by ascending `last_access` time; front is least recently used
- **INV-006**: `insert` atomically checks capacity, evicts if needed, then writes; no partial states on error
- **INV-007**: `evict_until_space_available` removes entries until `current_memory_bytes + needed <= max_memory_bytes` or queue is empty
- **INV-008**: `get` opens the region file and memory-maps it; returns data as `Vec<u8>`
- **INV-009**: `remove` removes from both `entries` and `lru_queue`, and deletes the region file
- **INV-010**: `clear` removes all entries, deletes all region files, resets memory tracking
- **INV-011**: On `Drop`, `MmapCache` calls `clear`, ensuring all temp files are removed
- **INV-012**: Region file names are sanitized: `/`, `\`, `:` replaced with `_`
- **INV-013**: `prefetch` loads the mmap without returning data; validates region exists
- **INV-014**: `read_ahead` calls `prefetch` for each key in order; continues on individual errors
- **INV-015**: `contains_key` does not update LRU ordering (read-only check)
- **INV-016**: `get` does not update LRU ordering (read-only access)

### 5. Error Taxonomy

```rust
enum MmapCacheError {
  IoError(std::io::Error),         // File system operation failed
  MmapError(std::io::Error),       // mmap system call failed
  RegionNotFound(String),          // Key not found in cache
  InvalidRegion,                   // Region file corrupted or malformed
  CacheFull,                       // Cannot allocate; eviction failed
  SerializationError,              // Data encoding/decoding error
}
```

#### 5.1 Error Categories

| Variant | Category | Description |
|---------|----------|-------------|
| `IoError` | System | OS-level I/O failure (disk full, permissions, etc.) |
| `MmapError` | System | Memory mapping failure |
| `RegionNotFound` | Input | Key does not exist in cache |
| `InvalidRegion` | Data | Region file exists but data is invalid/corrupted |
| `CacheFull` | Capacity | Cannot make space via eviction |
| `SerializationError` | Data | Data encoding/decoding error |

#### 5.2 Error Transitions

| Operation | Ok | IoError | MmapError | RegionNotFound | InvalidRegion | CacheFull | SerializationError |
|-----------|----|---------|-----------|----------------|---------------|-----------|---------------------|
| `new` | new cache | create dir fail | - | - | - | - | - |
| `insert` | stored | write/flush fail | - | - | - | evict fails | - |
| `get` | data | open fail | mmap fail | key missing | - | - | - |
| `remove` | removed | delete fail | - | key missing | - | - | - |
| `prefetch` | done | open fail | mmap fail | key missing | - | - | - |
| `clear` | cleared | some deletes fail | - | - | - | - | - |

### 6. MmapCache Protocol

1. **Create**: Validate `base_path` accessibility, initialize empty cache
2. **Insert**: Check capacity, evict LRU entries if needed, allocate region file, write data, update tracking
3. **Get**: Lookup entry, open and mmap region file, read data, return as `Vec<u8>`
4. **Remove**: Lookup entry, delete region file, remove from tracking structures
5. **Prefetch**: Validate entry exists, open and mmap region file, drop mmap immediately
6. **Clear**: Iterate all entries, delete all region files, reset tracking
7. **Drop**: Call clear, ensure cleanup

### 7. Constraints

- Cache is NOT `Sync` because internal state (LRU counter) requires synchronized access
- Cache IS `Send` to support actor ownership transfer
- Memory-mapped files are OS-managed; no manual memory management needed
- Region files are ephemeral; designed to be temporary/cache only
- Eviction is synchronous; insert blocks until space is available
- `MmapCache` is lock-based (`parking_lot::Mutex`); operations hold lock briefly
- Maximum key length is filesystem-dependent; safe keys are alphanumeric with `_`, `-`
- The cache does not support concurrent access from multiple processes

### 8. Relevant Files

- `crates/vo-storage/src/mmap_cache.rs` (implementation)
- `crates/vo-storage/src/lib.rs` (module exports)

### 9. Acceptance Criteria

- [ ] `MmapCacheError` enum is exhaustive and covers all failure modes
- [ ] All invariants (INV-001 through INV-016) are formally stated
- [ ] Error transitions table documents all operation/error combinations
- [ ] LRU eviction correctly removes least-recently-used entries when cache is full
- [ ] `Drop` implementation guarantees cleanup of all region files
- [ ] Memory-mapped file access is read-only on get; prefetch validates without retaining
- [ ] Builder pattern provides sensible defaults (100 MiB default limit)
- [ ] Contract supports both unit testing and formal verification
