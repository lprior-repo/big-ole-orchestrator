## Contract: Checksum Verification Pipeline

### 1. Purpose

Defines the contract for a multi-algorithm checksum verification pipeline in the veloxide storage system. This contract establishes types, invariants, and error taxonomy for streaming, parallel, and incremental checksum computation supporting CRC32, SHA-256, and BLAKE3 algorithms.

### 2. Source ADRs

- `docs/adr/v2/ADR-016-v2-atomic-storage-snapshots.md` (storage snapshots)
- `docs/adr/v2/ADR-002-v2-fjall-storage.md` (Fjall storage engine)

### 3. Algorithm Types

#### 3.1 ChecksumAlgorithm

Supported checksum algorithms.

```
enum ChecksumAlgorithm {
  Crc32,   // CRC32 fast checksum
  Sha256,  // SHA-256 cryptographic hash
  Blake3,  // BLAKE3 cryptographic hash
}
```

#### 3.2 Checksum

Multi-algorithm checksum result containing all three algorithm outputs.

```
struct Checksum {
  crc32: u32,
  sha256: [u8; 32],
  blake3: [u8; 32],
}
```

### 4. Hasher Types

#### 4.1 StreamingHasher

Incremental hasher for streaming data processing. Updates multiple hash algorithms simultaneously.

```
struct StreamingHasher {
  crc32: crc32fast::Hasher,
  sha256: sha2::Sha256,
  blake3: blake3::Hasher,
}
```

**Operations:**
- `new()` → StreamingHasher
- `update(&mut self, data: &[u8])` — update all hashers
- `finalize(self) → Checksum` — produce final checksum

#### 4.2 ChunkedHasher

Streaming hasher that splits data into chunks with individual checksums.

```
struct ChunkedHasher {
  chunk_size: u64,
  current_offset: u64,
  current_hasher: StreamingHasher,
  chunks: Vec<ChunkInfo>,
}
```

#### 4.3 ChunkInfo

Metadata and checksum for a single chunk.

```
struct ChunkInfo {
  offset: u64,      // Byte offset in original data
  size: u64,       // Chunk size in bytes
  checksum: Checksum,
}
```

### 5. Verification Types

#### 5.1 VerificationResult

Outcome of checksum verification.

```
enum VerificationResult {
  Valid,                    // Checksum matches
  Invalid(ChecksumError),   // Checksum mismatch
}
```

### 6. Invariants (INV-*)

- **INV-001**: `StreamingHasher::finalize` produces identical `Checksum` regardless of how `update` calls are distributed across chunks
- **INV-002**: `compute_checksum(data)` is equivalent to `StreamingHasher::new().update(data).finalize()`
- **INV-003**: `verify_checksum(data, expected)` returns `Ok(())` if and only if `compute_checksum(data) == expected`
- **INV-004**: `ChunkedHasher` produces chunks where `chunk.offset` is monotonically increasing
- **INV-005**: The sum of all `ChunkInfo.size` values equals the total bytes processed
- **INV-006**: `ChunkedHasher::finalize` always produces at least one chunk if any data was processed
- **INV-007**: An empty data slice (`&[]`) passed to `compute_checksum` produces a valid `Checksum` with all fields set to algorithm-specific empty hash values
- **INV-008**: `Checksum` is `PartialEq` and `Eq` — two checksums are equal iff all three algorithm outputs match
- **INV-009**: `Checksum` serializes and deserializes identically via serde (JSON, binary)
- **INV-010**: `ChecksumError::Mismatch` always reports the first failing algorithm (SHA256, then BLAKE3, then CRC32 order)

### 7. Error Taxonomy

```rust
enum ChecksumError {
  Mismatch {
    algorithm: ChecksumAlgorithm,
    expected: String,
    actual: String,
  },
  Io(String),
}
```

#### 7.1 Error Categories

| Category | Description |
|----------|-------------|
| `Mismatch` | Computed checksum does not match expected value |
| `Io` | I/O error during checksum computation (e.g., read failure) |

#### 7.2 Error Semantics

- **Mismatch**: The data has been corrupted or the expected checksum is incorrect. The `algorithm` field identifies which checksum failed first.
- **Io**: An I/O operation failed during streaming checksum computation. The wrapped message describes the I/O error.

#### 7.3 Display Format

```
checksum mismatch for sha256: expected abc123..., got def456...
checksum I/O error: failed to read from buffer
```

### 8. Processing Model

#### 8.1 Single-Pass Computation

```
compute_checksum(data: &[u8]) → Checksum
  1. Create StreamingHasher
  2. Call update(data)
  3. Call finalize() → Checksum
```

#### 8.2 Streaming Computation

```
StreamingHasher
  1. new() → empty hasher
  2. update(chunk) → accumulate (repeat as needed)
  3. finalize() → Checksum
```

#### 8.3 Chunked Computation

```
ChunkedHasher
  1. new(chunk_size) → empty chunked hasher
  2. update(data) → split into chunks, hash each (repeat as needed)
  3. finalize() → Vec<ChunkInfo>
```

#### 8.4 Verification

```
verify_checksum(data: &[u8], expected: &Checksum) → Result<(), ChecksumError>
  1. Compute compute_checksum(data)
  2. Compare computed.sha256 to expected.sha256
  3. If mismatch → return Mismatch error
  4. Compare computed.blake3 to expected.blake3
  5. If mismatch → return Mismatch error
  6. Compare computed.crc32 to expected.crc32
  7. If mismatch → return Mismatch error
  8. Return Ok(())
```

### 9. Constraints

- **Data Layer Only**: The checksum module provides pure types and computation with no I/O dependencies
- **No Dynamic Dispatch**: All hashers use static dispatch for performance
- **Thread Safe**: `Checksum` and `ChecksumAlgorithm` are `Send + Sync`
- **Deterministic**: Same input always produces identical output across runs
- **Incremental**: Hashers support incremental updates without reprocessing previous data
- **Parallel-Friendly**: Multiple `StreamingHasher` instances can operate independently

### 10. Relevant Files

- `crates/vo-storage/src/checksum.rs` (implementation)
- `crates/vo-storage/src/snapshots.rs` (checksum usage in snapshots)
- `crates/vo-storage/src/codec.rs` (checksum integration in codec)

### 11. Acceptance Criteria

- Checksum types compile with all three algorithm fields
- StreamingHasher supports incremental updates with consistent results
- ChunkedHasher correctly splits data and produces per-chunk checksums
- verify_checksum returns Ok only when all three algorithms match
- ChecksumError::Mismatch reports the failing algorithm and hex-encoded values
- All invariants (INV-001 through INV-010) are formally stated
- Error taxonomy is exhaustive for all checksum error scenarios
- The contract is self-contained and references only existing implementation
