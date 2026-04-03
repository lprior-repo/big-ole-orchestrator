# Architecture Spec: ADR Freeze-Set Bead Corpus Remediation

## 1. Purpose

This spec defines the remediation plan for the bead corpus so `arch-spec-to-beads` can generate a new, implementation-valid issue set.

This is not a product spec. It is a planning contract for rebuilding the bead graph so it matches:

1. the frozen ADR corpus under `docs/adr/v2/`,
2. `docs/ADR_DEPENDENCY_GRAPH.md`,
3. `docs/ADR_FREEZE_AUDIT.md`,
4. `docs/IMPLEMENTATION_BUILD_ORDER.md`, and
5. the actual workspace shape in `Cargo.toml`.

---

## 2. Problem Statement

The current bead corpus has drifted in three directions at once:

1. **Freeze-set coverage drift**
   - The semantic freeze set from `ADR_FREEZE_AUDIT.md` is:
     `001, 002, 003, 004, 012, 014, 016, 027, 028, 029, 030, 031, 032, 033, 034, 035, 036, 038, 039, 040, 041, 042, 043`.
   - Explicit bead coverage is missing or materially weak for several freeze-set ADRs.

2. **Workspace reality drift**
   - The actual workspace currently contains:
     `vo-types`, `vo-storage`, `vo-api`, `vo-cli`, `vo-worker`, `vo-frontend`, `vo-linter`, `vo-actor`, `vo-core`, `vo-common`, `vo-ipc`, `vo-sdk`.
   - There is **no `vo-engine` crate** in the workspace today.
   - Existing active beads still reference nonexistent crates, outdated crate layouts, and stale docs/file paths.

3. **Planning quality drift**
   - Active beads are concentrated around old scaffolding and bundled meta-work.
   - Several beads are too broad, depend on stale prerequisites, or encode obsolete assumptions.
   - The open/in-progress set is misaligned with the ADR build order and has no ready path to the exactly-once core.

The result is a deadlocked bead corpus: implementation agents would be steered toward stale crate names, wrong dependency order, and vague meta tasks instead of atomic work.

---

## 3. Current Observed Truth

### 3.1 Workspace truth

`Cargo.toml` is the primary source of truth for crate existence.

Current members:

- `crates/vo-types`
- `crates/vo-storage`
- `crates/vo-api`
- `crates/vo-cli`
- `crates/vo-worker`
- `crates/vo-frontend`
- `crates/vo-linter`
- `crates/vo-actor`
- `crates/vo-core`
- `crates/vo-common`
- `crates/vo-ipc`
- `crates/vo-sdk`

### 3.2 Drift signals already visible in repo

1. `README.md` still describes `sled`, `vo-engine`, and crate boundaries that do not match the workspace.
2. `CLAUDE.md` still describes `vo-engine` and `vo-ui`, neither of which matches current workspace membership.
3. Active beads still reference:
   - `vo-engine` crate creation,
   - legacy `docs/v2/*` paths,
   - stale composition-root assumptions,
   - bundled work that predates the ADR freeze set.

### 3.3 Bead tracker truth

Observed status summary:

- `59` total issues
- `13` open
- `3` in progress
- `0` ready

This is a planning smell: the active corpus is blocked and does not express a valid path through the freeze-set implementation order.

---

## 4. Target State

The remediated bead corpus shall satisfy all of the following:

1. **Freeze-set completeness**
   - Every freeze-set ADR has either:
     - explicit implementation beads, or
     - one intentional umbrella bead that only coordinates a tightly scoped family of atomic child beads.

2. **Workspace correctness**
   - No bead references nonexistent crates, files, or obsolete docs paths.
   - Crate targeting follows current workspace reality unless a new crate is introduced by an explicit prerequisite bead.

3. **Dependency correctness**
   - Dependencies align to `IMPLEMENTATION_BUILD_ORDER.md`.
   - No phase-N bead depends on phase-(N+1)+ work.

4. **Atomicity**
   - Beads are implementation-sized and testable.
   - No vague meta beads like “implement exact-once core” or “add workflow engine support”.

5. **Supersession hygiene**
   - Stale active beads are rewritten, split, or superseded instead of left open as traps.

