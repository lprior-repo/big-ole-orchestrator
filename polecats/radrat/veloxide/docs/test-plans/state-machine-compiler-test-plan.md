# Test Plan: State Machine Compiler

## Summary

- **Bead**: ve-803f (Test Plan: State machine compiler)
- **Contract**: ve-t9yr (Contract: State machine compiler)
- **Implementation**: `crates/vo-types/src/state/lifecycle.rs`, `crates/vo-types/src/state/transition.rs`, `crates/vo-types/src/lifecycle_superstate.rs`
- **Behaviors identified**: 156
- **Trophy allocation**: 120 unit / 20 integration / 10 proptest / 6 mutation (Total 156 tests)
- **Proptest invariants**: 8
- **Target Mutation Kill Rate**: ≥90%

---

## 1. Behavior Inventory

### 1.1 LifecycleState Variants (8 variants)

1. `Pending` - Initial state, bead queued, not yet assigned
2. `RunningDecision` - Decision phase, evaluating which step to execute
3. `StepScheduled` - Step scheduled but not yet executing
4. `StepExecuting` - Step actively executing
5. `WaitingForTimer` - Waiting for external timer/callback
6. `Completed` - Terminal state, bead completed successfully
7. `Failed` - Terminal state, bead failed
8. `Cancelled` - Terminal state, bead was cancelled

### 1.2 LifecycleState::is_terminal()

9. `is_terminal()` returns `true` for `Completed`
10. `is_terminal()` returns `true` for `Failed`
11. `is_terminal()` returns `true` for `Cancelled`
12. `is_terminal()` returns `false` for `Pending`
13. `is_terminal()` returns `false` for `RunningDecision`
14. `is_terminal()` returns `false` for `StepScheduled`
15. `is_terminal()` returns `false` for `StepExecuting`
16. `is_terminal()` returns `false` for `WaitingForTimer`

### 1.3 LifecycleState::get_operational_status()

17. `get_operational_status()` returns `Healthy` for `Pending`
18. `get_operational_status()` returns `Healthy` for `RunningDecision`
19. `get_operational_status()` returns `Healthy` for `StepScheduled`
20. `get_operational_status()` returns `Healthy` for `StepExecuting`
21. `get_operational_status()` returns `Healthy` for `WaitingForTimer`
22. `get_operational_status()` returns `Recovering` for `Failed`
23. `get_operational_status()` returns `Blocked(BlockedReason::ManualHold)` for `Completed`
24. `get_operational_status()` returns `Blocked(BlockedReason::ManualHold)` for `Cancelled`

### 1.4 LifecycleState::superstate() Mapping (INV-008)

25. `superstate()` returns `Active` for `Pending` (INV-008)
26. `superstate()` returns `Active` for `RunningDecision` (INV-008)
27. `superstate()` returns `Active` for `StepScheduled` (INV-008)
28. `superstate()` returns `Active` for `StepExecuting` (INV-008)
29. `superstate()` returns `Suspended` for `WaitingForTimer` (INV-008)
30. `superstate()` returns `Terminal` for `Completed` (INV-008)
31. `superstate()` returns `Terminal` for `Failed` (INV-008)
32. `superstate()` returns `Terminal` for `Cancelled` (INV-008)

### 1.5 TransitionEvent Variants (10 variants)

33. `TransitionEvent::AssignToNode` - From Pending
34. `TransitionEvent::Cancel` - From any non-terminal
35. `TransitionEvent::StepScheduled` - From RunningDecision
36. `TransitionEvent::Fail` - From RunningDecision, StepScheduled, StepExecuting, WaitingForTimer
37. `TransitionEvent::ExecuteStep` - From StepScheduled
38. `TransitionEvent::WaitForTimer` - From StepExecuting
39. `TransitionEvent::CompleteStep` - From StepExecuting
40. `TransitionEvent::TimerFired` - From WaitingForTimer
41. `TransitionEvent::TimerExpired` - From WaitingForTimer
42. `TransitionEvent::InstanceResumed` - From Failed only (INV-003)

### 1.6 LifecycleState::get_valid_transitions()

43. `get_valid_transitions()` returns `[AssignToNode, Cancel]` for `Pending`
44. `get_valid_transitions()` returns `[StepScheduled, Cancel, Fail]` for `RunningDecision`
45. `get_valid_transitions()` returns `[ExecuteStep, Cancel, Fail]` for `StepScheduled`
46. `get_valid_transitions()` returns `[WaitForTimer, CompleteStep, Cancel, Fail]` for `StepExecuting`
47. `get_valid_transitions()` returns `[TimerFired, TimerExpired, Cancel, Fail]` for `WaitingForTimer`
48. `get_valid_transitions()` returns `[]` for `Completed`
49. `get_valid_transitions()` returns `[]` for `Cancelled`
50. `get_valid_transitions()` returns `[InstanceResumed]` for `Failed`

