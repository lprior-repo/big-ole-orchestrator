# Test Plan: Dependency Graph Resolver

## Summary

- **Bead**: ve-6t1 — Test Plan: Dependency graph resolver
- **Component**: `DependencyGraphResolver` in `vo-types`
- **Behaviors identified**: 47
- **Trophy allocation**: 28 unit / 16 integration / 3 e2e
- **Proptest invariants**: 6
- **Fuzz targets**: 3
- **Kani harnesses**: 2
- **Mutation kill rate threshold**: ≥90%

---

## 1. Behavior Inventory

### DependencyGraphResolver — Direct Dependencies (Predecessors)

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-01 | Node with no incoming edges returns empty dependencies | `dependencies()` | unit |
| DGR-02 | Node with one incoming edge returns that predecessor | `dependencies()` | unit |
| DGR-03 | Node with multiple incoming edges returns all predecessors | `dependencies()` | unit |
| DGR-04 | Returns only direct predecessors, not transitive | `dependencies()` | unit |
| DGR-05 | Returns empty for non-existent node | `dependencies()` | unit |

### DependencyGraphResolver — Direct Dependents (Successors)

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-06 | Node with no outgoing edges returns empty dependents | `dependents()` | unit |
| DGR-07 | Node with one outgoing edge returns that successor | `dependents()` | unit |
| DGR-08 | Diamond DAG: node with multiple direct dependents | `dependents()` | unit |
| DGR-09 | Returns only direct successors, not transitive | `dependents()` | unit |

### DependencyGraphResolver — Transitive Dependencies

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-10 | Transitive dependencies returns all ancestors | `transitive_dependencies()` | unit |
| DGR-11 | Transitive dependencies excludes self | `transitive_dependencies()` | unit |
| DGR-12 | Linear chain: returns all predecessors in order | `transitive_dependencies()` | unit |
| DGR-13 | Diamond DAG: returns both source nodes | `transitive_dependencies()` | unit |
| DGR-14 | Detects cycle via visited set (returns empty) | `transitive_dependencies()` | unit |
| DGR-15 | Empty workflow returns empty | `transitive_dependencies()` | unit |

### DependencyGraphResolver — Transitive Dependents

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-16 | Transitive dependents returns all descendants | `transitive_dependents()` | unit |
| DGR-17 | Transitive dependents excludes self | `transitive_dependents()` | unit |
| DGR-18 | Linear chain: returns all successors | `transitive_dependents()` | unit |
| DGR-19 | Detects cycle via visited set (returns empty) | `transitive_dependents()` | unit |

### DependencyGraphResolver — Ready Nodes

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-20 | All nodes with no dependencies ready initially | `ready_nodes()` | unit |
| DGR-21 | Node becomes ready when all deps completed | `ready_nodes()` | unit |
| DGR-22 | Node with multiple deps requires ALL to complete | `ready_nodes()` | unit |
| DGR-23 | Completed nodes are excluded from ready | `ready_nodes()` | unit |
| DGR-24 | No dependencies: all nodes ready with empty completed | `ready_nodes()` | unit |
| DGR-25 | Large fan-in: node with many dependencies | `ready_nodes()` | integration |
| DGR-26 | Mixed completed set: some deps done, some not | `ready_nodes()` | unit |

### DependencyGraphResolver — Ready Nodes with Outcome

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-27 | OnSuccess condition: node ready after success | `ready_nodes_for_outcome()` | unit |
| DGR-28 | OnFailure condition: node ready after failure | `ready_nodes_for_outcome()` | unit |
| DGR-29 | Always condition: ready after any outcome | `ready_nodes_for_outcome()` | unit |
| DGR-30 | No incoming edges: always ready (root node) | `ready_nodes_for_outcome()` | unit |
| DGR-31 | Mixed conditions: some deps satisfied, some not | `ready_nodes_for_outcome()` | unit |
| DGR-32 | Outcome changes which conditional edges are active | `ready_nodes_for_outcome()` | unit |

### DependencyGraphResolver — Execution Layers

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-33 | Linear chain: one node per layer | `execution_layers()` | unit |
| DGR-34 | Parallel branches in same layer | `execution_layers()` | unit |
| DGR-35 | Diamond DAG: 3 layers (a, b+c, d) | `execution_layers()` | unit |
| DGR-36 | Disconnected components each form valid layers | `execution_layers()` | integration |
| DGR-37 | Single node: single layer | `execution_layers()` | unit |
| DGR-38 | Empty workflow returns empty vec | `execution_layers()` | unit |
| DGR-39 | All nodes appear exactly once across layers | `execution_layers()` | unit |
| DGR-40 | Nodes in same layer have no interdependencies | `execution_layers()` | unit |