6. **Anti-drift enforcement**
   - The new corpus encodes guardrails so future beads cannot drift back toward legacy `vo-engine` assumptions or phase-order violations.

---

## 5. EARS Requirements For The Remediation Corpus

### 5.1 Ubiquitous requirements

- THE SYSTEM SHALL treat `Cargo.toml` workspace membership as the source of truth for crate existence.
- THE SYSTEM SHALL treat `ADR_FREEZE_AUDIT.md` core freeze set as the minimum semantic coverage set.
- THE SYSTEM SHALL generate beads in build-order sequence, not in arbitrary topical order.
- THE SYSTEM SHALL require one ADR anchor per bead or one explicit umbrella anchor plus child ADR anchors.
- THE SYSTEM SHALL reject bead text that references nonexistent crates, nonexistent files, or stale docs paths.

### 5.2 Event-driven requirements

- WHEN a freeze-set ADR has no explicit implementation coverage, THE SYSTEM SHALL create a bead family for that ADR.
- WHEN an active bead references `vo-engine` as an existing crate, THE SYSTEM SHALL rewrite or supersede it before downstream planning proceeds.
- WHEN a bead spans multiple independent behaviors, THE SYSTEM SHALL split it into atomic beads with explicit dependencies.
- WHEN a bead dependency contradicts `IMPLEMENTATION_BUILD_ORDER.md`, THE SYSTEM SHALL rewrite the dependency edge.

### 5.3 Unwanted requirements

- IF a bead is only a coordination note with no code/test artifact, THE SYSTEM SHALL NOT keep it as implementation work.
- IF a bead requires a nonexistent crate without first creating that crate explicitly, THE SYSTEM SHALL NOT allow downstream beads to depend on it.
- IF a bead mixes multiple ADR layers in one task, THE SYSTEM SHALL NOT keep it bundled.
- IF a bead claims exact-once semantics without covering admission, fencing, journaling, and verification, THE SYSTEM SHALL NOT treat it as valid coverage.

---

## 6. Domain Contracts For New Beads

### 6.1 Bead validity contract

Every newly generated implementation bead must include:

1. one owning crate from the real workspace,
2. one primary ADR anchor,
3. one concrete artifact boundary,
4. explicit preconditions,
5. explicit postconditions,
6. at least one executable success criterion.

### 6.2 Atomicity contract

A bead is valid only if it changes exactly one of these implementation surfaces:

1. one domain type family,
2. one storage partition/codec path,
3. one actor/runtime state transition path,
4. one ingress/operator API surface,
5. one connector/runtime contract,
6. one verification harness slice,
7. one projection/query surface.

If a proposed bead spans more than one surface, it must be split.

### 6.3 Umbrella contract

Umbrella beads are allowed only when all are true:

1. the umbrella corresponds to one freeze-set ADR,
2. the umbrella has no direct implementation work of its own,
3. it exists only to hold ordered child beads,
4. child beads are atomic and independently closable.

---

## 7. Required Missing Freeze-Set Workstreams

The following ADRs require explicit new coverage.

### 7.1 Missing coverage ADRs

#### ADR-028 Exactly-once ingress deduplication
Required bead family:

1. command/dedupe key types and validation
2. dedupe partition keying and retention record model
3. atomic workflow-start admission path
4. atomic signal/approval admission path
5. rejection path for exact ingress without dedupe key
6. retention expiry and operator visibility

#### ADR-029 Execution leases and fencing
Required bead family:

1. lease record and fence token types
2. lease partition codec/storage path
3. fence acquisition before scheduling
4. stale completion rejection on write path
5. retry/recovery fence advancement

#### ADR-030 Managed effects and sink contracts
Required bead family:

1. `EffectIntent` / prepared / committed domain types
2. effect journal partition and event integration
3. exact workflow publish-time rejection for unsupported sinks
4. managed-effect execution path separated from unsafe activity path

#### ADR-034 Saga compensation and reversibility
Required bead family:

1. compensation policy in canonical workflow model
2. compensation lifecycle states and transitions
3. forward-effect to compensation-effect linkage
4. manual compensation command path
5. reverse dependency ordering logic

#### ADR-035 Event schema evolution and upcasting
Required bead family:

