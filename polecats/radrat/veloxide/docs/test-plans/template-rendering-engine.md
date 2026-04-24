# Test Plan: Template Rendering Engine

## Summary
- Behaviors identified: 58
- Trophy allocation: 38 unit / 12 integration / 4 proptest (Total 54 tests)
- Proptest invariants: 6
- Target Mutation Kill Rate: ≥90%
- Contract: `docs/contracts/template-rendering-engine.md`
- Parent bead: ve-yj4s

## Source Files Under Test

| File | Types / Functions |
|------|-------------------|
| `crates/vo-frontend/src/ui/domain_types.rs` | `NodeTemplateId`, `HttpMethod`, `HandleKind` |
| `crates/vo-frontend/src/ui/prototype_palette.rs` | `SketchNode`, `PaletteEntry`, `generate_skeleton` |
| `crates/vo-frontend/src/ui/command_palette.rs` | `CommandTemplate`, `filtered_templates`, `is_escape_key` |

## 1. Behavior Inventory

### 1.1 NodeTemplateId Core (domain_types.rs)

1. `NodeTemplateId::all()` returns exactly 14 variants in a fixed order.
2. `NodeTemplateId::as_str()` returns a unique, lowercase, hyphenated string for each variant.
3. `NodeTemplateId::as_str()` values are all distinct (no collisions).
4. `NodeTemplateId::from_str()` returns `Some(variant)` for every `as_str()` value.
5. `NodeTemplateId::from_str()` returns `None` for unknown strings.
6. `NodeTemplateId::from_str()` is case-sensitive (uppercase input returns `None`).
7. `NodeTemplateId::label()` returns a non-empty human-readable string for every variant.
8. `NodeTemplateId::hint()` returns a non-empty one-line description for every variant.
9. `NodeTemplateId::Display` impl outputs the same string as `as_str()`.
10. `NodeTemplateId::FromStr` impl returns `Err` for unknown strings.
11. `NodeTemplateId::FromStr` impl returns `Ok(variant)` for every valid `as_str()` value.

### 1.2 NodeTemplateId Variants Exhaustive

12. All 14 `as_str()` values match the canonical list: `http-handler`, `kafka-handler`, `cron-trigger`, `workflow-submit`, `run`, `service-call`, `object-call`, `send-message`, `get-state`, `set-state`, `condition`, `parallel`, `timer`, `timeout`.
13. All 14 `label()` values are non-empty and do not contain leading/trailing whitespace.
14. All 14 `hint()` values are non-empty and do not contain leading/trailing whitespace.
15. `NodeTemplateId::all()` contains each variant exactly once (no duplicates).

### 1.3 SketchNode (prototype_palette.rs)

16. `SketchNode` with default label stores the `node_type.label()` value.
17. `SketchNode` with custom label overrides the default.
18. `SketchNode::PartialEq` compares both `node_type` and `label`.
19. `SketchNode::Clone` produces an independent copy.

### 1.4 generate_skeleton (prototype_palette.rs)

20. Empty input produces header-only YAML with `name:` and `steps:` and no step entries.
21. Single node produces YAML with `step-1` and no `depends_on`.
22. Two nodes produce YAML where `step-2` has `depends_on: [step-1]`.
23. Three+ nodes produce a linear chain where each step depends on the previous.
24. Each step entry contains `type: {node_type.as_str()}`.
25. Each step entry contains `config: {}`.
26. Step IDs are sequential: `step-1`, `step-2`, ..., `step-N`.
27. First step never has `depends_on`.
28. Every step after the first has exactly one `depends_on` referencing the prior step.
29. Output starts with `name: "prototype-workflow"`.
30. Output is valid YAML (parseable by a YAML parser).

### 1.5 PaletteEntry (prototype_palette.rs)

31. `PALETTE_ENTRIES` contains entries for rendering in the prototype palette UI.
32. Each `PaletteEntry` has a non-empty `icon` string.
33. Each `PaletteEntry` references a valid `NodeTemplateId` variant.

### 1.6 filtered_templates (command_palette.rs)

34. Empty query returns all 14 templates.
35. Query matching on `as_str` (e.g., `"http-handler"`) returns `HttpHandler`.
36. Query matching on `label` (e.g., `"HTTP"`) returns `HttpHandler`.
37. Query matching on `hint` (e.g., `"durably"`) returns `Run`.
38. Matching is case-insensitive: `"kafka-handler"` and `"KAFKA-HANDLER"` return same results.
39. Query with leading/trailing whitespace is trimmed before matching.
40. Non-matching query returns empty vec.
41. Partial match within a word matches (e.g., `"handler"` matches both `HttpHandler` and `KafkaHandler`).
42. Multi-word match semantics: query `"http grpc"` matches templates whose label/hint/id contains both terms.
43. Returns `Vec<CommandTemplate>` with correct `node_type` for each match.

### 1.7 is_escape_key (command_palette.rs)

44. `"escape"` returns `true`.
45. `"Escape"` returns `true`.
46. `"ESCAPE"` returns `true`.
47. `"esc"` returns `true`.
48. `"Esc"` returns `true`.
49. `"ESC"` returns `true`.
50. `"Enter"` returns `false`.
51. `""` returns `false`.
52. `"a"` returns `false`.

