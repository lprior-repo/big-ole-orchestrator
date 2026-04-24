# Findings: tw-ygp1 vo-storage: Persist lease entry after successful acquire

## Issue Description
Call insert_lease() after successful acquire to ensure the lease is actually stored.
Currently acquire returns LeaseRecord but never writes it.

## File Analyzed
`crates/vo-storage/src/lease_partition/fjall_lease_store.rs` lines 171-181

## Analysis Result: NO ACTION NEEDED - Issue Already Fixed

The code at lines 171-181 in `FjallLeaseStore::acquire()`:

```rust
let entry = LeaseEntry::new(
    instance_id.to_string(),
    step_id.to_string(),
    fence_token,
    now_ms.saturating_add(ttl_ms),
)?;

self.insert_lease(&entry)?;  // <-- This IS called

entry.to_lease_record()
```

The `insert_lease(&entry)` IS correctly called after creating the `LeaseEntry` and BEFORE
converting to `LeaseRecord` via `to_lease_record()`.

## Verification
- Build passes: `cargo build -p vo-storage` succeeds
- Code inspection confirms `insert_lease` is called
- No duplicate/missing persist call detected

## Conclusion
The bug described in the issue does not exist in the current code.
The lease IS being persisted after successful acquire. This appears to be a stale
bead where the issue was already resolved prior to this review.
