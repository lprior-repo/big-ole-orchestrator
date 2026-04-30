# ADR 057: Blob Integrity Hash Algorithm Specification

## Status
Accepted

## Context
Veloxide stores canonical payload blobs with content-addressed identifiers derived from cryptographic hashes. Multiple hash algorithms are used across the codebase (SHA-256 for content addressing, BLAKE3 for merkle tree chunk hashing, CRC32 for snapshot checksums) but there is no single ADR that specifies which algorithm is the authoritative integrity algorithm for blob storage, when verification must occur, and what the failure semantics are.

The `checksum.rs` module provides a multi-algorithm `Checksum` struct combining CRC32, SHA-256, and BLAKE3. The `FsBlobStore` computes SHA-256 content addresses and verifies them on retrieve. The `FjallBlobStore` computes SHA-256 content addresses but does NOT verify them on `get()`. This inconsistency needs to be resolved by formal specification.

## Decision

### 1. Content Addressing Algorithm

The blob content address (used as the storage key and `output_hash`) is always SHA-256.

- **Algorithm**: SHA-256 (NIST FIPS 180-4)
- **Digest size**: 32 bytes (256 bits)
- **Encoding**: 64 lowercase hexadecimal characters (0-9, a-f)
- **Type**: `ContentAddress` in `blob_store/content_address.rs`

This is the sole authoritative algorithm for content addressing. No other algorithm may be used as a blob content address.

```
ContentAddress = SHA-256(blob_data) formatted as 64-char lowercase hex
```

### 2. Blob Integrity Verification on Read

When a blob is retrieved from any storage backend, its integrity MUST be verified before the data is returned to the caller.

**FsBlobStore**: Already implements this correctly in `retrieve_async()`:
1. Read blob file from disk
2. Compute SHA-256 of retrieved data
3. Compare computed hash against the content address used as the file path
4. Return `BlobStoreError::ChecksumMismatch` if they differ
5. Return data if they match

**FjallBlobStore**: MUST implement this same verification in `get()`:
1. Retrieve blob bytes from Fjall keyspace
2. Compute SHA-256 of retrieved data
3. Compare computed hash against the `ContentAddress` key
4. Return `BlobStoreError::ChecksumMismatch` if they differ
5. Return data if they match

This invariant ensures that storage-level corruption (bit rot, LSM-tree compaction errors, Fjall bugs) is always detected before corrupted data propagates to callers.

### 3. Multi-Algorithm Checksums (Checksum struct)

The `Checksum` struct in `checksum.rs` combines three algorithms for layered verification:

| Algorithm | Purpose | When Used |
|-----------|---------|-----------|
| SHA-256 | Content addressing, integrity verification | Blob storage keys, retrieve verification |
| BLAKE3 | Merkle tree chunk hashes, fast tree construction | Merkle trees for large blob chunk verification |
| CRC32 | Fast pre-check corruption detection | Snapshot headers, quick integrity screening |

The `Checksum::verify_checksum()` function checks SHA-256 and BLAKE3 (primary integrity) but NOT CRC32 (fast screening only). This ordering is intentional: CRC32 is too weak for security-critical integrity but useful for fast rejection of obviously corrupted data.

### 4. Hash Computation Functions

All hash computation for blob integrity uses a single canonical function per algorithm:

- **SHA-256**: `sha2::Sha256::digest(data)` — used by both `FsBlobStore::compute_content_address()` and `FjallBlobStore::compute_content_address()`
- **BLAKE3**: `blake3::hash(data)` — used by merkle tree chunk hashing
- **CRC32**: `crc32fast::hash(data)` — used by snapshot checksums

The `checksum.rs` `StreamingHasher` struct computes all three in a single pass for efficiency.

### 5. Write-Time Integrity

At write time, the blob data is hashed and the content address is derived. The blob is stored at a path/key determined by this address. No separate integrity file or metadata is stored — the content address itself IS the integrity hash.

For `FsBlobStore`: The blob file is named `<sha256_hex>` and the meta file is `<sha256_hex>.json`. The file path is the integrity guarantee.

For `FjallBlobStore`: The key is the SHA-256 hex encoding. The key itself is the integrity guarantee.

### 6. Non-Exhaustive Content Address Validation

The `ContentAddress::new()` constructor validates that a string is exactly 64 lowercase hex characters. This is a format validation, NOT a hash computation. It ensures that:

- No uppercase hex is accepted (canonical form)
- No truncated hashes are accepted (must be full 256-bit)
- No non-hex characters are accepted

## Consequences

- **Positive**: Content address is both the identifier AND the integrity check — no separate checksum storage needed.
- **Positive**: Multi-algorithm checksums provide defense-in-depth: CRC32 for fast screening, SHA-256 for security, BLAKE3 for merkle tree performance.
- **Positive**: FsBlobStore already verifies on read — this ADR makes FjallBlobStore consistent.
- **Negative**: FjallBlobStore `get()` must be updated to add SHA-256 verification, adding one hash computation per read.
- **Negative**: Any existing data written with a different hash algorithm is incompatible — content addressing is non-negotiable.
- **Positive**: The `Checksum` struct's triple-algorithm approach covers all use cases without needing per-context algorithm selection.

## Non-Decision

- SHA-256 is not being replaced by BLAKE3 for content addressing despite BLAKE3's performance advantages. SHA-256 is the standard, has wider tooling support, and the performance difference is negligible for blob storage where I/O dominates.
- Additional algorithms are not being added to `Checksum` without explicit ADR approval.

## References

- ADR-040: Canonical Blob Durability and Publication
- ADR-043: Exact-Once Verification Strategy
- `crates/vo-storage/src/blob_store/content_address.rs`: ContentAddress type
- `crates/vo-storage/src/blob_store/fjall_store.rs`: FjallBlobStore (needs update)
- `crates/vo-storage/src/fs_store/operations.rs`: FsBlobStore retrieve (already correct)
- `crates/vo-storage/src/checksum.rs`: Multi-algorithm Checksum struct
- `crates/vo-storage/src/merkle_tree.rs`: BLAKE3-based merkle tree