### 1.7 TransitionEvent::all_variants()

51. `all_variants()` returns exactly 10 variants
52. `all_variants()` contains all defined TransitionEvent variants

### 1.8 apply() Happy Path Transitions (15 transitions)

53. `apply(Pending, AssignToNode)` returns `Ok(RunningDecision)` (INV-002)
54. `apply(Pending, Cancel)` returns `Ok(Cancelled)` (INV-004)
55. `apply(RunningDecision, StepScheduled)` returns `Ok(StepScheduled)` (INV-002)
56. `apply(RunningDecision, Cancel)` returns `Ok(Cancelled)` (INV-004)
57. `apply(RunningDecision, Fail)` returns `Ok(Failed)` (INV-005)
58. `apply(StepScheduled, ExecuteStep)` returns `Ok(StepExecuting)` (INV-002)
59. `apply(StepScheduled, Cancel)` returns `Ok(Cancelled)` (INV-004)
60. `apply(StepScheduled, Fail)` returns `Ok(Failed)` (INV-005)
61. `apply(StepExecuting, WaitForTimer)` returns `Ok(WaitingForTimer)` (INV-002)
62. `apply(StepExecuting, CompleteStep)` returns `Ok(Completed)` (INV-006)
63. `apply(StepExecuting, Cancel)` returns `Ok(Cancelled)` (INV-004)
64. `apply(StepExecuting, Fail)` returns `Ok(Failed)` (INV-005)
65. `apply(WaitingForTimer, TimerFired)` returns `Ok(StepExecuting)` (INV-002)
66. `apply(WaitingForTimer, TimerExpired)` returns `Ok(Failed)` (INV-007)
67. `apply(WaitingForTimer, Cancel)` returns `Ok(Cancelled)` (INV-004)
68. `apply(WaitingForTimer, Fail)` returns `Ok(Failed)` (INV-007)

### 1.9 apply() Recovery Transition

69. `apply(Failed, InstanceResumed)` returns `Ok(RunningDecision)` (INV-003)

### 1.10 apply() Terminal State Rejections (INV-001, INV-009)

70. `apply(Completed, AssignToNode)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
71. `apply(Completed, Cancel)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
72. `apply(Completed, StepScheduled)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
73. `apply(Completed, ExecuteStep)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
74. `apply(Completed, WaitForTimer)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
75. `apply(Completed, CompleteStep)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
76. `apply(Completed, TimerFired)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
77. `apply(Completed, TimerExpired)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
78. `apply(Completed, Fail)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
79. `apply(Completed, InstanceResumed)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)

