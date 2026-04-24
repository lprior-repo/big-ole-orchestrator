# ARCH-DRIFT Batch 7 Findings

## Summary
**STATUS: CRITICAL DRIFT DETECTED**

The veloxide codebase at `/home/lewis/gt/veloxide/` has significant architectural drift including deleted critical files and oversized modules violating the 300-line limit.

---

## Critical Issues: Deleted Files

The following **critical configuration files have been deleted** from the veloxide repository and not restored:

| File | Severity | Impact |
|------|----------|--------|
| `AGENTS.md` | CRITICAL | Agent instructions lost - threatens autonomous operation |
| `CLAUDE.md` | CRITICAL | Project context lost - AI understanding degraded |
| `.gitignore` | HIGH | Build artifacts may be committed |
| `.beads/README.md` | HIGH | Beads setup documentation lost |
| `.beads/config.yaml` | HIGH | Beads configuration lost |
| `.beads/hooks/*` | HIGH | Git hooks lost (5 hook files) |
| `.beads/metadata.json` | HIGH | Beads metadata lost |
| `.claude/settings.json` | HIGH | Claude settings lost |
| `polecats/brahmin/.beads/ve-1kao/findings.md` | MEDIUM | Prior bead findings lost |

**Recommendation**: Restore all deleted files immediately from git history.

---

## File Size Violations (>300 lines)

The following files violate the architectural rule that files should be ≤300 lines:

### Tier 1 - Critical (1000+ lines)
| File | Lines | Module |
|------|-------|--------|
| `mayor/rig/crates/vo-actor/src/lib.rs` | 1923 | vo-actor |
| `mayor/rig/crates/vo-actor/src/message_router.rs` | 1419 | vo-actor |
| `mayor/rig/crates/vo-actor/src/probe/types.rs` | 1351 | vo-actor |
| `mayor/rig/crates/vo-actor/src/instance_registry_tests.rs` | 1324 | vo-actor (test file) |
| `mayor/rig/crates/vo-actor/src/lifecycle.rs` | 1017 | vo-actor |

### Tier 2 - High (500-999 lines)
| File | Lines | Module |
|------|-------|--------|
| `mayor/rig/crates/vo-actor/src/reanimator/recovery_tests.rs` | 911 | vo-actor |
| `mayor/rig/crates/vo-actor/src/actor_messages/tests.rs` | 798 | vo-actor |
| `mayor/rig/crates/vo-actor/src/heartbeat.rs` | 515 | vo-actor |

### Tier 3 - Medium (300-499 lines)
| File | Lines | Module |
|------|-------|--------|
| `mayor/rig/crates/vo-actor/src/semaphore/execution.rs` | 487 | vo-actor |
| `mayor/rig/crates/vo-actor/src/reanimator/tests.rs` | 464 | vo-actor |
| `mayor/rig/crates/vo-actor/src/signal_buffer.rs` | 458 | vo-actor |
| `mayor/rig/crates/vo-actor/src/reanimator/loop_core.rs` | 420 | vo-actor |
| `mayor/rig/crates/vo-actor/src/control_actor.rs` | 354 | vo-actor |
| `mayor/rig/crates/vo-actor/src/instance_registry.rs` | 320 | vo-actor |
| `mayor/rig/crates/vo-actor/src/budget.rs` | 314 | vo-actor |
| `mayor/rig/crates/vo-actor/src/async_message_router.rs` | 305 | vo-actor |

**Total violation count**: 16 files exceed 300 lines.

---

## Root Cause Analysis

1. **Deleted critical files**: Someone ran `git checkout` or `git reset` that removed files, or files were never committed after creation
2. **Large modules**: The vo-actor crate is monolithic - 1923 line lib.rs is a code smell indicating poor separation of concerns

---

## Required Actions

### Immediate (P0)
1. **Restore deleted files**:
   ```bash
   git checkout HEAD -- AGENTS.md CLAUDE.md .gitignore .beads .claude
   ```
2. **Restore brahmin findings**:
   ```bash
   git checkout HEAD -- polecats/brahmin/.beads/ve-1kao/findings.md
   ```

### Short-term (P1)
3. **Split vo-actor/lib.rs**: Extract ports, supervisors, and storage into separate modules
4. **Split vo-actor/message_router.rs**: Extract routing logic into dedicated router crate
5. **Split vo-actor/probe/types.rs**: This file likely contains multiple type families that should be separated

---

## DDD Observations

The vo-actor module appears to violate several Scott Wlaschin DDD principles:
- **Primitive obsession**: `message_router.rs` likely uses raw `String` types instead of newtypes for message addresses
- **Large modules**: 1923 lines in lib.rs indicates poor bounded context separation
- **State machines**: Need to verify if lifecycle.rs properly models states as types

---

*Findings compiled: 2026-04-24*
*Arch-drift batch 7 - polecat fury*
