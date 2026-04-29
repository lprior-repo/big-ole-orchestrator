# Findings: tw-o36c - vo-storage key_encoding

## Issue Summary
- **Title**: vo-storage: key_encoding must produce lexicographically sortable keys
- **Description**: Fix key encoding that uses format strings which don't sort lexicographically for numeric values (key_9 sorts after key_10)

## Research Findings

### 1. vo-storage crate does NOT exist in fury worktree
- The fury worktree (`/home/lewis/gt/veloxide/polecats/fury/veloxide/crates/`) contains:
  - vo-actor, vo-api, vo-common, vo-frontend, vo-ipc, vo-sdk, vo-types, vo-worker
  - **vo-storage is missing**

### 2. vo-storage EXISTS in other polecat worktrees
- Bandit has vo-storage at: `/home/lewis/gt/veloxide/polecats/bandit/veloxide/crates/vo-storage/`
- The key_encoding module exists at: `crates/vo-storage/src/key_encoding/mod.rs`

### 3. Bandit's key_encoding ALREADY uses correct approach
Bandit's implementation already uses big-endian binary encoding for lexicographic sorting:

```rust
// Big-endian u64 encoding
pub const fn encode_u64_be(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}

// Event key format: [instance_id_len(2)][instance_id(16)][sequence(8)] = 26 bytes
pub fn encode_event_key(instance_id: &InstanceId, sequence: SequenceNumber) -> Vec<u8>
```

This produces correct lexicographic ordering because big-endian byte order matches numeric order.

### 4. Issue description doesn't match Bandit implementation
- Issue says: "For InstanceId keys, use 20-char zero-padded numeric prefix followed by raw ID bytes"
- Bandit uses: 16-byte binary ULID (no zero-padding needed because binary sorting works)

### 5. Git history shows key_encoding was previously a file, now a directory
```
commit b011b41b: key_encoding.rs => key_encoding/mod.rs
```

## Conclusion
The issue describes a problem (format string-based key encoding) that either:
1. Was already fixed in Bandit's implementation
2. Was created for a different codebase state

Since vo-storage doesn't exist in the fury worktree, there is no key_encoding.rs to fix in this context.

## Recommendation
- If key_encoding needs to be added to fury's worktree, it should use the Bandit implementation as reference
- The Bandit implementation already satisfies the lexicographic sorting requirement via big-endian binary encoding
