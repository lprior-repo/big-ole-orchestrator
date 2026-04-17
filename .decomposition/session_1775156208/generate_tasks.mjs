const sessionSpec = ".forge/forge_1775156208/architecture-spec.md";

const adrDocs = {
  "009": "docs/adr/v2/ADR-009-v2-multi-task-binary.md",
  "027": "docs/adr/v2/ADR-027-v2-deterministic-event-sourced-replay.md",
  "028": "docs/adr/v2/ADR-028-v2-exactly-once-ingress-deduplication.md",
  "029": "docs/adr/v2/ADR-029-v2-execution-leases-and-fencing.md",
  "030": "docs/adr/v2/ADR-030-v2-managed-effects-and-sink-contracts.md",
  "031": "docs/adr/v2/ADR-031-v2-canonical-workflow-spec-sdk-ui.md",
  "032": "docs/adr/v2/ADR-032-v2-write-path-qos-and-hot-cold-storage.md",
  "033": "docs/adr/v2/ADR-033-v2-fairness-and-workload-classes.md",
  "034": "docs/adr/v2/ADR-034-v2-saga-compensation-and-reversibility.md",
  "035": "docs/adr/v2/ADR-035-v2-event-schema-evolution-and-upcasting.md",
  "036": "docs/adr/v2/ADR-036-v2-command-identity-correlation-and-causation.md",
  "037": "docs/adr/v2/ADR-037-v2-rebuildable-projections-and-self-healing.md",
  "038": "docs/adr/v2/ADR-038-v2-workflow-lineage-and-continue-as-new.md",
  "039": "docs/adr/v2/ADR-039-v2-hierarchical-lifecycle-state-machine.md",
  "040": "docs/adr/v2/ADR-040-v2-canonical-blob-durability-and-publication.md",
  "041": "docs/adr/v2/ADR-041-v2-managed-connector-runtime-contract.md",
  "042": "docs/adr/v2/ADR-042-v2-signal-matching-and-wake-up-semantics.md",
  "043": "docs/adr/v2/ADR-043-v2-exact-once-verification-strategy.md"
};

const phasePriority = (phase) => {
  if (phase <= 2) return 1;
  if (phase <= 5) return 2;
  return 3;
};

const implTask = ({ id, title, crate, adr, phase, effort, description, files, artifact, surface }) => {
  const adrDoc = adrDocs[adr];
  return {
    id,
    title,
    type: "feature",
    priority: phasePriority(phase),
    effort,
    description,
    clarifications: {
      resolved: [
        `Primary ADR anchor is ADR-${adr}.`,
        `Owning crate is ${crate} from Cargo.toml workspace truth.`
      ],
      open: [],
      assumptions: [
        "Downstream beads will integrate this seam after its phase prerequisites are planned.",
        "No legacy vo-engine crate or docs/v2 path may be referenced."
      ]
    },
    ears: {
      ubiquitous: [
        `THE SYSTEM SHALL implement ${artifact} inside ${crate} only.`,
        `THE SYSTEM SHALL keep this bead within ${surface}.`
      ],
      event_driven: [
        {
          trigger: `WHEN ADR-${adr} coverage for ${artifact} is missing`,
          shall: `THE SYSTEM SHALL add the isolated ${artifact} seam before downstream work depends on it.`
        }
      ],
      unwanted: [
        {
          condition: `IF the change reaches outside ${crate}`,
          shall_not: "THE SYSTEM SHALL NOT keep the bead bundled.",
          because: "Molecular slicing requires one crate-sized seam."
        }
      ]
    },
    contracts: {
      preconditions: [
        `${adrDoc} exists in the frozen ADR corpus.`,
        `${files[0]} exists in the current workspace.`,
        `Cargo.toml still lists crates/${crate} as a workspace member.`
      ],
      postconditions: [
        `${artifact} exists in ${crate}.`,
        `A focused test or validator proves the new ${artifact} seam.`,
        "No reference to vo-engine or stale docs/v2 paths was introduced."
      ],
      invariants: [
        `This bead stays inside ${surface}.`,
        `This bead does not skip ahead of phase ${phase} prerequisites.`,
        "The change remains independently revertable."
      ]
    },
    tests: {
      happy: [
        `A focused check accepts the expected ${artifact} behavior.`
      ],
      error: [
        `A focused check rejects the invalid ${artifact} case.`
      ],
      edge: [
        `A regression check covers the boundary input for ${artifact}.`
      ]
    },
    research: {
      files: [...files, adrDoc],
      patterns: [
        `Follow the smallest existing pattern in ${files[0]}.`
      ],
      questions: []
    },
    implementation: {
      phase_0: [
        `Inspect ${files[0]} plus ${adrDoc} for the isolated seam.`
      ],
      phase_1: [
        `Add a focused failing test or validator for ${artifact}.`
      ],
      phase_2: [
        `Implement ${artifact} in ${crate} without touching unrelated crates.`
      ]
    },
    context: {
      related_files: [...files, adrDoc, sessionSpec],
      similar: [adrDoc]
    }
  };
};

