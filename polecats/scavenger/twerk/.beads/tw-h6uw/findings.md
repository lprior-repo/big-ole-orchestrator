# tw-h6uw Findings: ExecProbe Command Path Validation

## Status: NO CODE CHANGES (audit only — misfiled bead)

## Summary

Bead `tw-h6uw` is filed in the **twerk** rig but references code in the **veloxide** repo (`crates/vo-actor/src/probe.rs`). The twerk worktree (`polecats/scavenger/twerk/`) contains no source code and no git repo. This bead should have been filed in the **veloxide** rig with prefix `ve-`.

## Vulnerability Analysis

**File**: `crates/vo-actor/src/probe.rs:469-512`

**Severity**: HIGH (ADR-012)

**Issue**: `ExecProbe::new(command, args)` accepts any arbitrary string as the command path. The `check()` method (line 501-512) passes it directly to `tokio::process::Command::new(&self.command)` with zero validation.

**Attack Vector**: Any caller or deserialized config can specify:
- Absolute paths to sensitive binaries (`/bin/rm`, `/bin/sh`)
- Relative paths that resolve to attacker-controlled executables
- Paths with shell metacharacters (though `Command::new` doesn't shell-expand)

**Current Code** (lines 478-485):
```rust
pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
    Self {
        id: ProbeId::new(),
        command: command.into(),
        args,
        expected_exit_code: Some(0),
        timeout: Duration::from_secs(30),
    }
}
```

No allowlist. No path canonicalization. No existence check. No directory restriction.

**Additional concerns**:
1. `unwrap()` on line 457 (`SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`) — panic on clock skew (unlikely but violates zero-panic)
2. Command output is captured but never returned to callers — potential information leak if stdout/stderr contain sensitive data in future changes

## Recommended Fix (for veloxide rig)

1. Add a `command_allowlist` field to `ExecProbe` — a list of allowed binary directories (e.g., `/usr/bin`, `/usr/local/bin`, `/opt/veloxide/bin`)
2. In `new()` or a dedicated `validate()` method:
   - Parse the command path, resolve symlinks via `std::fs::canonicalize`
   - Verify the resolved absolute path starts with one of the allowed directories
   - Return `ProbeError::InvalidCommand` if validation fails
3. Add a `CommandAllowlist` type to `vo-actor` constants or config
4. Unit tests: verify rejected paths, accepted paths, symlink traversal attempts

## Corrective Action Needed

This bead should be refiled in the **veloxide** rig:
```bash
bd create --rig veloxide --title "vo-actor: Validate ExecProbe command path against allowlist" --description="HIGH ADR-012: ExecProbe accepts arbitrary command without path validation..." --type=bug --priority=1
```
