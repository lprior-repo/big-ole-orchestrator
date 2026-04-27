# Findings: tw-mk3k - Make ConfigManager::new() and global_config() return Result

## Issue
Bead tw-mk3k describes: "config_core.rs:554,561 use expect() on IO operations. Make Default impl and global_config() fallible."

## Analysis

### File Location
The bead description references `crates/core/src/config/config_core.rs` but this file does NOT exist in the twerk worktree at `/home/lewis/gt/polecats/fury/twerk/`. The file exists at:

`/home/lewis/gt/hardline/polecats/fury/hardline/crates/core/src/config/config_core.rs`

This appears to be a cross-rig issue: a twerk bead (tw-mk3k) points to hardline code.

### Code Analysis

In `config_core.rs`:

1. **Default impl (lines 551-556)**:
   ```rust
   #[allow(clippy::expect_used)]
   impl Default for ConfigManager {
       fn default() -> Self {
           Self::new().expect("Failed to create config manager")
       }
   }
   ```
   - Uses `expect()` on `ConfigManager::new()` which returns `Result`
   - Problem: `Default::default()` cannot return `Result`, it must return `Self`

2. **global_config() function (lines 558-562)**:
   ```rust
   #[allow(clippy::expect_used)]
   pub fn global_config() -> ConfigManager {
       ConfigManager::new().expect("Failed to create config manager")
   }
   ```
   - Uses `expect()` on `ConfigManager::new()` which already returns `Result<Self>`
   - Problem: Should propagate error instead of panicking

### Proposed Fix

1. **Remove Default impl**: Since `Default::default()` cannot return `Result`, the only safe option is to remove this impl entirely. It was using IO that could fail.

2. **Change global_config() return type**:
   ```rust
   pub fn global_config() -> Result<ConfigManager> {
       ConfigManager::new()
   }
   ```
   This propagates the error instead of panicking.

### Callers Impact

Callers of `global_config()` like `session.rs:84`:
```rust
let config = scp_core::config::global_config().load()?;
```
Would need to change to:
```rust
let config = scp_core::config::global_config()?.load()?;
```

### Additional Finding

There is also a copy of `config_core.rs` in:
`/home/lewis/gt/hardline/polecats/fury/hardline/deacon/dogs/alpha/hardline/crates/core/src/config/config_core.rs`

This deacon copy also has the same `expect()` issue at lines 541-545 and would need the same fix.

## Cross-Worktree Issue

The bead is filed in twerk rig (tw-mk3k) but the referenced code is in hardline. The twerk worktree at `/home/lewis/gt/polecats/fury/twerk/` does not contain the actual source code - only `.beads/` and `.runtime/` directories.

## Status

Code changes were attempted but there is a fundamental mismatch between:
- The bead's rig (twerk) where this polecat (fury) is assigned
- The actual location of the code being reviewed (hardline)

This may indicate a bead routing issue or an intentional cross-rig audit task.