1. schema version fields on durable records
2. upcaster registry/chain interfaces
3. replay-time normalization path
4. snapshot compatibility/discard logic
5. projection compatibility window rules

#### ADR-036 Command identity, correlation, causation
Required bead family:

1. `CommandEnvelope` type family
2. command metadata propagation into events
3. operator/API mutation dedupe by `command_id`
4. issuer/correlation/causation query surfaces

#### ADR-038 Workflow lineage and continue-as-new
Required bead family:

1. lineage and epoch identifiers
2. continued-as-new event and atomic rollover path
3. lineage-aware query/routing model
4. dedupe/signal routing update across rollover

#### ADR-040 Canonical blob durability and publication
Required bead family:

1. canonical blob record/store abstraction
2. publication barrier before `output_ref`
3. routing-critical inline data rules vs blob rules
4. blob failure semantics and optional-output rules
5. blob retention/gc path

#### ADR-041 Managed connector runtime contract
Required bead family:

1. connector trait/runtime interfaces
2. prepare/commit/reconcile state machine
3. ambiguity handling and timeout states
4. receipt persistence requirements
5. first strong connector implementations only

#### ADR-043 Exact-once verification strategy
Required bead family:

1. crash-point matrix definitions
2. replay property tests
3. fencing stale-winner tests
4. connector ambiguity reconciliation tests
5. lineage/signal correctness tests
6. release-gate integration

### 7.2 Weak coverage ADRs

The following areas exist only partially and must be rewritten/split into stronger bead families.

#### ADR-031 Canonical `WorkflowSpec`
Required strengthening:

1. canonical schema type family in real crates
2. publish-time exact eligibility validation
3. signal/compensation/capability metadata coverage
4. SDK emission path and consumer parity path

#### ADR-032 Write-path QoS and hot/cold storage
Required strengthening:

1. write class taxonomy
2. separate queues/budgets for control-plane vs projections vs blobs
3. degraded-mode admission coupling
4. metrics for writer and blob pressure

#### ADR-033 Fairness and workload classes
Required strengthening:

1. workload class types
2. permit/reserved budget implementation
3. recovery reservation path
4. rejection reason surfacing

#### ADR-042 Signal matching and wake-up semantics
Required strengthening:

1. lineage-aware signal addressing model
2. wait-key matching rules
3. `Reject` / `BufferOne` / `BufferMany` bounded buffering behaviors
4. atomic accept-and-resume path
5. epoch-scoped vs lineage-scoped failure rules

---

## 8. Required Remediation Of Stale Active Beads

The following active beads are not valid as-is and must be rewritten, split, or superseded.

### 8.1 `vel-3fs`
- Problem: references nonexistent `vo-engine` crate scaffold as a prerequisite for broad downstream work.
- Required action: **supersede**.
- Replacement rule: either
  1. create an explicit new-crate decision bead first, or
  2. retarget composition-root work into existing crates only.

### 8.2 `vel-7dg`
- Problem: bundles legacy HTTP API around stale composition assumptions.
- Required action: **rewrite and split**.
- Replacement rule: separate ingress admission, operator mutation, query APIs, and exact-safe error mapping into atomic `vo-api` beads aligned to ADR-028/036/042.

### 8.3 `vel-60k`
- Problem: stale `vo-engine` endpoint framing and insufficient projection/query alignment.
- Required action: **rewrite**.
- Replacement rule: retarget to real query/projection surfaces and tie to ADR-037 plus actual API crate ownership.

### 8.4 `vel-sd1`
- Problem: load shedding work is underspecified and phase-mixed.
- Required action: **split**.
- Replacement rule: separate semaphore limits, workload class budgets, degraded mode, and admission reasons across ADR-006/013/032/033.

### 8.5 `vel-3hv`
- Problem: recovery sweep bead mixes orphan detection, startup gating, throttle, disk watchdog, and stale process assumptions.
- Required action: **split**.
- Replacement rule: separate recovery queue throttling, orphan process detection, degraded-mode watchdog, and replay recovery states.

### 8.6 `vel-9i2`
- Problem: file watcher and metadata persistence are bundled and tied to stale crate model.
- Required action: **rewrite and split**.
- Replacement rule: separate workflow version metadata persistence, watcher debounce, startup reload, and reaper GC.