80. `apply(Cancelled, AssignToNode)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
81. `apply(Cancelled, Cancel)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
82. `apply(Cancelled, StepScheduled)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
83. `apply(Cancelled, ExecuteStep)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
84. `apply(Cancelled, WaitForTimer)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
85. `apply(Cancelled, CompleteStep)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
86. `apply(Cancelled, TimerFired)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
87. `apply(Cancelled, TimerExpired)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
88. `apply(Cancelled, Fail)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)
89. `apply(Cancelled, InstanceResumed)` returns `Err(TerminalStateTransition)` (INV-001, INV-009)

### 1.11 apply() Invalid Transition from Non-Terminal States (INV-010)

90. `apply(Pending, StepScheduled)` returns `Err(InvalidTransition)` (INV-010)
91. `apply(Pending, ExecuteStep)` returns `Err(InvalidTransition)` (INV-010)
92. `apply(Pending, WaitForTimer)` returns `Err(InvalidTransition)` (INV-010)
93. `apply(Pending, CompleteStep)` returns `Err(InvalidTransition)` (INV-010)
94. `apply(Pending, TimerFired)` returns `Err(InvalidTransition)` (INV-010)
95. `apply(Pending, TimerExpired)` returns `Err(InvalidTransition)` (INV-010)
96. `apply(Pending, Fail)` returns `Err(InvalidTransition)` (INV-010)
97. `apply(Pending, InstanceResumed)` returns `Err(InvalidTransition)` (INV-010)

98. `apply(RunningDecision, AssignToNode)` returns `Err(InvalidTransition)` (INV-010)
99. `apply(RunningDecision, ExecuteStep)` returns `Err(InvalidTransition)` (INV-010)
100. `apply(RunningDecision, WaitForTimer)` returns `Err(InvalidTransition)` (INV-010)
101. `apply(RunningDecision, CompleteStep)` returns `Err(InvalidTransition)` (INV-010)
102. `apply(RunningDecision, TimerFired)` returns `Err(InvalidTransition)` (INV-010)
103. `apply(RunningDecision, TimerExpired)` returns `Err(InvalidTransition)` (INV-010)
104. `apply(RunningDecision, InstanceResumed)` returns `Err(InvalidTransition)` (INV-010)

105. `apply(StepScheduled, AssignToNode)` returns `Err(InvalidTransition)` (INV-010)
106. `apply(StepScheduled, StepScheduled)` returns `Err(InvalidTransition)` (INV-010)
107. `apply(StepScheduled, WaitForTimer)` returns `Err(InvalidTransition)` (INV-010)
108. `apply(StepScheduled, CompleteStep)` returns `Err(InvalidTransition)` (INV-010)
109. `apply(StepScheduled, TimerFired)` returns `Err(InvalidTransition)` (INV-010)
110. `apply(StepScheduled, TimerExpired)` returns `Err(InvalidTransition)` (INV-010)
111. `apply(StepScheduled, InstanceResumed)` returns `Err(InvalidTransition)` (INV-010)

112. `apply(StepExecuting, AssignToNode)` returns `Err(InvalidTransition)` (INV-010)
113. `apply(StepExecuting, StepScheduled)` returns `Err(InvalidTransition)` (INV-010)
114. `apply(StepExecuting, ExecuteStep)` returns `Err(InvalidTransition)` (INV-010)
115. `apply(StepExecuting, CompleteStep)` returns `Err(InvalidTransition)` (INV-010)
116. `apply(StepExecuting, TimerFired)` returns `Err(InvalidTransition)` (INV-010)
117. `apply(StepExecuting, TimerExpired)` returns `Err(InvalidTransition)` (INV-010)
118. `apply(StepExecuting, InstanceResumed)` returns `Err(InvalidTransition)` (INV-010)

119. `apply(WaitingForTimer, AssignToNode)` returns `Err(InvalidTransition)` (INV-010)
120. `apply(WaitingForTimer, StepScheduled)` returns `Err(InvalidTransition)` (INV-010)
121. `apply(WaitingForTimer, ExecuteStep)` returns `Err(InvalidTransition)` (INV-010)
122. `apply(WaitingForTimer, WaitForTimer)` returns `Err(InvalidTransition)` (INV-010)
123. `apply(WaitingForTimer, CompleteStep)` returns `Err(InvalidTransition)` (INV-010)
124. `apply(WaitingForTimer, InstanceResumed)` returns `Err(InvalidTransition)` (INV-010)

125. `apply(Failed, AssignToNode)` returns `Err(TerminalStateTransition)` (INV-001)
126. `apply(Failed, Cancel)` returns `Err(TerminalStateTransition)` (INV-001)
127. `apply(Failed, StepScheduled)` returns `Err(TerminalStateTransition)` (INV-001)
128. `apply(Failed, ExecuteStep)` returns `Err(TerminalStateTransition)` (INV-001)
129. `apply(Failed, WaitForTimer)` returns `Err(TerminalStateTransition)` (INV-001)
130. `apply(Failed, CompleteStep)` returns `Err(TerminalStateTransition)` (INV-001)
131. `apply(Failed, TimerFired)` returns `Err(TerminalStateTransition)` (INV-001)
132. `apply(Failed, TimerExpired)` returns `Err(TerminalStateTransition)` (INV-001)
133. `apply(Failed, Fail)` returns `Err(TerminalStateTransition)` (INV-001)

---

## 2. Invariant Tests (INV-001 through INV-010)

### 2.1 INV-001: Terminal states reject all transitions except InstanceResumed from Failed

134. Terminal states (Completed, Cancelled) reject all events except InstanceResumed (INV-001)
135. Failed state rejects all events except InstanceResumed (INV-001)

### 2.2 INV-002: No self-loops or cycles in the state transition graph

136. No state transitions to itself via valid events (no self-loops)
137. No cycles exist in the transition graph except the Failed -> InstanceResumed -> RunningDecision recovery path

### 2.3 INV-003: InstanceResumed is only valid from Failed state

138. InstanceResumed is invalid from Pending
139. InstanceResumed is invalid from RunningDecision
140. InstanceResumed is invalid from StepScheduled
141. InstanceResumed is invalid from StepExecuting
142. InstanceResumed is invalid from WaitingForTimer
143. InstanceResumed is invalid from Completed
144. InstanceResumed is invalid from Cancelled
145. InstanceResumed is valid from Failed (returns RunningDecision)

### 2.4 INV-004: Cancel is valid from all non-terminal states

146. Cancel is valid from Pending
147. Cancel is valid from RunningDecision
148. Cancel is valid from StepScheduled
149. Cancel is valid from StepExecuting
150. Cancel is valid from WaitingForTimer
151. Cancel is invalid from Completed
152. Cancel is invalid from Failed
153. Cancel is invalid from Cancelled

### 2.5 INV-005: Fail is valid from eligible states

154. Fail is invalid from Pending
155. Fail is valid from RunningDecision
156. Fail is valid from StepScheduled
157. Fail is valid from StepExecuting
158. Fail is valid from WaitingForTimer
159. Fail is invalid from Completed
160. Fail is invalid from Cancelled

### 2.6 INV-006: Completed is only reachable via CompleteStep from StepExecuting

161. Completed cannot be reached from Pending
162. Completed cannot be reached from RunningDecision
163. Completed cannot be reached from StepScheduled
164. Completed cannot be reached from WaitingForTimer
165. Completed can only be reached from StepExecuting via CompleteStep

### 2.7 INV-007: Failed is reachable via Fail OR via TimerExpired

166. Failed can be reached from RunningDecision via Fail
167. Failed can be reached from StepScheduled via Fail
168. Failed can be reached from StepExecuting via Fail
169. Failed can be reached from WaitingForTimer via Fail
170. Failed can be reached from WaitingForTimer via TimerExpired
171. Failed cannot be reached from Pending
172. Failed cannot be reached from StepExecuting via TimerExpired

### 2.8 INV-008: Superstate mapping consistency

173. Superstate consistency: all Active states map to Active superstate
174. Superstate consistency: WaitingForTimer maps to Suspended superstate
175. Superstate consistency: Terminal states map to Terminal superstate

### 2.9 INV-009: apply() returns TerminalStateTransition for terminal state transitions

176. INV-009 is verified by behaviors 70-89

### 2.10 INV-010: apply() returns InvalidTransition for invalid (state, event) pairs

177. INV-010 is verified by behaviors 90-133

---

## 3. Lifecycle Path Tests (Integration)

### 3.1 Happy Path: Pending -> Completed

178. Full happy path: Pending -> AssignToNode -> RunningDecision -> StepScheduled -> ExecuteStep -> StepExecuting -> CompleteStep -> Completed

### 3.2 Fail Path via Fail Event

179. Fail from RunningDecision: Pending -> AssignToNode -> RunningDecision -> Fail -> Failed
180. Fail from StepScheduled: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed
181. Fail from StepExecuting: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed
182. Fail from WaitingForTimer: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> Fail -> Failed

### 3.3 Fail Path via TimerExpired

183. TimerExpired path: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> TimerExpired -> Failed

### 3.4 Cancel Path from Various States

184. Cancel from Pending: Pending -> Cancel -> Cancelled
185. Cancel from RunningDecision: Pending -> AssignToNode -> RunningDecision -> Cancel -> Cancelled
186. Cancel from StepScheduled: Pending -> AssignToNode -> StepScheduled -> Cancel -> Cancelled
187. Cancel from StepExecuting: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> StepExecuting -> Cancel -> Cancelled
188. Cancel from WaitingForTimer: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> Cancel -> Cancelled

### 3.5 Recovery Path

189. Recovery path: Pending -> AssignToNode -> RunningDecision -> StepScheduled -> ExecuteStep -> StepExecuting -> Fail -> Failed -> InstanceResumed -> RunningDecision

### 3.6 Timer Firing Path

190. Timer firing path: Pending -> AssignToNode -> StepScheduled -> ExecuteStep -> WaitForTimer -> TimerFired -> StepExecuting

---

## 4. Property-Based Tests (Proptest)

### 4.1 Exhaustive Transition Matrix Property

191. For all LifecycleState variants and all TransitionEvent variants, apply() returns a deterministic result (either Ok or Err)

### 4.2 Superstate Consistency Property

192. For all non-terminal states, the superstate after apply() is consistent with the state transition

### 4.3 Terminal State Absorbing Property

193. Once a state is terminal (Completed, Failed, Cancelled), it remains terminal regardless of applied event

### 4.4 OperationalStatus Consistency Property

194. OperationalStatus is consistent with the state classification

### 4.5 No Invalid Self-Transitions Property

195. No state can transition to itself via any valid event

### 4.6 Valid Transitions Completeness Property

196. get_valid_transitions() returns exactly the events that apply() accepts

### 4.7 InstanceResumed Only From Failed Property

197. InstanceResumed is only valid when current state is Failed

### 4.8 Cancel Universality Property

198. Cancel is accepted from all non-terminal states

---

## 5. Mutation Testing

### 5.1 Transition Logic Mutations

199. Killing mutant: changing Completed to RunningDecision in apply() match arm
200. Killing mutant: changing Failed terminal rejection to accept InstanceResumed from Cancelled
201. Killing mutant: removing Cancel from Pending valid transitions

### 5.2 State Classification Mutations

202. Killing mutant: changing is_terminal() to return false for Completed
203. Killing mutant: changing superstate() to return Active for WaitingForTimer

### 5.3 Event Vocabulary Mutations

204. Killing mutant: removing variants from TransitionEvent::all_variants()

---

## 6. Existing Test Coverage Summary

| File | Tests | Coverage |
|------|-------|----------|
| tests_apply_happy.rs | 15 | Happy path transitions |
| tests_apply_errors.rs | 43 | Terminal rejections + InvalidTransition |
| tests_helpers.rs | 20 | Helper functions |
| tests_derives.rs | 22 | Derives, semantic types, TransitionError |
| tests_lease.rs | 21 | LeaseRecord |
| lifecycle_superstate.rs | 15 | Superstate mapping |
| tests_proptest.rs | 1 | LeaseRecord immutability |
| **Total** | **137** | |

---

## 7. Gap Analysis

### 7.1 Missing Unit Tests

- Missing: behaviors 90-133 (InvalidTransition from non-terminal states) - **COVERED** in tests_apply_errors.rs
- Missing: behaviors 70-89 (Terminal rejections) - **COVERED** in tests_apply_errors.rs
- Missing: InstanceResumed from Failed rejection (behavior 133) - **COVERED** in tests_apply_errors.rs

### 7.2 Missing Integration Tests

- Lifecycle path tests (178-190) - **NOT COVERED**
- Full happy path sequence - **NOT COVERED**
- Recovery path sequence - **NOT COVERED**

### 7.3 Missing Property-Based Tests

- Exhaustive transition matrix (8 states x 10 events = 80 pairs) - **NOT COVERED**
- Superstate consistency property - **NOT COVERED**
- Terminal state absorbing property - **NOT COVERED**
- OperationalStatus consistency property - **NOT COVERED**
- Valid transitions completeness property - **NOT COVERED**
- InstanceResumed only from Failed property - **NOT COVERED**
- Cancel universality property - **NOT COVERED**

### 7.4 Missing Mutation Tests

- No mutation testing infrastructure identified

---

## 8. Implementation Recommendations

### 8.1 New Test File: tests_integration_paths.rs

Create integration tests for lifecycle path sequences:
- `tests_integration_paths.rs` - Full path tests (behaviors 178-190)

### 8.2 New Test File: tests_properties.rs

Create property-based tests:
- `tests_properties.rs` - Proptest for transition matrix and invariants

### 8.3 Mutation Testing

Consider integrating `mutagen` or `cargo-mutants` for mutation testing.

---

## 9. Test File Locations

| File | Location | Purpose |
|------|----------|---------|
| tests_apply_happy.rs | state/ | Happy path unit tests |
| tests_apply_errors.rs | state/ | Error case unit tests |
| tests_helpers.rs | state/ | Helper function unit tests |
| tests_derives.rs | state/ | Derive macro unit tests |
| tests_lease.rs | state/ | LeaseRecord unit tests |
| tests_proptest.rs | state/ | Property-based tests |
| tests_integration_paths.rs | state/ | **NEW** - Lifecycle path integration tests |
| tests_properties.rs | state/ | **NEW** - Exhaustive property tests |

---

## 10. Acceptance Criteria

- [ ] All 137 existing tests continue to pass
- [ ] New integration tests cover lifecycle paths (13 behaviors)
- [ ] New property tests cover transition matrix exhaustiveness (8 behaviors)
- [ ] All 10 invariants (INV-001 through INV-010) have explicit test coverage
- [ ] Mutation testing achieves ≥90% kill rate on transition logic