### 1.8 CommandTemplate (command_palette.rs)

53. `CommandTemplate::PartialEq` compares `node_type` field.
54. `CommandTemplate::Copy` produces an identical value.

### 1.9 HttpMethod (domain_types.rs)

55. `HttpMethod::from_str_ignore_case("get")` returns `Get`.
56. `HttpMethod::from_str_ignore_case("POST")` returns `Post`.
57. `HttpMethod::from_str_ignore_case("invalid")` returns `Post` (default).
58. `HttpMethod::as_str()` returns uppercase method names.

## 2. Trophy Allocation

### Unit Tests (38)
- **NodeTemplateId identity** (INV-001 through INV-004): 11 tests covering `all()`, `as_str()` uniqueness, `from_str()` roundtrip, `label()`/`hint()` non-empty, `Display`, `FromStr`.
- **SketchNode**: 4 tests for construction, custom label, equality, clone.
- **generate_skeleton**: 11 tests for empty, single, multi-node, depends_on correctness, sequential IDs, YAML validity, header format.
- **PaletteEntry**: 3 tests for valid references, non-empty icons, distinct entries.
- **filtered_templates** (INV-009, INV-010): 10 tests for empty query, label/hint/id matching, case insensitivity, whitespace trimming, no-match, partial match.
- **is_escape_key**: 9 tests for all escape variants and non-matches.
- **CommandTemplate**: 2 tests for equality and copy.

### Integration Tests (12)
- **Skeleton YAML parsing**: 3 tests — parse the generated YAML string with `serde_yaml` and verify structure for empty, single-node, multi-node cases.
- **Filtered templates completeness**: 2 tests — verify every `NodeTemplateId` variant is reachable via at least one query term.
- **generate_skeleton → filtered_templates pipeline**: 1 test — generate skeleton from a sketch, extract node types, verify all are findable via filtered_templates.
- **from_str → Display roundtrip**: 2 tests — verify `format!("{}", id) == id.as_str()` and `id.from_str(id.as_str()) == Some(id)` for all variants.
- **NodeTemplateId::all() ordering stability**: 1 test — verify `all()` returns the same order across calls.
- **SketchNode label propagation**: 2 tests — verify UI click path (node_type → label() → SketchNode.label) produces correct label strings.
- **Palette coverage audit**: 1 test — verify all 14 NodeTemplateId variants appear in at least one of PALETTE_ENTRIES or are reachable via filtered_templates.

### Proptest (4 strategies, 6 invariants)
1. **INV-002 (as_str uniqueness)**: Generate random `NodeTemplateId` pairs, assert `as_str()` differs.
2. **INV-003 (from_str/as_str inverse)**: For all `NodeTemplateId` variants, assert `from_str(as_str())` is `Some(self)`.
3. **INV-009 (case-insensitive filter)**: Generate random mixed-case queries, assert results match lowercase equivalent.
4. **INV-010 (empty query returns all)**: Assert `filtered_templates("")` always has length 14 regardless of prior state.
5. **generate_skeleton purity**: Generate random `Vec<SketchNode>`, assert calling twice produces identical output.
6. **Skeleton step count invariant**: Generate random non-empty `Vec<SketchNode>`, assert output contains exactly N step entries.

## 3. BDD Scenarios

### INV-001: NodeTemplateId::all() returns exactly 14

```gherkin
Given: NodeTemplateId::all() is called
Then: The returned array has length 14
  And: No variant appears more than once
  And: Every variant from the enum definition is present
```

### INV-002: as_str values are unique

```gherkin
Given: All 14 NodeTemplateId variants
When: as_str() is called on each
Then: All 14 returned strings are distinct
  And: Each string matches the pattern /^[a-z][a-z-]*[a-z]$/
```

### INV-003: from_str inverts as_str

```gherkin
Given: A NodeTemplateId variant `v`
When: NodeTemplateId::from_str(v.as_str()) is called
Then: Returns Some(v)
  And: NodeTemplateId::from_str(v.as_str().to_uppercase()) returns None
```

### INV-004: Labels and hints are non-empty

```gherkin
Given: Any NodeTemplateId variant
When: label() or hint() is called
Then: The returned string is non-empty
  And: The string does not consist solely of whitespace
```

### INV-005: SketchNode label defaults to node_type.label()

```gherkin
Given: A NodeTemplateId::HttpHandler
When: SketchNode { node_type: HttpHandler, label: HttpHandler.label().to_string() } is created
Then: node.label == "HTTP Handler"
```

### INV-006: generate_skeleton produces sequential step IDs

```gherkin
Given: A Vec<SketchNode> with 3 elements
When: generate_skeleton is called
Then: Output contains "id: step-1", "id: step-2", "id: step-3"
  And: Output does not contain "id: step-0" or "id: step-4"
```

### INV-007: depends_on only after first node

```gherkin
Given: A Vec<SketchNode> with N > 1 elements
When: generate_skeleton is called
Then: The step-1 block does not contain "depends_on"
  And: Every step-N block (N > 1) contains "depends_on: [step-{N-1}]"
```

