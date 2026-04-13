# Test Plan: vo-frontend

## Summary

- **Bead**: ve-wy5j — TDD Red: vo-frontend has 0 tests
- **Current State**: 64 passing tests in command_palette, domain_types, prototype_palette, template_rendering_tests
- **Problem**: Majority of UI modules cannot be tested due to architectural issues
- **Behaviors Identified**: ~45 testable behaviors across UI rendering, state management, API integration

---

## 1. Current Test Coverage

### ✅ Working Tests (64 total)

| Module | Tests | Status |
|--------|-------|--------|
| command_palette | 7 | Fully testable |
| domain_types | 9 | Fully testable |
| prototype_palette | 3 | Fully testable |
| template_rendering_tests | 45 | Fully testable |

### ❌ Broken / Untestable

| Module | Issue |
|--------|-------|
| edges | Imports from non-existent `editor_interactions` module |
| selected_node_panel | Tests import from `oya_frontend` (not a crate) |
| execution_history_panel | Imports from `oya_frontend` |
| execution_plan_panel | Imports from `oya_frontend` |
| inspector_panel | Imports from `oya_frontend` |
| payload_preview_panel | Imports from `oya_frontend` |
| parallel_group_overlay | Imports from `oya_frontend` |
| validation_panel | Imports from `oya_frontend` |

---

## 2. Architectural Issues

### Issue 1: Missing `oya_frontend` Dependency

All UI modules import from `oya_frontend::graph` but this crate doesn't exist in:
- Workspace dependencies
- crates.io
- Any path dependency

**Affected Files**: ~25 files across vo-frontend

### Issue 2: Non-existent `editor_interactions` Module

`edges/layout.rs` imports from `crate::ui::editor_interactions` which doesn't exist.

### Issue 3: Module Visibility

`ui/mod.rs` only exports 4 modules publicly. Many internal modules exist but are inaccessible for testing.

---

## 3. Behaviors to Test (TDD Red Phase)

### UI Rendering

| # | Behavior | Component | Test Status |
|---|----------|-----------|-------------|
| UR-01 | Command palette filters templates by query | command_palette | ✅ Implemented |
| UR-02 | Escape key handling in command palette | command_palette | ✅ Implemented |
| UR-03 | Template skeleton generation | prototype_palette | ✅ Implemented |
| UR-04 | Category filtering for templates | template_rendering_tests | ✅ Implemented |
| UR-05 | Error display in template rendering | template_rendering_tests | ✅ Implemented |
| UR-06 | Edge layout calculation (bend delta) | edges | ❌ Blocked - normalize_bend_delta missing |
| UR-07 | Edge anchor point calculation | edges | ❌ Blocked - module broken |
| UR-08 | Parallel offset computation | edges | ❌ Blocked - module broken |

### State Management

| # | Behavior | Component | Test Status |
|---|----------|-----------|-------------|
| SM-01 | NodeTemplateId parsing and validation | domain_types | ✅ Implemented |
| SM-02 | HttpMethod parsing | domain_types | ✅ Implemented |
| SM-03 | HandleKind conversion | domain_types | ✅ Implemented |
| SM-04 | Sketch node creation from template | prototype_palette | ✅ Implemented |
| SM-05 | Timeline push with cap | selected_node_panel | ❌ Blocked - oya_frontend |
| SM-06 | Snapshot metadata management | selected_node_panel | ❌ Blocked - oya_frontend |
| SM-07 | Extension preview collection | selected_node_panel | ❌ Blocked - oya_frontend |

### API Integration

| # | Behavior | Component | Test Status |
|---|----------|-----------|-------------|
| AI-01 | Workflow serialization (JSON/YAML) | template_rendering_tests | ✅ Implemented |
| AI-02 | Validation error display | template_rendering_tests | ✅ Implemented |
| AI-03 | Circular dependency detection | template_rendering_tests | ✅ Implemented |
| AI-04 | Node execution state transitions | execution_history_panel | ❌ Blocked - oya_frontend |
| AI-05 | Execution plan display | execution_plan_panel | ❌ Blocked - oya_frontend |
| AI-06 | Payload preview generation | payload_preview_panel | ❌ Blocked - oya_frontend |

---

## 4. TDD Red: Implemented Failing Tests

### For edges module (Blocked by normalize_bend_delta)

```rust
#[test]
fn given_zoom_level_when_normalizing_bend_delta_then_result_is_scaled() {
    // RED: normalize_bend_delta doesn't exist yet
    let result = normalize_bend_delta(100.0, 2.0);
    assert_eq!(result, 50.0);
}

#[test]
fn given_zoom_of_one_when_normalizing_bend_delta_then_same_delta() {
    let result = normalize_bend_delta(75.0, 1.0);
    assert_eq!(result, 75.0);
}

#[test]
fn given_invalid_zoom_zero_when_normalizing_bend_delta_then_zero() {
    let result = normalize_bend_delta(100.0, 0.0);
    assert_eq!(result, 0.0);
}
```

### For selected_node_panel module (Blocked by oya_frontend)

```rust
#[test]
fn timeline_keeps_latest_items_with_cap() {
    let mut timeline: Vec<ExtensionTimelineEvent> = Vec::new();
    for idx in 0..14 {
        timeline = push_timeline(timeline, ExtensionTimelineEventKind::Applied, format!("entry-{idx}"), None);
    }
    assert_eq!(timeline.len(), 12); // Cap is 12
}
```

---

## 5. Trophy Allocation

| Layer | Current | Target | Delta |
|-------|---------|--------|-------|
| Unit / Calc | 64 | 80 | +16 |
| Integration | 0 | 20 | +20 |
| E2E | 0 | 5 | +5 |
| Static | 0 | 1 | +1 |

---

## 6. Recommendations

1. **Fix oya_frontend dependency**: Either add the missing crate or refactor code to use existing types
2. **Create editor_interactions module**: Or remove references from edges/layout.rs
3. **Export testable modules**: Add `pub(crate) mod edges;` to ui/mod.rs once dependencies are fixed
4. **Add integration tests**: Test UI component interactions with mock workflow state
5. **Add snapshot tests**: Use insta for snapshot testing of rendered components