const staleTask = ({ id, bead, title, crate, adr, effort, description, files, artifact }) => {
  const adrDoc = adrDocs[adr];
  return {
    id,
    title,
    type: "task",
    priority: 1,
    effort,
    description,
    clarifications: {
      resolved: [
        `${bead} is stale per the remediation spec.`,
        `Replacement planning must target ${crate} or another real workspace crate.`
      ],
      open: [],
      assumptions: [
        "The replacement family will be generated in the same remediation pass.",
        "No downstream bead may retain a dependency on the stale identifier."
      ]
    },
    ears: {
      ubiquitous: [
        `THE SYSTEM SHALL treat ${bead} as invalid corpus state.`,
        `THE SYSTEM SHALL rewrite ${bead} against workspace-truth crate ownership.`
      ],
      event_driven: [
        {
          trigger: `WHEN ${bead} appears as an active dependency`,
          shall: `THE SYSTEM SHALL replace it with ${artifact}.`
        }
      ],
      unwanted: [
        {
          condition: `IF ${bead} still points at vo-engine or another nonexistent crate`,
          shall_not: "THE SYSTEM SHALL NOT keep it open.",
          because: "Stale beads would trap downstream planning."
        }
      ]
    },
    contracts: {
      preconditions: [
        `${sessionSpec} lists ${bead} as stale active work.`,
        `${adrDoc} exists as the replacement ADR anchor.`,
        `${files[0]} exists in the current workspace.`
      ],
      postconditions: [
        `${bead} is marked superseded or rewritten in the replacement corpus.`,
        `Replacement tasks reference real crates only.`,
        `No new dependency edge targets ${bead}.`
      ],
      invariants: [
        `The replacement keeps ownership in ${crate}.`,
        "The stale identifier stays closed once superseded.",
        "The rewrite remains atomic at corpus level."
      ]
    },
    tests: {
      happy: [
        `Corpus validation shows ${bead} replaced by the intended molecular family.`
      ],
      error: [
        `Corpus validation fails if a new bead still depends on ${bead}.`
      ],
      edge: [
        `Validation rejects any replacement text that still names vo-engine.`
      ]
    },
    research: {
      files: [...files, adrDoc],
      patterns: [
        "Follow the remediation spec replacement rule exactly."
      ],
      questions: []
    },
    implementation: {
      phase_0: [
        `Inspect ${sessionSpec} plus ${adrDoc} for the replacement rule.`
      ],
      phase_1: [
        `Add a failing corpus validation that still sees ${bead} as active.`
      ],
      phase_2: [
        `Record ${artifact} without creating any new stale dependency.`
      ]
    },
    context: {
      related_files: [...files, adrDoc, sessionSpec],
      similar: [sessionSpec]
    }
  };
};