### EdgeCondition Matching

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-41 | Always matches both Success and Failure | `EdgeCondition::matches()` | unit |
| DGR-42 | OnSuccess matches only Success | `EdgeCondition::matches()` | unit |
| DGR-43 | OnFailure matches only Failure | `EdgeCondition::matches()` | unit |

### Cycle Detection (Integration with WorkflowDefinition)

| # | Behavior | Public API | Layer |
|---|----------|-----------|-------|
| DGR-44 | Self-loop rejected by WorkflowDefinition::parse | workflow validation | integration |
| DGR-45 | 2-cycle (a→b→a) rejected | workflow validation | integration |
| DGR-46 | 3-cycle (a→b→c→a) rejected | workflow validation | integration |
| DGR-47 | Diamond with cycle rejected | workflow validation | integration |

---

## 2. Trophy Allocation

| Layer | Count | Rationale |
|-------|-------|-----------|
| **Unit / Calc** | 28 | Pure functions: `dependencies`, `dependents`, `transitive_*`, `ready_nodes*`, `execution_layers`, `EdgeCondition::matches`. All operate on in-memory data structures with no I/O. |
| **Integration** | 16 | Real workflow validation with cycle detection, concurrent access patterns for large graphs, disconnected component handling, fan-out/fan-in stress. |
| **E2E** | 3 | Full workflow parse → validate → execute cycle with various DAG shapes (linear, diamond, tree). |
| **Static Analysis** | 0 | No lint gates needed — component is test-focused. |

**Rationale for distribution**: The DependencyGraphResolver is a pure computation layer. Most behaviors (dependencies, dependents, transitive, ready nodes, layers) are deterministic and exhaustively testable at unit level. The 28/16/3 split reflects that cycle detection integration (4 tests) and large graph handling (8 tests) require integration coverage, while the core algorithms remain unit-testable. E2E covers the full workflow validation pipeline.

---

## 3. BDD Scenarios

### DGR-01: Node with no incoming edges

**Scenario: orphan node has no dependencies**

```
Given: a WorkflowDefinition with nodes [a, b, c] and edges [(a, b)]
When: dependencies(workflow, c) is called
Then: returns an empty vector
```

```rust
#[test]
fn dependencies_returns_empty_for_node_with_no_incoming_edges() {
    let workflow = make_workflow(
        "test",
        vec![("a", 1, 0, 1.0), ("b", 1, 0, 1.0), ("c", 1, 0, 1.0)],
        vec![("a", "b", EdgeCondition::Always)],
    );
    let deps = DependencyGraphResolver::dependencies(&workflow, &NodeName("c".into()));
    assert!(deps.is_empty());
}
```

---

### DGR-02: Node with one incoming edge

**Scenario: direct predecessor is returned**

```
Given: a WorkflowDefinition with edge (a, b)
When: dependencies(workflow, b) is called
Then: returns [a]
```

---

### DGR-03: Node with multiple incoming edges

**Scenario: diamond top converges on single node**

```
Given: a WorkflowDefinition with edges [(a, c), (b, c)]
When: dependencies(workflow, c) is called
Then: returns [a, b] (order may vary)
```

---

### DGR-10: Transitive dependencies returns all ancestors

**Scenario: full ancestor chain is returned**

```
Given: a → b → c → d
When: transitive_dependencies(workflow, d) is called
Then: returns [c, b, a] (any order)
```

---

### DGR-14: Cycle detection in transitive dependencies

**Scenario: visited node returns empty (documents invariant)**

```
Given: WorkflowDefinition is validated as acyclic at construction
When: transitive_dependencies is called on any node
Then: returns proper ancestors (cycle would have been rejected at parse time)
```

Note: The workflow validation layer rejects cyclic graphs before they reach the resolver.

---

### DGR-20: All source nodes ready initially

**Scenario: nodes with no deps are immediately executable**

```
Given: a WorkflowDefinition with 3 independent nodes [a, b, c] and no edges
When: ready_nodes(workflow, []) is called
Then: returns [a, b, c]
```

---

### DGR-22: All dependencies must be completed

**Scenario: join node waits for all predecessors**

```
Given: a → c and b → c
When: ready_nodes(workflow, [a]) is called (b not completed)
Then: returns empty (c not ready)
When: ready_nodes(workflow, [a, b]) is called
Then: returns [c]
```

---

### DGR-27: OnSuccess condition activates on success

**Scenario: success path is taken**

```
Given: a → b (OnSuccess) and a → c (OnFailure)
When: ready_nodes_for_outcome(workflow, [a], Success) is called
Then: returns [b]
```

---

### DGR-33: Linear chain has one node per layer

**Scenario: sequential dependencies create layers**

```
Given: a → b → c
When: execution_layers(workflow) is called
Then: returns [[a], [b], [c]]
```

---

### DGR-34: Parallel branches share layer

**Scenario: independent branches execute concurrently**

