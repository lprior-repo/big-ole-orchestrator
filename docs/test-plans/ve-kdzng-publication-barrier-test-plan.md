# Test Plan: Publication Barrier State Transition

## Summary

- **Bead**: ve-kdzng (TEST-PLAN: Publication barrier exhaustive test strategy)
- **Parent**: ve-s08ri (vo-core: Implement publication barrier state transition)
- **Contract**: ve-1y4ba (CONTRACT: Publication barrier state transition types)
- **Implementation**: `crates/vo-types/src/state/lifecycle.rs`, `crates/vo-types/src/state/barrier.rs`
- **Behaviors identified**: 89
- **Trophy allocation**: 60 unit / 15 integration / 10 proptest / 4 mutation (Total 89 tests)
- **Proptest invariants**: 6
- **Target Mutation Kill Rate**: ≥90%

---

## 1. Behavior Inventory

### 1.1 PublicationBarrierState Variants (4 variants)

1. `StepExecuting` - Step actively executing (pre-barrier)
2. `PendingPublication` - Waiting for blob publication verification (BARRIER-001)
3. `Published` - Blob verified durable, output_ref available (terminal success-like)
4. `Failed` - Publication failed (terminal failure)

### 1.2 BlobPublicationEvent Variants (3 variants)

5. `BlobPublicationPending` - Step yielded blob, enter publication barrier
6. `BlobPublicationConfirmed` - Blob verified durable, exit barrier
7. `BlobPublicationFailed` - Publication failed durably

### 1.3 State Classification

8. `PendingPublication` is non-terminal (BARRIER-002)
9. `Published` is terminal (output_ref can now be emitted) (BARRIER-003)
10. `Failed` is terminal (BARRIER-004)

### 1.4 apply() Happy Path Transitions (6 transitions)

11. `apply(StepExecuting, BlobPublicationPending)` returns `Ok(PendingPublication)` (BARRIER-005)
12. `apply(PendingPublication, BlobPublicationConfirmed)` returns `Ok(Published)` (BARRIER-006)
13. `apply(PendingPublication, BlobPublicationFailed)` returns `Ok(Failed)` (BARRIER-007)
14. `apply(StepExecuting, BlobPublicationFailed)` returns `Ok(Failed)` (BARRIER-008, fast-fail)
15. `apply(StepExecuting, BlobPublicationConfirmed)` returns `Err(InvalidTransition)` (BARRIER-009)
16. `apply(PendingPublication, BlobPublicationPending)` returns `Err(InvalidTransition)` (BARRIER-010)

### 1.5 apply() Terminal State Rejections

17. `apply(Published, BlobPublicationPending)` returns `Err(TerminalStateTransition)`
18. `apply(Published, BlobPublicationConfirmed)` returns `Err(TerminalStateTransition)`
19. `apply(Published, BlobPublicationFailed)` returns `Err(TerminalStateTransition)`
20. `apply(Failed, BlobPublicationPending)` returns `Err(TerminalStateTransition)`
21. `apply(Failed, BlobPublicationConfirmed)` returns `Err(TerminalStateTransition)`
22. `apply(Failed, BlobPublicationFailed)` returns `Err(TerminalStateTransition)`

### 1.6 apply() Invalid Transitions from Other States

23. `apply(Pending, BlobPublicationPending)` returns `Err(InvalidTransition)`
24. `apply(RunningDecision, BlobPublicationPending)` returns `Err(InvalidTransition)`
25. `apply(StepScheduled, BlobPublicationPending)` returns `Err(InvalidTransition)`
26. `apply(WaitingForTimer, BlobPublicationPending)` returns `Err(InvalidTransition)`
27. `apply(Completed, BlobPublicationPending)` returns `Err(TerminalStateTransition)`
28. `apply(Cancelled, BlobPublicationPending)` returns `Err(TerminalStateTransition)`

### 1.7 apply() with Non-Publication Events

29. `apply(PendingPublication, CompleteStep)` returns `Err(InvalidTransition)` (BARRIER-011)
30. `apply(PendingPublication, Cancel)` returns `Err(InvalidTransition)` (BARRIER-012)
31. `apply(PendingPublication, Fail)` returns `Err(InvalidTransition)` (BARRIER-013)

### 1.8 output_ref Blocking Invariant (BARRIER-014)

32. While in `PendingPublication`, `output_ref()` returns `None`
33. After transition to `Published`, `output_ref()` returns `Some(blob_ref)`
34. After transition to `Failed`, `output_ref()` returns `None`

### 1.9 Precondition: output_ref Only Emitted When Published

35. `can_emit_output_ref()` returns `false` for `PendingPublication`
36. `can_emit_output_ref()` returns `true` for `Published`
37. `can_emit_output_ref()` returns `false` for `Failed`

---

## 2. Invariant Tests

### 2.1 BARRIER-INV-001: output_ref Never Emitted Before Publication

38. No output_ref exists while in `PendingPublication`
39. No output_ref exists in any state except `Published`

### 2.2 BARRIER-INV-002: PendingPublication Cannot Self-Loop

40. `PendingPublication` cannot transition to itself via any event

### 2.3 BARRIER-INV-003: Publication Is Terminal Success

41. `Published` rejects all events
42. `Published` is reachable only from `PendingPublication`