const staleDefs = [
  {
    bead: "vel-3fs",
    title: "tracker: supersede vel-3fs with workspace-truth crate targeting",
    crate: "vo-core",
    adr: "031",
    effort: "30min",
    description: "Close vel-3fs as a stale prerequisite, point replacement work at existing crates only, block any dependency on a nonexistent vo-engine scaffold.",
    files: ["Cargo.toml", "crates/vo-core/src/lib.rs"],
    artifact: "a workspace-truth supersession mapping for vel-3fs"
  },
  {
    bead: "vel-7dg",
    title: "tracker: rewrite vel-7dg into atomic vo-api ingress slices",
    crate: "vo-api",
    adr: "028",
    effort: "30min",
    description: "Replace vel-7dg with separate vo-api beads for workflow admission, signal admission, query surfaces, exact-safe errors.",
    files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/handlers/signal.rs"],
    artifact: "a split replacement family for vel-7dg"
  },
  {
    bead: "vel-60k",
    title: "tracker: rewrite vel-60k against real projection query surfaces",
    crate: "vo-api",
    adr: "037",
    effort: "30min",
    description: "Retarget vel-60k away from stale endpoint framing, bind it to real projection or query seams in the current API crate.",
    files: ["crates/vo-api/src/lib.rs", "crates/vo-storage/src/query/mod.rs"],
    artifact: "a projection-safe rewrite for vel-60k"
  },
  {
    bead: "vel-sd1",
    title: "tracker: split vel-sd1 into qos workload slices",
    crate: "vo-core",
    adr: "032",
    effort: "30min",
    description: "Replace vel-sd1 with isolated beads for semaphore limits, workload budgets, degraded admission, rejection reasons.",
    files: ["crates/vo-core/src/lib.rs", "crates/vo-api/src/types/errors.rs"],
    artifact: "a qos replacement family for vel-sd1"
  },
  {
    bead: "vel-3hv",
    title: "tracker: split vel-3hv into recovery-state slices",
    crate: "vo-actor",
    adr: "029",
    effort: "30min",
    description: "Replace vel-3hv with isolated beads for recovery throttling, orphan detection, degraded watchdog handling, replay recovery states.",
    files: ["crates/vo-actor/src/reanimator.rs", "crates/vo-actor/src/timer_supervisor.rs"],
    artifact: "a recovery rewrite family for vel-3hv"
  },
  {
    bead: "vel-9i2",
    title: "tracker: rewrite vel-9i2 into metadata watcher slices",
    crate: "vo-storage",
    adr: "031",
    effort: "30min",
    description: "Replace vel-9i2 with separate beads for workflow version metadata, watcher debounce, startup reload, reaper gc.",
    files: ["crates/vo-storage/src/lib.rs", "crates/vo-sdk/src/write.rs"],
    artifact: "a metadata rewrite family for vel-9i2"
  },
  {
    bead: "vel-2gi",
    title: "tracker: rewrite vel-2gi for single-active fencing scope",
    crate: "vo-core",
    adr: "029",
    effort: "30min",
    description: "Keep vel-2gi focused on single-active registry scope, retarget downstream work to explicit fencing plus lifecycle beads.",
    files: ["crates/vo-core/src/lib.rs", "crates/vo-actor/src/lib.rs"],
    artifact: "a fencing-aware rewrite for vel-2gi"
  },
  {
    bead: "vel-q8s",
    title: "tracker: rewrite vel-q8s for upcast-first replay",
    crate: "vo-storage",
    adr: "035",
    effort: "30min",
    description: "Retarget vel-q8s to snapshot-aware replay, upcast-before-apply flow, deterministic blocked or error handling.",
    files: ["crates/vo-storage/src/snapshots/mod.rs", "crates/vo-storage/tests/integration_replay.rs"],
    artifact: "an upcast-safe rewrite for vel-q8s"
  },
  {
    bead: "vel-1rz",
    title: "tracker: supersede vel-1rz with runtime step slices",
    crate: "vo-actor",
    adr: "027",
    effort: "30min",
    description: "Close vel-1rz as over-bundled runtime work, replace it with isolated beads for selection, scheduling, completion, retry, stale-result checks.",
    files: ["crates/vo-actor/src/lib.rs", "crates/vo-core/src/lib.rs"],
    artifact: "a split runtime replacement family for vel-1rz"
  },
  {
    bead: "vel-y7g",
    title: "tracker: split vel-y7g into macro crate plus dispatch slices",
    crate: "vo-cli",
    adr: "009",
    effort: "30min",
    description: "Replace vel-y7g with one bead for macro crate reality plus separate dispatch beads for graph and execute-node behavior.",
    files: ["crates/vo-cli/src/dispatch_mod.rs", "crates/vo-cli/src/main.rs"],
    artifact: "a dispatch-safe replacement family for vel-y7g"
  },
  {
    bead: "vel-edo",
    title: "tracker: rewrite vel-edo into protocol-scoped vo-sdk slices",
    crate: "vo-sdk",
    adr: "031",
    effort: "30min",
    description: "Retarget vel-edo to separate vo-sdk beads for read helpers, write helpers, single-write guards, graph emission, execute-node dispatch helpers.",
    files: ["crates/vo-sdk/src/read.rs", "crates/vo-sdk/src/write.rs"],
    artifact: "a protocol-scoped rewrite family for vel-edo"
  },
  {
    bead: "vel-50j",
    title: "tracker: rewrite vel-50j to minimum vo-actor scaffolding",
    crate: "vo-actor",
    adr: "039",
    effort: "30min",
    description: "Convert vel-50j into a workspace-truth bead that validates current vo-actor boundaries, adds only minimal actor scaffolding if a gap remains.",
    files: ["crates/vo-actor/src/lib.rs", "Cargo.toml"],
    artifact: "a minimum-scaffold rewrite for vel-50j"
  }
];