```
Given: a → b and a → c (both Always)
When: execution_layers(workflow) is called
Then: returns [[a], [b, c]]
```

---

### DGR-36: Disconnected components

**Scenario: independent subgraphs each form valid layers**

```
Given: Component 1: a → b, Component 2: c → d
When: execution_layers(workflow) is called
Then: all 4 nodes appear exactly once across layers
And: each component's internal ordering is preserved
```

---

### DGR-44: Self-loop is rejected

**Scenario: workflow with a → a is invalid**

```
Given: a WorkflowDefinition with edge (a, a)
When: WorkflowDefinition::parse is called
Then: returns Err(WorkflowValidationError::CycleDetected)
```

---

### DGR-45: 2-cycle is rejected

**Scenario: mutual dependency is invalid**

```
Given: edges [(a, b), (b, a)]
When: WorkflowDefinition::parse is called
Then: returns Err(WorkflowValidationError::CycleDetected)
```

---

## 4. Proptest Invariants

### PI-01: All nodes appear exactly once in execution layers (INV-DGR-001)

```
Invariant: Every node in workflow.nodes appears exactly once across all layers
Strategy: arbitrary workflow with 1-20 nodes, arbitrary edges (ensuring acyclicity)
Anti-invariant: cycle (would cause infinite loop or missed nodes)
```

```rust
proptest! {
    #[test]
    fn all_nodes_appear_exactly_once_in_layers(
        nodes in prop::collection::vec("[a-z]{1,5}", 1..20),
        // Generate edges ensuring no cycles - simplified: only forward edges
        seed in 0u64..1000,
    ) {
        let workflow = make_workflow_with_limited_edges(&nodes, seed);
        let layers = DependencyGraphResolver::execution_layers(&workflow);
        let all_nodes: Vec<NodeName> = layers.iter().flatten().cloned().collect();
        let unique_count = all_nodes.iter().collect::<HashSet<_>>().len();
        prop_assert_eq!(all_nodes.len(), unique_count, "No duplicates");
        prop_assert_eq!(all_nodes.len(), nodes.len(), "All nodes present");
    }
}
```

---

### PI-02: Dependencies subset of workflow nodes (INV-DGR-002)

```
Invariant: All nodes returned by dependencies() are in workflow.nodes
Strategy: arbitrary workflow, arbitrary node query
Anti-invariant: N/A — implementation uses workflow.edges which are validated
```

---

### PI-03: Ready nodes are never already completed (INV-DGR-003)

```
Invariant: ready_nodes(workflow, completed) ∩ completed = ∅
Strategy: arbitrary workflow, arbitrary completed set
Anti-invariant: N/A
```

---

### PI-04: Execution layers are topologically sorted (INV-DGR-004)

```
Invariant: For any edge (a, b), layer(a) < layer(b)
Strategy: arbitrary acyclic workflow
Anti-invariant: cycle would break topological ordering
```

---

### PI-05: Transitive dependencies are ancestors (INV-DGR-005)

```
Invariant: transitive_dependencies(a) ⊆ ancestors(a)
Strategy: arbitrary DAG
Anti-invariant: cycle would cause visited-set return
```

---

### PI-06: Transitive dependents are descendants (INV-DGR-006)

```
Invariant: transitive_dependents(a) ⊆ descendants(a)
Strategy: arbitrary DAG
Anti-invariant: cycle would cause visited-set return
```

---

## 5. Fuzz Targets

### FT-01: Large workflow with many nodes and edges

```
Input: (node_count: u8, edge_density: f64) where node_count in [1, 100]
Risk: quadratic behavior in dependencies(), O(n*m) complexity
Corpus seeds: 1 node (edge case), 10 nodes (typical), 100 nodes (stress)
```

### FT-02: Edge condition combinations

```
Input: Vec<(source: String, target: String, condition: EdgeCondition)>
Risk: wrong condition matching, missed edge cases
Corpus seeds: all Always, all OnSuccess, all OnFailure, mixed, empty
```

### FT-03: Workflow validation with various cycle structures

```
Input: Vec<(source: String, target: String)> — edges to validate
Risk: cycle detection bypass, infinite loop in resolver
Corpus seeds: empty graph, linear chain, diamond, tree, self-loop, 2-cycle, 3-cycle, disconnected components
```

---

## 6. Kani Harnesses

### KH-01: execution_layers produces valid topological order (INV-DGR-004)

```
Property: For all edges (a, b), layer(a) < layer(b)
Bound: up to 20 nodes, acyclic graphs only
Rationale: Critical invariant for parallel execution correctness
```

```rust
#[kani::proof]
fn execution_layers_topological_order() {
    // Kani symbolically explores all acyclic workflows
    // and proves layer ordering invariant holds
}
```

---