### INV-008: Palette entries render without panics

```gherkin
Given: Every entry in PALETTE_ENTRIES
When: entry.node_type.label(), entry.icon are accessed
Then: No panic occurs
  And: label() returns a non-empty string
  And: icon is a non-empty string
```

### INV-009: filtered_templates is case-insensitive

```gherkin
Given: The query "HTTP HANDLER"
When: filtered_templates is called
Then: Results include CommandTemplate { node_type: HttpHandler }
  And: Results are identical to query "http handler"
```

### INV-010: Empty query returns all templates

```gherkin
Given: The query ""
When: filtered_templates is called
Then: Returns exactly 14 CommandTemplate entries
  And: Each NodeTemplateId variant appears exactly once
```

### Skeleton: Empty sketch produces header-only YAML

```gherkin
Given: An empty Vec<SketchNode>
When: generate_skeleton is called
Then: Output contains "name:" and "steps:"
  And: Output does not contain "step-1"
  And: Output is valid YAML
```

### Skeleton: Multi-node produces linear dependency chain

```gherkin
Given: Vec of [HttpHandler, Run, Condition, SetState]
When: generate_skeleton is called
Then: step-1 has type "http-handler" and no depends_on
  And: step-2 has type "run" and depends_on: [step-1]
  And: step-3 has type "condition" and depends_on: [step-2]
  And: step-4 has type "set-state" and depends_on: [step-3]
```

### Filter: Non-matching query returns empty

```gherkin
Given: The query "zz-no-match-zz"
When: filtered_templates is called
Then: Returns an empty Vec
```

### Filter: Partial word match

```gherkin
Given: The query "handler"
When: filtered_templates is called
Then: Results include HttpHandler
  And: Results include KafkaHandler
  And: Results do not include Timer
```

### Escape key: Case-insensitive variants

```gherkin
Given: The key strings ["escape", "Escape", "ESCAPE", "esc", "Esc", "ESC"]
When: is_escape_key is called on each
Then: All return true
```

### Escape key: Non-escape keys

```gherkin
Given: The key strings ["Enter", "Tab", "a", ""]
When: is_escape_key is called on each
Then: All return false
```

## 4. Test File Layout

```
crates/vo-frontend/src/ui/
├── domain_types.rs          # Add tests to existing #[cfg(test)] mod
├── prototype_palette.rs     # Add tests to existing #[cfg(test)] mod
├── command_palette.rs       # Add tests to existing #[cfg(test)] mod
└── template_rendering_tests.rs  # NEW: Integration tests across all 3 modules
```

Integration tests in `template_rendering_tests.rs` will test cross-module invariants:
- `generate_skeleton` output parsed by YAML and validated against `NodeTemplateId` metadata
- `filtered_templates` results cover all variants reachable from `NodeTemplateId::all()`
- Roundtrip: `as_str` → `from_str` → `as_str` is identity for all variants
- Pipeline: `SketchNode` from palette → `generate_skeleton` → YAML contains correct types

## 5. Existing Test Coverage Gap Analysis

### Currently Covered (17 tests)
- `domain_types.rs`: 5 tests (HTTP method parsing, handle kind, all() count, label)
- `prototype_palette.rs`: 3 tests (empty skeleton, 2-node skeleton, 3-node skeleton)
- `command_palette.rs`: 6 tests (empty query, case-insensitive, no-match, whitespace trim, escape key variants, non-escape keys)

### Gaps to Fill (37 new tests)
- **NodeTemplateId**: `as_str()` uniqueness, `from_str()` roundtrip, `hint()` non-empty, `Display` impl, `FromStr` impl, all variant exhaustiveness, ordering stability
- **SketchNode**: Construction, custom label, equality, clone
- **generate_skeleton**: YAML validity, `config: {}` presence, header format verification, `type:` field correctness, purity (idempotent)
- **PaletteEntry**: All entries reference valid types, non-empty icons, distinct entries
- **filtered_templates**: Partial word match, multi-word query, every variant reachable
- **CommandTemplate**: Equality, Copy
- **Integration**: Cross-module pipeline tests, YAML parse verification

## 6. Risk Areas

| Area | Risk | Mitigation |
|------|------|------------|
| `NodeTemplateId::Sleep` reference | `prototype_palette.rs` line 34 references `NodeTemplateId::Sleep` which doesn't exist — this will fail to compile | This is a bug discovered during analysis; file as a separate bead |
| PALETTE_ENTRIES incomplete | Only 9 of 14 variants have palette entries | Verify via test; coverage gap is intentional (UI subset) |
| Duplicate PALETTE_ENTRIES | `Condition` appears twice in PALETTE_ENTRIES | Verify via test; flag as bug |
| Contract types not implemented | `TemplateDescriptor`, `TemplateCategory`, `TemplateError`, `ValidationViolation`, `RenderContext`, `SerializationReason` not yet in code | Test plan covers only implemented types; future beads handle remaining contract types |
| YAML validity | `generate_skeleton` builds strings manually, not via serde | Integration test should parse output to catch formatting regressions |