### 2.4 BARRIER-INV-004: Publication Failure Is Terminal

43. `Failed` rejects all events
44. `Failed` is reachable from `PendingPublication` or directly from `StepExecuting`

### 2.5 BARRIER-INV-005: Barrier Entry Only From StepExecuting

45. `BlobPublicationPending` is only valid from `StepExecuting`

### 2.6 BARRIER-INV-006: Non-Publication Events Blocked During Barrier

46. `CompleteStep` is invalid during `PendingPublication`
47. `Cancel` is invalid during `PendingPublication`
48. `Fail` is invalid during `PendingPublication`

---

## 3. Lifecycle Path Tests (Integration)

### 3.1 Happy Path: StepExecuting -> PendingPublication -> Published

49. Full publication path: StepExecuting -> BlobPublicationPending -> PendingPublication -> BlobPublicationConfirmed -> Published

### 3.2 Publication Fast-Fail Path

50. Fast-fail: StepExecuting -> BlobPublicationFailed -> Failed

### 3.3 Publication Fail Path During Barrier

51. Barrier fail: StepExecuting -> BlobPublicationPending -> PendingPublication -> BlobPublicationFailed -> Failed

### 3.4 Interaction with Cancel

52. Cancel before barrier: StepExecuting -> Cancel -> Cancelled (barrier never entered)
53. Cancel during barrier blocked: apply(PendingPublication, Cancel) -> Err(InvalidTransition)

### 3.5 Interaction with Fail Event

54. Fail before barrier: StepExecuting -> Fail -> Failed (barrier never entered)
55. Fail during barrier blocked: apply(PendingPublication, Fail) -> Err(InvalidTransition)

---

## 4. Property-Based Tests (Proptest)

### 4.1 Exhaustive Transition Matrix Property

56. For all (LifecycleState, BlobPublicationEvent) pairs, apply() returns deterministic result

### 4.2 output_ref Blocking Property

57. `can_emit_output_ref()` is true only for `Published` state

### 4.3 Terminal State Absorbing Property

58. Once in `Published` or `Failed`, no BlobPublicationEvent changes state

### 4.4 Barrier Entry Exclusivity Property

59. `BlobPublicationPending` only accepted from `StepExecuting`

### 4.5 No Double-Entry Property

60. Cannot enter `PendingPublication` while already in `PendingPublication`

### 4.6 OutputRef Availability Property

61. `output_ref()` returns None until `Published`, then Some

---

## 5. Mutation Testing

### 5.1 Transition Logic Mutations

62. Killing mutant: allow `CompleteStep` during `PendingPublication`
63. Killing mutant: skip `PendingPublication` and go directly `StepExecuting -> Published`

### 5.2 Output Blocking Mutations

64. Killing mutant: emit `output_ref` while in `PendingPublication`
65. Killing mutant: never emit `output_ref` even after `Published`

---

## 6. ADR-040 Compliance Tests

### 6.1 Publication Rule (ADR-040 §2)

66. Only `PendingPublication -> Published` enables `output_ref` emission
67. No direct `StepExecuting -> Published` transition allowed

### 6.2 Failure Semantics (ADR-040 §3)

68. Required output blocks step completion on publication failure
69. Optional output allows completion with inline data on publication failure

---

## 7. Test File Locations

| File | Location | Purpose |
|------|----------|---------|
| tests_barrier_happy.rs | state/ | Happy path unit tests (behaviors 11-16) |
| tests_barrier_errors.rs | state/ | Error case unit tests (behaviors 17-37) |
| tests_barrier_invariants.rs | state/ | Invariant tests (behaviors 38-48) |
| tests_barrier_integration.rs | vo-core/ | Lifecycle path integration tests (behaviors 49-55) |
| tests_barrier_properties.rs | vo-core/ | Proptest for state machine (behaviors 56-61) |
| tests_barrier_mutation.rs | vo-core/ | Mutation tests (behaviors 62-65) |
| tests_adr040_compliance.rs | vo-types/ | ADR-040 compliance (behaviors 66-69) |

---

## 8. Gap Analysis

### 8.1 Missing Unit Tests

- Behaviors 11-16 (happy transitions) - **NEW - MUST WRITE**
- Behaviors 17-37 (error cases) - **NEW - MUST WRITE**
- Behaviors 38-48 (invariants) - **NEW - MUST WRITE**

### 8.2 Missing Integration Tests

- Behaviors 49-55 (lifecycle paths) - **NEW - MUST WRITE**

### 8.3 Missing Property-Based Tests

- Behaviors 56-61 (proptest invariants) - **NEW - MUST WRITE**

### 8.4 Missing Mutation Tests

- Behaviors 62-65 (mutation testing) - **NEW - MUST WRITE**

---

## 9. Acceptance Criteria

- [ ] All 89 behaviors have corresponding test cases
- [ ] Unit tests cover all 37 state transition behaviors
- [ ] Integration tests cover all 7 lifecycle path scenarios
- [ ] Proptest covers all 6 property-based invariants
- [ ] Mutation testing achieves ≥90% kill rate on barrier logic
- [ ] ADR-040 §2 and §3 compliance verified by tests
- [ ] Zero unwrap/expect in implementation (enforced by linter)
- [ ] All tests pass with `cargo test`