const implDefs = [
  { title: "types: add schema version fields on durable records", crate: "vo-types", adr: "035", phase: 0, effort: "1hr", description: "Introduce version markers on durable record types so replay can branch by schema revision without guessing payload shape.", files: ["crates/vo-types/src/events.rs", "crates/vo-types/src/types.rs"], artifact: "schema version fields on durable records", surface: "one domain type family" },
  { title: "core: add upcaster registry interfaces", crate: "vo-core", adr: "035", phase: 0, effort: "1hr", description: "Define a small upcaster registry interface that resolves replay transforms by durable schema version inside vo-core.", files: ["crates/vo-core/src/lib.rs", "crates/vo-types/src/events.rs"], artifact: "upcaster registry interfaces", surface: "one connector/runtime contract" },
  { title: "core: normalize replay input through upcaster chain", crate: "vo-core", adr: "035", phase: 0, effort: "1hr", description: "Route replay input through a normalization seam that applies registered upcasters before state transition logic runs.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/tests/integration_replay.rs"], artifact: "a replay-time normalization path", surface: "one actor/runtime state transition path" },
  { title: "storage: discard incompatible snapshots by schema window", crate: "vo-storage", adr: "035", phase: 0, effort: "1hr", description: "Add snapshot compatibility checks that discard incompatible images instead of replaying from an unsafe snapshot boundary.", files: ["crates/vo-storage/src/snapshots/mod.rs", "crates/vo-storage/src/snapshots/tests.rs"], artifact: "snapshot compatibility or discard logic", surface: "one storage partition or codec path" },
  { title: "storage: add projection compatibility window rules", crate: "vo-storage", adr: "035", phase: 0, effort: "1hr", description: "Encode a projection compatibility window rule so stale projection payloads can be detected before query replay consumes them.", files: ["crates/vo-storage/src/query/mod.rs", "crates/vo-storage/src/query/tests.rs"], artifact: "projection compatibility window rules", surface: "one projection or query surface" },
  { title: "types: add CommandEnvelope type family", crate: "vo-types", adr: "036", phase: 0, effort: "1hr", description: "Introduce the canonical CommandEnvelope types for command identity, issuer data, correlation ids, causation ids.", files: ["crates/vo-types/src/types.rs", "crates/vo-types/src/lib.rs"], artifact: "the CommandEnvelope type family", surface: "one domain type family" },
  { title: "types: propagate command metadata into event types", crate: "vo-types", adr: "036", phase: 0, effort: "1hr", description: "Extend event type definitions so command metadata can flow into durable events without implicit side channels.", files: ["crates/vo-types/src/events.rs", "crates/vo-types/src/lib.rs"], artifact: "command metadata propagation into events", surface: "one domain type family" },
  { title: "types: add canonical WorkflowSpec schema family", crate: "vo-types", adr: "031", phase: 1, effort: "2hr", description: "Define the canonical WorkflowSpec schema family in vo-types so every publisher shares one durable workflow contract.", files: ["crates/vo-types/src/workflow/types.rs", "crates/vo-types/src/workflow/mod.rs"], artifact: "the canonical WorkflowSpec schema family", surface: "one domain type family" },
  { title: "api: validate exact eligibility during workflow publish", crate: "vo-api", adr: "031", phase: 1, effort: "1hr", description: "Reject workflow publish requests whose declared exact semantics lack the required canonical metadata.", files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/types/errors.rs"], artifact: "publish-time exact eligibility validation", surface: "one ingress or operator API surface" },
  { title: "types: add signal compensation capability metadata", crate: "vo-types", adr: "031", phase: 1, effort: "1hr", description: "Add canonical metadata fields for signal contracts, compensation policy hooks, runtime capability flags in WorkflowSpec.", files: ["crates/vo-types/src/workflow/types.rs", "crates/vo-types/src/workflow/mod.rs"], artifact: "signal compensation capability metadata", surface: "one domain type family" },
  { title: "sdk: emit canonical WorkflowSpec payloads", crate: "vo-sdk", adr: "031", phase: 1, effort: "1hr", description: "Emit canonical WorkflowSpec payloads from vo-sdk so downstream consumers can verify parity against the shared schema.", files: ["crates/vo-sdk/src/write.rs", "crates/vo-sdk/src/tests/write_success_tests.rs"], artifact: "the SDK WorkflowSpec emission path", surface: "one connector or runtime contract" },
  { title: "core: add write class taxonomy", crate: "vo-core", adr: "032", phase: 2, effort: "1hr", description: "Introduce explicit write classes for control-plane, projection, blob paths so storage pressure can be budgeted per class.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/lib.rs"], artifact: "the write class taxonomy", surface: "one actor or runtime state transition path" },
  { title: "core: budget queues per write class", crate: "vo-core", adr: "032", phase: 2, effort: "2hr", description: "Split queue budgeting by write class so control-plane writes stay isolated from projection or blob pressure.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/append.rs"], artifact: "separate queues and budgets per write class", surface: "one actor or runtime state transition path" },
  { title: "core: couple degraded admission to write pressure", crate: "vo-core", adr: "032", phase: 2, effort: "1hr", description: "Bind degraded-mode admission to current write pressure so new work can be rejected before control-plane durability is threatened.", files: ["crates/vo-core/src/lib.rs", "crates/vo-api/src/types/errors.rs"], artifact: "degraded-mode admission coupling", surface: "one actor or runtime state transition path" },
  { title: "core: expose writer pressure metrics", crate: "vo-core", adr: "032", phase: 2, effort: "1hr", description: "Add metrics that expose writer pressure plus blob pressure without changing unrelated admission behavior.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/lib.rs"], artifact: "writer pressure metrics", surface: "one verification harness slice" },
  { title: "types: add dedupe key value objects", crate: "vo-types", adr: "028", phase: 4, effort: "1hr", description: "Introduce exact-ingress dedupe key value objects with validation rules for command publication paths.", files: ["crates/vo-types/src/string_types.rs", "crates/vo-types/src/types.rs"], artifact: "command-side dedupe key types", surface: "one domain type family" },
  { title: "storage: add dedupe retention record model", crate: "vo-storage", adr: "028", phase: 4, effort: "1hr", description: "Define the dedupe partition key plus retention record model used to persist exact-ingress admission state.", files: ["crates/vo-storage/src/partitions.rs", "crates/vo-storage/src/codec.rs"], artifact: "the dedupe partition key plus retention record model", surface: "one storage partition or codec path" },
  { title: "api: add atomic workflow-start admission", crate: "vo-api", adr: "028", phase: 4, effort: "2hr", description: "Add the workflow-start admission seam that requires a valid dedupe key before the exact workflow start path can proceed.", files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/handlers/helpers.rs"], artifact: "the atomic workflow-start admission path", surface: "one ingress or operator API surface" },
  { title: "api: add atomic signal admission", crate: "vo-api", adr: "028", phase: 4, effort: "2hr", description: "Add the signal or approval admission seam that checks exact dedupe state before accepting an inbound mutation.", files: ["crates/vo-api/src/handlers/signal.rs", "crates/vo-api/src/handlers/helpers.rs"], artifact: "the atomic signal admission path", surface: "one ingress or operator API surface" },
  { title: "api: reject exact ingress without dedupe key", crate: "vo-api", adr: "028", phase: 4, effort: "1hr", description: "Reject any exact ingress request that omits a dedupe key, surface a precise error instead of implicit downgrade behavior.", files: ["crates/vo-api/src/types/errors.rs", "crates/vo-api/src/handlers/workflow.rs"], artifact: "the rejection path for exact ingress without a dedupe key", surface: "one ingress or operator API surface" },
  { title: "storage: expire dedupe retention records", crate: "vo-storage", adr: "028", phase: 4, effort: "1hr", description: "Add retention expiry handling for dedupe records plus the persisted visibility fields needed by operator query paths.", files: ["crates/vo-storage/src/purge.rs", "crates/vo-storage/src/query/mod.rs"], artifact: "dedupe retention expiry plus operator visibility", surface: "one storage partition or codec path" },
  { title: "types: add lease record plus fence token types", crate: "vo-types", adr: "029", phase: 4, effort: "1hr", description: "Introduce the lease record plus fence token types used to identify the current execution owner.", files: ["crates/vo-types/src/types.rs", "crates/vo-types/src/state.rs"], artifact: "lease record plus fence token types", surface: "one domain type family" },
  { title: "storage: add lease partition codecs", crate: "vo-storage", adr: "029", phase: 4, effort: "1hr", description: "Define the storage codec path for lease records so fence state can be loaded and persisted atomically.", files: ["crates/vo-storage/src/partitions.rs", "crates/vo-storage/src/codec.rs"], artifact: "the lease partition codec path", surface: "one storage partition or codec path" },
  { title: "actor: acquire fence before scheduling", crate: "vo-actor", adr: "029", phase: 4, effort: "2hr", description: "Require a valid fence acquisition step before the actor runtime schedules execution for a workflow instance.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "fence acquisition before scheduling", surface: "one actor or runtime state transition path" },
  { title: "core: reject stale completions on fenced writes", crate: "vo-core", adr: "029", phase: 4, effort: "1hr", description: "Reject completion writes whose fence token is older than the current lease state for the instance.", files: ["crates/vo-core/src/lib.rs", "crates/vo-actor/tests/adversarial_contract.rs"], artifact: "stale completion rejection on the write path", surface: "one actor or runtime state transition path" },
  { title: "actor: advance fence during retry recovery", crate: "vo-actor", adr: "029", phase: 4, effort: "1hr", description: "Advance the fence token during retry or recovery handoff so stale workers cannot win a resumed completion race.", files: ["crates/vo-actor/src/reanimator.rs", "crates/vo-actor/tests/adversarial_contract.rs"], artifact: "retry or recovery fence advancement", surface: "one actor or runtime state transition path" },
  { title: "core: define exact-once crash-point matrix", crate: "vo-core", adr: "043", phase: 4, effort: "1hr", description: "Define the exact-once crash-point matrix as a small verification harness input, scoped to the current phase coverage.", files: ["crates/vo-core/tests/red_queen_adversarial.rs", "crates/vo-core/src/lib.rs"], artifact: "the exact-once crash-point matrix", surface: "one verification harness slice" },
  { title: "storage: add replay property tests", crate: "vo-storage", adr: "043", phase: 4, effort: "1hr", description: "Add replay property tests that prove deterministic state reconstruction across persisted event streams.", files: ["crates/vo-storage/tests/integration_replay.rs", "crates/vo-storage/src/snapshots/tests_property.rs"], artifact: "replay property tests", surface: "one verification harness slice" },
  { title: "actor: add stale-winner fencing tests", crate: "vo-actor", adr: "043", phase: 4, effort: "1hr", description: "Add focused tests that prove a stale execution winner cannot commit after fence ownership changes.", files: ["crates/vo-actor/tests/adversarial_contract.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "fencing stale-winner tests", surface: "one verification harness slice" },
  { title: "worker: add connector ambiguity tests", crate: "vo-worker", adr: "043", phase: 4, effort: "1hr", description: "Add tests that prove ambiguous connector outcomes route into reconciliation instead of duplicate effect commits.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-core/tests/red_queen_adversarial.rs"], artifact: "connector ambiguity reconciliation tests", surface: "one verification harness slice" },
  { title: "actor: add lineage signal correctness tests", crate: "vo-actor", adr: "043", phase: 4, effort: "1hr", description: "Add correctness tests for lineage-aware signal delivery across waiting or rollover states.", files: ["crates/vo-actor/src/timer_supervisor_tests.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "lineage and signal correctness tests", surface: "one verification harness slice" },
  { title: "cli: add exact-once release gate check", crate: "vo-cli", adr: "043", phase: 10, effort: "1hr", description: "Add a release-gate command path that fails when the exact-once verification matrix is incomplete or red.", files: ["crates/vo-cli/src/commands/check.rs", "crates/vo-cli/src/main.rs"], artifact: "exact-once release-gate integration", surface: "one verification harness slice" },
  { title: "types: add workload class types", crate: "vo-types", adr: "033", phase: 5, effort: "1hr", description: "Introduce workload class types that distinguish reserved recovery work from normal admission classes.", files: ["crates/vo-types/src/types.rs", "crates/vo-types/src/lib.rs"], artifact: "the workload class type family", surface: "one domain type family" },
  { title: "actor: add reserved permit budgeting", crate: "vo-actor", adr: "033", phase: 5, effort: "1hr", description: "Implement reserved permit budgeting so recovery work keeps a bounded share of execution capacity.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "reserved permit budgeting", surface: "one actor or runtime state transition path" },
  { title: "actor: reserve permits for recovery path", crate: "vo-actor", adr: "033", phase: 5, effort: "1hr", description: "Add the recovery reservation path that consumes only the reserved permit class during backlog repair.", files: ["crates/vo-actor/src/reanimator.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "the recovery reservation path", surface: "one actor or runtime state transition path" },
  { title: "api: surface workload rejection reasons", crate: "vo-api", adr: "033", phase: 5, effort: "1hr", description: "Expose explicit workload rejection reasons at the API boundary when fairness budgets reject new work.", files: ["crates/vo-api/src/types/errors.rs", "crates/vo-api/src/handlers/workflow.rs"], artifact: "workload rejection reason surfacing", surface: "one ingress or operator API surface" },
  { title: "types: add lineage-aware signal addresses", crate: "vo-types", adr: "042", phase: 5, effort: "1hr", description: "Add the lineage-aware signal addressing model so signal routes can distinguish epoch-local from lineage-wide delivery.", files: ["crates/vo-types/src/workflow/types.rs", "crates/vo-types/src/types.rs"], artifact: "the lineage-aware signal addressing model", surface: "one domain type family" },
  { title: "actor: add wait-key matching rules", crate: "vo-actor", adr: "042", phase: 5, effort: "1hr", description: "Implement the wait-key matcher used to decide whether an inbound signal resumes a blocked workflow state.", files: ["crates/vo-actor/src/timer_supervisor.rs", "crates/vo-actor/src/timer_supervisor_tests.rs"], artifact: "the wait-key matching rules", surface: "one actor or runtime state transition path" },
  { title: "actor: add bounded signal buffering modes", crate: "vo-actor", adr: "042", phase: 5, effort: "2hr", description: "Implement Reject, BufferOne, BufferMany buffering modes with explicit bounds for unmatched signals.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "bounded signal buffering behaviors", surface: "one actor or runtime state transition path" },
  { title: "actor: add atomic accept-resume step", crate: "vo-actor", adr: "042", phase: 5, effort: "1hr", description: "Add the atomic accept-resume step that persists signal acceptance together with workflow wake-up intent.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "the atomic accept-resume path", surface: "one actor or runtime state transition path" },
  { title: "actor: add epoch-scope failure rules for signals", crate: "vo-actor", adr: "042", phase: 5, effort: "1hr", description: "Implement failure rules that distinguish epoch-scoped signal errors from lineage-scoped signal errors.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "epoch-scope or lineage-scope signal failure rules", surface: "one actor or runtime state transition path" },
  { title: "api: dedupe mutations by command_id", crate: "vo-api", adr: "036", phase: 5, effort: "1hr", description: "Use command_id to dedupe operator or API mutations before they fan into runtime admission.", files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/handlers/signal.rs"], artifact: "operator or API mutation dedupe by command_id", surface: "one ingress or operator API surface" },
  { title: "storage: expose issuer correlation causation query keys", crate: "vo-storage", adr: "036", phase: 5, effort: "1hr", description: "Expose query keys for issuer, correlation, causation metadata so higher surfaces can fetch command ancestry without hidden joins.", files: ["crates/vo-storage/src/query/mod.rs", "crates/vo-storage/src/query/tests.rs"], artifact: "issuer correlation causation query surfaces", surface: "one projection or query surface" },
  { title: "types: add managed effect intent states", crate: "vo-types", adr: "030", phase: 6, effort: "1hr", description: "Introduce EffectIntent, PreparedEffect, CommittedEffect domain states for managed effect tracking.", files: ["crates/vo-types/src/state.rs", "crates/vo-types/src/lib.rs"], artifact: "managed effect intent state types", surface: "one domain type family" },
  { title: "storage: add effect journal partition", crate: "vo-storage", adr: "030", phase: 6, effort: "1hr", description: "Add the effect journal partition and event integration seam used to persist managed effect transitions.", files: ["crates/vo-storage/src/partitions.rs", "crates/vo-storage/src/append.rs"], artifact: "the effect journal partition with event integration", surface: "one storage partition or codec path" },
  { title: "api: reject unsupported exact sinks during publish", crate: "vo-api", adr: "030", phase: 6, effort: "1hr", description: "Reject exact workflow publish requests whose sink declarations cannot satisfy the managed-effect contract.", files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/types/errors.rs"], artifact: "publish-time rejection for unsupported exact sinks", surface: "one ingress or operator API surface" },
  { title: "worker: isolate managed-effect execution path", crate: "vo-worker", adr: "030", phase: 6, effort: "2hr", description: "Separate managed-effect execution from unsafe activity execution so committed effects follow the journaled contract only.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-core/src/lib.rs"], artifact: "the managed-effect execution path", surface: "one connector or runtime contract" },
  { title: "worker: add connector runtime traits", crate: "vo-worker", adr: "041", phase: 6, effort: "1hr", description: "Define the connector runtime traits that managed connectors must implement inside the real worker crate.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-core/src/lib.rs"], artifact: "connector runtime trait interfaces", surface: "one connector or runtime contract" },
  { title: "worker: add prepare-commit-reconcile state machine", crate: "vo-worker", adr: "041", phase: 6, effort: "2hr", description: "Implement the smallest prepare, commit, reconcile state machine for managed connector execution.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-core/src/lib.rs"], artifact: "the prepare-commit-reconcile state machine", surface: "one actor or runtime state transition path" },
  { title: "worker: add ambiguity timeout states", crate: "vo-worker", adr: "041", phase: 6, effort: "1hr", description: "Add explicit ambiguity plus timeout states so connector uncertainty does not collapse into silent success.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-core/src/lib.rs"], artifact: "ambiguity or timeout states", surface: "one actor or runtime state transition path" },
  { title: "storage: persist managed connector receipts", crate: "vo-storage", adr: "041", phase: 6, effort: "1hr", description: "Persist connector receipts with the minimum fields needed for reconciliation after a crash or timeout boundary.", files: ["crates/vo-storage/src/append.rs", "crates/vo-storage/src/codec.rs"], artifact: "connector receipt persistence requirements", surface: "one storage partition or codec path" },
  { title: "worker: limit initial connector set to strong connectors", crate: "vo-worker", adr: "041", phase: 6, effort: "1hr", description: "Gate initial managed connector support to strong connector implementations only, reject weaker adapters from this phase.", files: ["crates/vo-worker/src/lib.rs", "crates/vo-api/src/types/errors.rs"], artifact: "the first strong connector implementation gate", surface: "one connector or runtime contract" },
  { title: "types: add workflow compensation policy field", crate: "vo-types", adr: "034", phase: 6, effort: "1hr", description: "Add compensation policy fields to the canonical workflow model without widening unrelated workflow metadata.", files: ["crates/vo-types/src/workflow/types.rs", "crates/vo-types/src/workflow/mod.rs"], artifact: "the workflow compensation policy field", surface: "one domain type family" },
  { title: "actor: add compensation lifecycle states", crate: "vo-actor", adr: "034", phase: 6, effort: "1hr", description: "Introduce lifecycle states for pending, scheduled, running, completed compensation execution.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "compensation lifecycle states", surface: "one actor or runtime state transition path" },
  { title: "core: link forward effects to compensation effects", crate: "vo-core", adr: "034", phase: 6, effort: "1hr", description: "Add a single linkage seam from a committed forward effect to its registered compensation effect reference.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/append.rs"], artifact: "forward-effect to compensation-effect linkage", surface: "one actor or runtime state transition path" },
  { title: "api: add manual compensation command path", crate: "vo-api", adr: "034", phase: 6, effort: "1hr", description: "Add the operator command path that requests compensation for a workflow instance with explicit intent.", files: ["crates/vo-api/src/handlers/workflow.rs", "crates/vo-api/src/types/errors.rs"], artifact: "the manual compensation command path", surface: "one ingress or operator API surface" },
  { title: "actor: reverse dependency order for compensation", crate: "vo-actor", adr: "034", phase: 6, effort: "1hr", description: "Reverse dependency ordering for compensation execution so downstream effects unwind before upstream effects.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "reverse dependency ordering for compensation", surface: "one actor or runtime state transition path" },
  { title: "storage: add canonical blob store records", crate: "vo-storage", adr: "040", phase: 7, effort: "1hr", description: "Introduce the canonical blob record plus store abstraction used by durable output publication.", files: ["crates/vo-storage/src/lib.rs", "crates/vo-storage/src/codec.rs"], artifact: "the canonical blob record store abstraction", surface: "one storage partition or codec path" },
  { title: "core: add publication barrier before output_ref", crate: "vo-core", adr: "040", phase: 7, effort: "1hr", description: "Add the publication barrier that blocks output_ref exposure until blob durability succeeds.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/lib.rs"], artifact: "the publication barrier before output_ref", surface: "one actor or runtime state transition path" },
  { title: "types: classify inline output versus blob output", crate: "vo-types", adr: "040", phase: 7, effort: "1hr", description: "Classify routing-critical inline output versus blob-backed output in the shared type contract.", files: ["crates/vo-types/src/types.rs", "crates/vo-types/src/events.rs"], artifact: "inline-output versus blob-output rules", surface: "one domain type family" },
  { title: "core: add optional-output blob failure rules", crate: "vo-core", adr: "040", phase: 7, effort: "1hr", description: "Add failure rules for optional output publication so blob errors stay explicit without corrupting durable workflow state.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/lib.rs"], artifact: "blob failure semantics for optional output", surface: "one actor or runtime state transition path" },
  { title: "storage: add blob retention gc path", crate: "vo-storage", adr: "040", phase: 7, effort: "1hr", description: "Add the blob retention gc path that reclaims expired blobs without deleting routing-critical inline data.", files: ["crates/vo-storage/src/purge.rs", "crates/vo-storage/tests/purge_integration.rs"], artifact: "the blob retention gc path", surface: "one storage partition or codec path" },
  { title: "types: add lineage plus epoch identifiers", crate: "vo-types", adr: "038", phase: 8, effort: "1hr", description: "Introduce lineage id plus epoch id types so continued-as-new execution can carry stable ancestry.", files: ["crates/vo-types/src/types.rs", "crates/vo-types/src/state.rs"], artifact: "lineage plus epoch identifiers", surface: "one domain type family" },
  { title: "core: add continued-as-new rollover path", crate: "vo-core", adr: "038", phase: 8, effort: "2hr", description: "Add the continued-as-new event plus atomic rollover seam that opens a new epoch without losing lineage identity.", files: ["crates/vo-core/src/lib.rs", "crates/vo-storage/src/append.rs"], artifact: "the continued-as-new rollover path", surface: "one actor or runtime state transition path" },
  { title: "storage: add lineage-aware query routing", crate: "vo-storage", adr: "038", phase: 8, effort: "1hr", description: "Add lineage-aware query routing so lookup paths can resolve the active epoch from stable lineage identity.", files: ["crates/vo-storage/src/query/mod.rs", "crates/vo-storage/src/query/tests.rs"], artifact: "lineage-aware query routing", surface: "one projection or query surface" },
  { title: "actor: update dedupe signal routing across rollover", crate: "vo-actor", adr: "038", phase: 8, effort: "1hr", description: "Update dedupe plus signal routing after rollover so inbound work reaches the active epoch only.", files: ["crates/vo-actor/src/lib.rs", "crates/vo-actor/tests/integration_tests.rs"], artifact: "dedupe or signal routing across rollover", surface: "one actor or runtime state transition path" }
];

const tasks = [];
let nextId = 1;

for (const def of staleDefs) {
  tasks.push(staleTask({ id: `task-${String(nextId).padStart(3, "0")}`, ...def }));
  nextId += 1;
}

for (const def of implDefs) {
  tasks.push(implTask({ id: `task-${String(nextId).padStart(3, "0")}`, ...def }));
  nextId += 1;
}

process.stdout.write(JSON.stringify(tasks, null, 2));