### 8.7 `vel-2gi`
- Problem: single-writer registry bead predates explicit fencing and updated lifecycle model.
- Required action: **rewrite**.
- Replacement rule: keep single-active-instance registry, but align downstream dependencies to ADR-029 and ADR-039 instead of treating it as the full concurrency solution.

### 8.8 `vel-q8s`
- Problem: replay bead still carries older replay assumptions.
- Required action: **rewrite**.
- Replacement rule: align to ADR-016/027/035 with snapshot-aware replay, upcast-before-apply, and deterministic blocked/error handling.

### 8.9 `vel-1rz`
- Problem: too large; bundles decision loop, completion handling, retry logic, and idempotency behavior.
- Required action: **supersede and split**.
- Required split:
  1. next-step selection,
  2. step scheduling/start events,
  3. completion/failure handling,
  4. retry policy application,
  5. stale result/fence validation.

### 8.10 `vel-y7g`
- Problem: macro bead now also carries ADR-009 dispatch logic and may imply nonexistent crate/layout assumptions.
- Required action: **rewrite and split**.
- Replacement rule: separate macro crate existence from generated `--graph` / `--execute-node` dispatch behavior.

### 8.11 `vel-edo`
- Problem: in-progress, but still framed as SDK scaffold instead of freeze-set-aligned protocol coverage.
- Required action: **rewrite in place or supersede**.
- Replacement rule: split read/write helpers, single-write guard, graph emission helpers, and execute-node dispatch helpers; keep ownership in real `vo-sdk` crate.

### 8.12 `vel-50j`
- Problem: in-progress scaffold bead is legacy prerequisite glue, not freeze-set-aligned implementation planning.
- Required action: **rewrite in place**.
- Replacement rule: convert to a workspace-truth bead that validates current `vo-actor` boundaries and creates only the minimum missing actor module scaffolding needed by the actual crate.

---

## 9. Sequencing Contract For New Beads

`arch-spec-to-beads` shall emit beads in the following phase order and dependency direction.

### Phase 0: Type and state foundations
Primary ADRs:

- `039`
- `036`
- `035`
- `020`

Primary crate targets:

- `vo-types`
- `vo-core`
- `vo-storage`

### Phase 1: Canonical workflow definition
Primary ADRs:

- `031`
- `004`
- `009`
- `017`
- `022`
- `003` node-kind eligibility portions

Primary crate targets:

- `vo-types`
- `vo-sdk`
- `vo-ipc`
- `vo-api` only for publish validation surfaces if needed

### Phase 2: Storage and atomic control plane
Primary ADRs:

- `002`
- `016`
- `032`

Primary crate targets:

- `vo-storage`
- `vo-core`

### Phase 3: Execution boundary and pure-step runtime
Primary ADRs:

- `012`
- `014`
- `018`
- `011`
- `019`
- `023`
- `006`
- `015`

Primary crate targets:

- `vo-ipc`
- `vo-sdk`
- `vo-actor`
- `vo-worker` if execution ownership is placed there

### Phase 4: Exactly-once core
Primary ADRs:

- `027`
- `028`
- `029`
- `013`
- `016`
- `043` skeleton

Primary crate targets:

- `vo-core`
- `vo-storage`
- `vo-actor`
- `vo-api`

### Phase 5: Waiting, timers, signals
Primary ADRs:

- `005`
- `042`
- `033`
- `036`

Primary crate targets:

- `vo-actor`
- `vo-storage`
- `vo-api`

### Phase 6: Managed effects
Primary ADRs:

- `030`
- `041`
- `034`

Primary crate targets:

- `vo-core`
- `vo-storage`
- `vo-actor`
- `vo-worker` or other real runtime crate only if explicitly justified

### Phase 7: Privacy and blob publication
Primary ADRs:

- `040`
- `025`

Primary crate targets:

- `vo-storage`
- `vo-core`
- `vo-api`

### Phase 8: Long-lived workflow maturity
Primary ADRs:

- `035`
- `037`
- `038`

Primary crate targets:

- `vo-core`
- `vo-storage`
- `vo-actor`
- `vo-cli`

### Phase 9: UI, AI, operator surfaces
Primary ADRs:

- `007`
- `024`
- `008`
- `026`

