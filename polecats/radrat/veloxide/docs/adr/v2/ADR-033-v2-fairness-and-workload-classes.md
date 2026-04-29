# ADR 033 (v2): Fairness and Workload Classes

## Status
Accepted

## Context
Global backpressure alone is not enough. One pathological workflow can consume every subprocess permit, saturate stderr budgets, and monopolize the write path, starving exact workflows and recovery.

## Decision
We add lightweight workload classes and fairness controls without pretending to provide full sandbox isolation.

### 1. Workload Classes
The Engine distinguishes at least:
1. `ExactCritical`
2. `Standard`
3. `UnsafeBulk`
4. `Recovery`

### 2. Permit and Queue Budgets
- Global execution permits remain bounded.
- Each class receives reserved budget.
- Individual workflows receive per-workflow caps for process permits, stderr budget, and bulk blob pressure.

### 3. Scheduling Policy
- `UnsafeBulk` workloads may not starve `ExactCritical` workloads.
- Recovery receives reserved capacity so crash recovery always makes forward progress.
- Load shedding decisions expose the class and budget reason so operators understand why work is being rejected.

## Consequences
- **Positive:** The Engine gains meaningful fairness without heavy process isolation or containers.
- **Positive:** Exact workflows and recovery remain serviceable during noisy-neighbor conditions.
- **Negative:** Scheduling and admission control become more complex than a single global semaphore.
