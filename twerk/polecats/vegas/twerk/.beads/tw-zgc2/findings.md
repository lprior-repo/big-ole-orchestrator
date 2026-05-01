# Findings: tw-zgc2 - twerk-core ASL evaluator test

## Task

Test ASL evaluator handles nested conditional expressions in `crates/twerk-core/src/eval/mod.rs`

## Investigation

1. **Bead description** claims `crates/twerk-core/src/eval/mod.rs` exists with `eval()` function for ASL expressions
2. **Reality**: The `twerk-core` crate does not exist in this repository (veloxide)

### veloxide repo (this worktree)
- Crates: `vo-actor`, `vo-storage`, `vo-sdk-macros`, `vo-types`, `vo-ipc`, `vo-api`, etc.
- No `twerk-core` crate found
- No ASL evaluator or `eval()` function for conditional expressions

### twerk repo
- Crates: `twerk-app`, `twerk-cli`, `twerk-common`, `twerk-core`, `twerk-infrastructure`, etc. (from tw-mf8i findings)
- However, `twerk-core` in twerk repo also does not contain `eval/mod.rs` path

## Conclusion

**Cannot implement**: The `crates/twerk-core/src/eval/mod.rs` file referenced in the bead does not exist. The ASL evaluator for conditional expressions is not implemented in either the veloxide or twerk repositories.

This is consistent with other beads (e.g., tw-mf8i) that reference twerk crates that don't exist or don't have the expected structure.

## Recommendation

Close as `no-changes: twerk-core crate does not exist`. If this ASL evaluator feature is needed, a new bead should be created that tracks the actual implementation of twerk-core with the eval module.