Primary crate targets:

- `vo-api`
- `vo-cli`
- `vo-frontend`

### Phase 10: Freeze gate
Primary ADRs:

- `043`

Primary crate targets:

- cross-cutting verification beads only

No phase may be skipped by downstream dependencies.

---

## 10. Bead Generation Rules

The new corpus shall follow these generation rules.

### 10.1 Crate targeting rules

1. Prefer existing crates.
2. Do not reference `vo-engine` as an existing crate.
3. Do not reference `vo-ui` as an existing crate; current UI crate is `vo-frontend`.
4. Any proposal for a new crate must begin with an explicit workspace-membership bead and must not be assumed by other beads beforehand.

### 10.2 Documentation/path rules

1. Prefer `docs/adr/v2/*` and the top-level implementation documents already present.
2. Do not reference stale `docs/v2/*` paths unless the file actually exists.
3. Do not use `README.md` or `CLAUDE.md` legacy naming as authoritative for crate existence.

### 10.3 Quality rules

Each bead must avoid these banned patterns:

- “implement the engine”
- “wire everything together”
- “support exact-once”
- “add API support”
- “build workflow system”

Instead, each bead must name one implementation seam.

### 10.4 Dependency rules

1. No bead may depend on a superseded bead.
2. No bead may depend on a stale crate scaffold that does not exist in workspace truth.
3. No bead may cross from query/UI/operator surfaces back into missing core contracts.

---

## 11. Success Criteria

The remediation is complete only when all are true:

1. Every freeze-set ADR has explicit coverage or one intentional umbrella with atomic children.
2. ADRs `028, 029, 030, 034, 035, 036, 038, 040, 041, 043` each have concrete bead families.
3. ADRs `031, 032, 033, 042` have strengthened, split coverage rather than vague legacy beads.
4. All listed stale active beads have been rewritten, split, or superseded.
5. No newly emitted bead references nonexistent crates, files, or obsolete doc paths.
6. The dependency graph of new beads respects the implementation build order phases.
7. The corpus contains atomic implementation work, not umbrella-only coordination sludge.
8. The resulting bead set produces at least one valid ready path through Phase 0 → Phase 4.

---

## 12. Anti-Drift Constraints

The new corpus shall enforce the following permanent constraints:

1. **Workspace-first law**
   - `Cargo.toml` beats prose docs when crate existence differs.

2. **Freeze-set law**
   - No ADR in the freeze set may be treated as optional unless explicitly deferred by a new architectural decision.

3. **Exact-once truth law**
   - No bead may claim exact-once coverage while omitting admission dedupe, fencing, managed-effect semantics, or verification.

4. **Atomicity law**
   - No bead may bundle more than one independently testable behavior.

5. **Supersession law**
   - Stale beads stay closed/superseded; downstream work must not continue to depend on them.

6. **Real-surface law**
   - Query/UI/operator beads may only target surfaces backed by actual core contracts already planned below them.

---

## 13. Pre-Mortem

Three months from now, the most likely remediation failure is not missing ideas; it is **corpus relapse into stale names and vague beads**.

The likely failure modes are:

1. planners reintroduce `vo-engine` assumptions because legacy docs still say `vo-engine`,
2. exact-once work gets represented as broad meta beads rather than admission/fencing/effect/verification slices,
3. build-order discipline collapses and UI/API work gets planned ahead of the core,
4. active stale beads remain open and continue attracting implementation work.

Required detection signals:

1. count of beads referencing nonexistent crates/files,
2. count of freeze-set ADRs without explicit coverage,
3. count of beads spanning multiple ADR layers,
4. count of open beads marked superseded/stale but not closed,
5. count of ready beads per phase.

---

## 14. Output Expectation For `arch-spec-to-beads`

The next pipeline shall generate:

1. a corrected umbrella/child structure for the freeze set,
2. supersession records for stale active beads,
3. rewritten atomic beads targeted at real workspace crates,
4. dependency edges matching Phase 0 through Phase 10,
5. explicit success criteria on every bead.

The resulting corpus must be safe for autonomous implementation agents.

---

## 15. Next Step

Run:

```bash
opencode -a arch-spec-to-beads
```

This spec is intended to be the sole planning input for the remediation pass.
