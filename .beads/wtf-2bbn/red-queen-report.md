bead_id: wtf-2bbn
bead_title: integration test: Procedural checkpoint — ctx.activity() result survives crash
phase: red-queen-report
updated_at: 2026-03-22T03:16:00Z

# Red Queen Report: Procedural Crash Recovery

## Adversarial Test Scenarios

### 1. Duplicate Sequence Replay
**Scenario**: Same sequence number applied twice
**Expected**: Second application returns `AlreadyApplied`
**Result**: PASS - `applied_seq` check catches duplicates

### 2. Operation ID Mismatch
**Scenario**: ActivityCompleted for unknown activity_id
**Expected**: Returns `UnknownActivityId` error
**Result**: PASS - `in_flight` lookup fails correctly

### 3. Checkpoint Overwrite Attempt
**Scenario**: Complete same operation twice
**Expected**: First completes, second fails (not in in_flight)
**Result**: PASS - After first complete, op removed from in_flight

### 4. Determinism Violation
**Scenario**: Same events, different order
**Expected**: Different final state
**Result**: N/A - not applicable to this test

### 5. Large Operation Counter
**Scenario**: Many sequential operations
**Expected**: Counter increments correctly
**Result**: PASS - Counter is u32 and increments properly

## Verdict
**ALL DEFECTS CAUGHT** - No critical defects found in adversarial testing.
