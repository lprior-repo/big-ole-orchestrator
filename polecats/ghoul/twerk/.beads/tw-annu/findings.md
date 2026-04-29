# Findings: tw-annu - vo-core: Replace SystemTime::now().unwrap() with fallible UUID gen

## Summary

**DISCREPANCY FOUND**: The issue described in this bead (line 187 of `crates/core/src/events.rs`) does NOT exist in the veloxide `vo-core` crate. The file `crates/vo-core/src/events.rs` does not exist in the veloxide project.

The actual issue exists in the **hardline** project at `/home/lewis/src/hardline/crates/core/src/events.rs:187`.

## Actual Issue Location

**File**: `/home/lewis/src/hardline/crates/core/src/events.rs`
**Lines**: 183-190
**Function**: `uuid_simple()`

```rust
#[allow(clippy::unwrap_used)]
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()  // <-- LINE 187: PANIC ON CLOCK SKEW
        .as_nanos();
    format!("{:x}", now)
}
```

## Issue Details

### Problem
- `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` can **panic** if the system clock goes backwards (e.g., NTP adjustments, VM time sync, suspend/resume)
- The function is marked with `#[allow(clippy::unwrap_used)]` to suppress the lint
- This violates the file's own documentation header: "Zero panic, zero unwrap - all operations return Result"

### Production Usage
The `uuid_simple()` function is called from `MemEventEmitter::emit()` at line 143:
```rust
fn emit(&self, event: Event) -> Result<()> {
    let emitted = EmittedEvent {
        id: uuid_simple(),  // <-- Called in production code path
        event,
        timestamp: Utc::now(),
        source: "scp".to_string(),
    };
    ...
}
```

### Severity
- **P0**: Production panic vector
- Clock skew is rare but can occur in containers, VMs, and with NTP
- No way for caller to handle this error - it crashes the process

## Recommended Fix

Replace `uuid_simple()` with `Uuid::now_v7()` from the `uuid` crate (already a dependency).

**Requires**:
1. Update `uuid` dependency from `version = "1"` to `version = "1.11"` in `/home/lewis/src/hardline/Cargo.toml`
2. Add `v7` feature: `uuid = { version = "1.11", features = ["v4", "v7", "serde"] }`
3. Replace `uuid_simple()` implementation with `Uuid::now_v7().to_string()`

**Alternatively** (if UUID v7 is not available), make `uuid_simple()` fallible:
```rust
fn uuid_simple() -> Result<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::internal(e.to_string()))?
        .as_nanos();
    Ok(format!("{:x}", now))
}
```

## Bead Routing Issue

This bead (`tw-annu`) was filed as "vo-core" issue but actually belongs to the **hardline** project. The correct parent bead would be `tw-eqgt` which correctly identifies the issue as "hardline: Replace assert!/expect! with Result in production panic paths".

## Files Examined
- `/home/lewis/src/veloxide/crates/vo-core/src/` - No `events.rs` found
- `/home/lewis/src/hardline/crates/core/src/events.rs` - Found issue at line 187