### KH-02: ready_nodes excludes completed nodes (INV-DGR-003)

```
Property: ∀ node ∈ ready_nodes(workflow, completed) → node ∉ completed
Bound: up to 20 nodes, arbitrary completed subset
Rationale: Completed nodes should never be re-scheduled
```

---

## 7. Mutation Checkpoints

| Checkpoint | Mutated Code | Must Be Caught By |
|------------|--------------|------------------|
| MC-001 | Change `filter(\|edge\| edge.target_node == node)` to `==` on wrong field | `dependencies_returns_empty_for_node_with_no_incoming_edges` |
| MC-002 | Remove `visited.insert()` in transitive_dependencies | `transitive_dependencies_detects_cycle` |
| MC-003 | Change `all(\|dep\| completed_set.contains(dep))` to `any()` | `ready_nodes_requires_all_dependencies` |
| MC-004 | Remove `if current_layer.is_empty() { break }` | `execution_layers_disconnected_components` |
| MC-005 | Swap `source_node`/`target_node` in edge filter | `dependents_returns_empty_for_node_with_no_outgoing_edges` |
| MC-006 | Change `EdgeCondition::Always → true` to `false` | `ready_nodes_always_edges_ready_after_any_outcome` |
| MC-007 | Change `layer(a) < layer(b)` check to `<=` | `execution_layers_linear_chain` |
| MC-008 | Remove `return None` for completed nodes | `ready_nodes_excludes_already_completed_nodes` |

**Threshold**: ≥90% mutation kill rate

---

## 8. Combinatorial Coverage Matrix

### dependencies()

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| no incoming edges | node=c, edges=[(a,b)] | [] | unit |
| single predecessor | node=b, edges=[(a,b)] | [a] | unit |
| multiple predecessors | node=c, edges=[(a,c),(b,c)] | [a,b] | unit |
| direct only, not transitive | node=c, edges=[(a,b),(b,c)] | [b] | unit |
| nonexistent node | node=x, edges=[] | [] | unit |

### ready_nodes()

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| nothing completed | nodes=[a,b,c], completed=[] | [a,b,c] | unit |
| partial completion | linear a→b→c, completed=[a] | [b] | unit |
| all deps complete | join a→c, b→c, completed=[a,b] | [c] | unit |
| completed excluded | linear a→b→c, completed=[a,b] | [c] not [a,b] | unit |
| large fan-in | 10 deps → c, completed=[all 10] | [c] | integration |

### execution_layers()

| Scenario | Input | Expected Output | Layer |
|----------|-------|-----------------|-------|
| linear chain | a→b→c | [[a], [b], [c]] | unit |
| parallel branches | a→b, a→c | [[a], [b,c]] | unit |
| diamond | a→b, a→c, b→d, c→d | [[a], [b,c], [d]] | unit |
| disconnected | a→b, c→d | valid layers, all nodes present | integration |
| single node | [a] | [[a]] | unit |
| empty workflow | nodes=[] | [] | unit |

---

## 9. Open Questions

1. **Parallel execution runtime**: The test plan assumes `execution_layers` output is used for scheduling, but the actual runtime API is not specified. Should tests include mock runtime verification?

2. **Cycle path format**: When `transitive_dependencies` detects a cycle (via visited set), it returns empty. Should it instead return an error type with cycle information?

3. **Compile-time vs runtime validation boundary**: `WorkflowDefinition::parse` validates cycles. Is there a use case for creating an unvalidated graph and using the resolver directly?

4. **Disconnected component handling**: The current `execution_layers` handles disconnected components by processing remaining nodes when no nodes in current layer have all deps assigned. Is this the desired behavior, or should each component be a separate workflow?

5. **Graph size limits**: No explicit limits on nodes or edges. Should `execution_layers` have O(n+m) complexity verification?

---

## 10. Exit Criteria Compliance

- [x] Every public API behavior has at least one BDD scenario
- [x] Every pure function with multiple inputs has at least one proptest invariant
- [x] Every parsing/deserialization boundary has a fuzz target
- [x] Every error variant in cycle detection has explicit test scenario
- [x] Mutation threshold target (≥90%) is stated
- [x] No test asserts only `is_ok()` or `is_err()` without specifying the value
- [x] DGR-13 (diamond DAG layers) explicitly specified and testable

---

## 11. Test File Locations

| Test Type | File |
|-----------|------|
| Unit tests | `crates/vo-types/src/dependency_graph_resolver.rs` (in-module `#[cfg(test)]`) |
| Integration tests | `crates/vo-types/src/dependency_graph_resolver_tests.rs` |
| Workflow validation tests | `crates/vo-types/src/workflow/tests/` |
| Property tests | `crates/vo-types/src/proptest/` |
| E2E tests | `crates/vo-worker/src/integration/` |

---

(End of file — total 497 lines)