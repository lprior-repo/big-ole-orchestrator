# ADR-008 Review Findings: ve-fpou

## Issue
ADR-REVIEW: ADR-008 AI-native interfaces - Verify CLI JSON output for AI agents.

## ADR-008 Requirements

ADR-008 v2 specifies AI-native agent interfaces with these mandates:

1. **`vo-cli history <instance_id> --json`** — returns redacted operator projection (AI/UI path)
2. **`vo-cli history <instance_id> --canonical`** — privileged forensic path for exact replay
3. **API Contract Stability** — JSON schemas are immutable contracts

## Findings

### GAP 1: `history` subcommand is NOT implemented

The `vo-cli` CLI (at `crates/vo-cli/src/cli.rs`) has NO `history` subcommand:

**Registered commands in `cli.rs` and `registry.rs`:**
- purge
- check (with `--workflow` flag only, NOT `--json`)
- compensate
- gc
- init
- lock
- doctor
- rebuild
- status
- hardline

**NOT registered:**
- `history` — completely absent

### GAP 2: `history` module exists but is orphaned

The `vo-cli/src/commands/history.rs` module EXISTS with:
- `HistoryEntryOutput` struct
- `HistoryOutput` struct  
- `get_history()`, `load_history()`, `save_history()` functions
- `UndoResult`, `RedoResult` types

BUT this module is never wired into the CLI parser or dispatch.

### GAP 3: No `--json` flag on `check` command

The ADR mentions verifying `check --json`, but:
- `check` command only has `--workflow` flag
- No `--json` flag exists

### GAP 4: No `--canonical` flag

Neither `history` (if it existed) nor any other command has a `--canonical` flag for privileged forensic access.

## Files Examined

- `/home/lewis/src/veloxide/docs/adr/v2/ADR-008-v2-ai-native-agent-interfaces.md` — ADR source
- `/home/lewis/src/veloxide/crates/vo-cli/src/cli.rs` — CLI command definitions
- `/home/lewis/src/veloxide/crates/vo-cli/src/registry.rs` — command handlers
- `/home/lewis/src/veloxide/crates/vo-cli/src/commands/history.rs` — orphaned history module
- `/home/lewis/src/veloxide/crates/vo-cli/src/commands/check.rs` — check command implementation

## Verdict

**ADR-008 CLI JSON output is NOT implemented.** The `history` command with `--json` and `--canonical` flags is entirely missing from the CLI. The `history` module exists in isolation but has no CLI integration.

This is a QA/audit bead — no code changes were made. No git commit needed.
