# Test Plan: vo-sdk Adversarial Coverage (ve-z32z)

## Summary
- Behaviors identified: 42
- Trophy allocation: 18 unit / 16 integration / 8 e2e
- Proptest invariants: 8
- Fuzz targets: 2
- Kani harnesses: 3
- Mutation threshold: >=90%

## 1. Behavior Inventory

### FD3 Read (read.rs)
1. read_input_inner parses valid JSON envelope into TaskInput with correct fields
2. read_input_inner rejects empty input with InvalidInput and sets guard
3. read_input_inner rejects oversized input (>10 MiB) with InvalidInput
4. read_input_inner rejects non-UTF-8 bytes with InvalidInput
5. read_input_inner rejects malformed JSON with InvalidInput
6. read_input_inner rejects missing idempotency_key with InvalidInput
7. read_input_inner rejects missing data field with InvalidInput
8. read_input_inner rejects empty idempotency_key with InvalidInput
9. read_input_inner rejects idempotency_key with invalid characters with InvalidInput
10. read_input_inner rejects double-read with FdNotOpen (guard persistence)
11. read_input_inner sets is_read guard BEFORE parsing (failed parse still locks)
12. read_input_inner reads exactly MAX_INPUT_SIZE bytes without truncation at boundary

### FD4 Write Success (write.rs)
13. write_success_inner writes valid JSON envelope with status="success" and correct output
14. write_success_inner rejects double-write with AlreadyWritten
15. write_success_inner rejects oversized output (>10 MiB) with WriteError and sets guard
16. write_success_inner rejects I/O write failure with WriteError and sets guard
17. write_success_inner sets guard BEFORE serialization (failed serialize still locks)

### FD4 Write Failure (write.rs)
18. write_failure_inner writes valid JSON envelope with status="failure", kind, message
19. write_failure_inner rejects double-write with AlreadyWritten
20. write_failure_inner rejects message >1024 bytes with InvalidInput and sets guard
21. write_failure_inner accepts message exactly at 1024 bytes
22. write_failure_inner rejects multibyte UTF-8 messages exceeding 1024 bytes
23. write_failure_inner rejects I/O write failure with WriteError and sets guard
24. write_failure_inner sets guard BEFORE serialization

### DAG Builder (dag.rs)
25. Dag::add_node_with_kind rejects empty name with InvalidNodeName
26. Dag::add_node_with_kind rejects names with invalid chars with InvalidNodeName
27. Dag::connect rejects unknown from-node with NodeNotFound
28. Dag::connect rejects unknown to-node with NodeNotFound
29. Dag::build rejects empty workflow with EmptyWorkflow
30. Dag::build rejects invalid workflow name with InvalidNodeName
31. Dag::build accepts valid workflow and produces correct WorkflowSpec with all fields
32. Dag::build preserves node kinds through build pipeline
33. Dag::build accepts self-loops (KNOWN GAP: CycleDetected not checked)
34. Dag::build accepts 2-cycles and larger cycles (KNOWN GAP)
35. Dag::edges returns all edges as name pairs after multiple connects

### Workflow Builder (dag.rs)
36. Workflow::build delegates to Dag::build with stored workflow_name
37. Workflow::pure/effect/wait/signal/unsafe_node all produce correct NodeKind
38. Workflow::connect delegates to Dag::connect with type safety

### Graph Args (graph_args.rs)
39. parse_graph_args returns NoGraphFlag when --graph absent
40. parse_graph_args returns Ok when --graph present with no extra args
41. parse_graph_args rejects extra positional args after --graph
42. parse_graph_args handles --graph anywhere in arg list (not just last)

### WorkflowSpec Serde (graph_args.rs)
43. WorkflowSpec round-trips through JSON serialization preserving all fields
44. WorkflowSpec rejects invalid node names via serde
45. WorkflowSpec rejects invalid node kinds via serde
46. WorkflowSpec rejects malformed JSON
47. WorkflowSpec accepts extra fields silently (forward compat)
48. WorkflowSpec to_json_bytes produces valid UTF-8 JSON

### Cross-Crate Integration
49. vo_types::NodeName::parse validates names used by Dag builder
50. vo_types::WorkflowName::parse validates names used by Dag::build
51. vo_types::IdempotencyKey::parse validates keys used by read_input_inner

## 2. Trophy Allocation

| Layer | Count | Behaviors |
|-------|-------|-----------|
| Unit (Calc) | 18 | Pure parsing logic, guard state machines, error construction |
| Integration | 16 | Inner function I/O, serde boundaries, cross-crate type validation |
| E2E | 8 | Full read-then-write flows, WorkflowSpec emission via serde pipeline |

**Rationale**: vo-sdk is an I/O boundary layer. Most logic is in the `_inner` functions which are integration tests by nature (real I/O via injected readers/writers). The pure calculation is minimal (parse_envelope, size checks, guard booleans).

## 3. BDD Scenarios

### NEW TESTS (not yet covered)

#### T1: read_input_inner rejects non-UTF-8 input
```
Given: a cursor containing raw non-UTF-8 bytes (0xFF 0xFE)
When: read_input_inner is called
Then: returns Err(SdkError::InvalidInput)
And: is_read guard is set to true
```
Test name: `read_non_utf8_input_returns_invalid_input`

#### T2: read_input_inner rejects idempotency_key with spaces
```
Given: a valid envelope with idempotency_key "has spaces"
When: read_input_inner is called
Then: returns Err(SdkError::InvalidInput)
```
Test name: `read_whitespace_in_idempotency_key_returns_invalid_input`

#### T3: read_input_inner rejects idempotency_key with special characters
```
Given: a valid envelope with idempotency_key "key!@#$%"
When: read_input_inner is called
Then: returns Err(SdkError::InvalidInput)
```
Test name: `read_special_chars_in_idempotency_key_returns_invalid_input`

#### T4: read_input_inner accepts data field as any JSON value (null, array, nested)
```
Given: envelopes with data=null, data=[], data={"nested": {"deep": true}}
When: read_input_inner is called for each
Then: returns Ok with data matching input
```
Test name: `read_accepts_any_json_value_as_data`

#### T5: read_input_inner reads exactly at MAX_INPUT_SIZE boundary
```
Given: a cursor containing exactly 10*1024*1024 bytes of valid JSON
When: read_input_inner is called
Then: returns Ok (not truncated, not rejected)
```
Test name: `read_at_max_input_size_boundary_succeeds`

#### T6: read_input_inner reads one byte over MAX_INPUT_SIZE boundary
```
Given: a cursor containing exactly 10*1024*1024+1 bytes of valid JSON
When: read_input_inner is called
Then: returns Err(SdkError::InvalidInput)
```
Test name: `read_one_byte_over_max_input_size_returns_invalid_input`

#### T7: read_input_inner guard set before parsing
```
Given: a cursor with malformed JSON that fails parsing
When: read_input_inner is called
Then: returns Err(SdkError::InvalidInput)
And: is_read guard is true (guard set before parse)
```
Test name: `read_failed_parse_still_sets_guard`

#### T8: write_success_inner guard set before serialization
```
Given: a writer and a Value that will fail serialization (should not happen with serde_json, but test the guard)
And: is_written = false
When: write_failure_inner is called with message exceeding 1024 bytes
Then: returns Err(SdkError::InvalidInput)
And: is_written is true (guard set before check)
```
NOTE: Already partially tested by write_failure_message_too_long_returns_invalid_input. Need equivalent for write_success.
Test name: `write_success_oversized_still_sets_guard` (already exists - write_success_oversized_output_returns_write_error tests this)

#### T9: write_success_inner writes exact JSON structure
```
Given: output = {"result": 42}
When: write_success_inner is called
Then: written JSON has exactly keys "status" and "output"
And: no extra keys present
```
Test name: `write_success_envelope_has_exact_keys`

#### T10: write_failure_inner writes exact JSON structure
```
Given: kind=User, message="err"
When: write_failure_inner is called
Then: written JSON has exactly keys "status", "kind", "message"
And: no extra keys present
```
Test name: `write_failure_envelope_has_exact_keys`

#### T11: write_failure_inner rejects empty message
```
Given: message = ""
When: write_failure_inner is called
Then: returns Ok (empty message is valid)
And: written envelope has message=""
```
Test name: `write_failure_empty_message_succeeds`

#### T12: write_failure_inner rejects message with newlines
```
Given: message = "line1\nline2"
When: write_failure_inner is called
Then: returns Ok (newlines in message are valid)
```
Test name: `write_failure_newline_in_message_succeeds`

#### T13: write_failure_inner rejects message with null bytes
```
Given: message contains \0
When: write_failure_inner is called
Then: returns Ok (null bytes are valid UTF-8)
```
Test name: `write_failure_null_byte_in_message_succeeds`

#### T14: write_success then write_failure returns AlreadyWritten
```
Given: write_success_inner succeeds on writer
When: write_failure_inner is called on same writer with same guard
Then: returns Err(SdkError::AlreadyWritten)
```
Test name: `write_failure_after_success_returns_already_written`

#### T15: write_failure then write_success returns AlreadyWritten
```
Given: write_failure_inner succeeds on writer
When: write_success_inner is called on same writer with same guard
Then: returns Err(SdkError::AlreadyWritten)
```
Test name: `write_success_after_failure_returns_already_written`

#### T16: Dag::add_node_with_kind rejects name with only hyphens
```
Given: name = "---"
When: add_node_with_kind is called
Then: returns Err(DagError::InvalidNodeName)
```
Test name: `dag_add_node_rejects_name_with_only_hyphens`

#### T17: Dag::add_node_with_kind rejects name starting with number
```
Given: name = "123node"
When: add_node_with_kind is called
Then: returns Err(DagError::InvalidNodeName)
```
Test name: `dag_add_node_rejects_name_starting_with_number`

#### T18: Dag::add_node_with_kind rejects name with consecutive underscores
```
Given: name = "node__bad"
When: add_node_with_kind is called
Then: returns Err(DagError::InvalidNodeName)
```
Test name: `dag_add_node_rejects_consecutive_underscores`

#### T19: Dag::connect with phantom handle from different Dag returns NodeNotFound
```
Given: two separate Dag instances, each with a node
When: connect is called with handles from different Dags
Then: returns Err(DagError::NodeNotFound)
```
Test name: `dag_connect_rejects_handle_from_different_dag`

#### T20: Dag::build produces WorkflowSpec with correct edge ordering
```
Given: a Dag with edges added in order A->B, A->C, B->D
When: build is called
Then: spec.edges preserves insertion order [(A,B), (A,C), (B,D)]
```
Test name: `dag_build_preserves_edge_insertion_order`

#### T21: Dag::build produces WorkflowSpec with correct node ordering
```
Given: a Dag with nodes added in order A, B, C
When: build is called
Then: spec.nodes preserves insertion order [A, B, C]
```
Test name: `dag_build_preserves_node_insertion_order`

#### T22: Dag::build rejects workflow name with special characters
```
Given: a Dag with one node, workflow_name = "bad name!"
When: build is called
Then: returns Err(DagError::InvalidNodeName { name: "bad name!" })
```
Test name: `dag_build_rejects_workflow_name_with_special_chars`

#### T23: Dag::node_count and edge_count are consistent
```
Given: a Dag with 3 nodes and 2 edges
When: node_count() and edge_count() are called
Then: returns 3 and 2 respectively
```
Test name: `dag_node_and_edge_count_are_consistent`

#### T24: Workflow::new stores workflow_name and passes it to build
```
Given: Workflow::new("my_wf") with one node added
When: build is called
Then: spec.workflow_name.as_str() == "my_wf"
```
Test name: `workflow_build_uses_stored_workflow_name`

#### T25: parse_graph_args handles --graph in middle of args
```
Given: args = ["bin", "other", "--graph"]
When: parse_graph_args is called
Then: returns Err(GraphArgsError::UnrecognizedArgument { arg: "other" })
```
NOTE: Current implementation skips(1) then iterates. "other" comes before "--graph" so it's ignored. After "--graph" is found, "other2" would fail.
Test name: `parse_graph_args_rejects_args_after_graph_in_middle`

#### T26: parse_graph_args handles multiple --graph flags
```
Given: args = ["bin", "--graph", "--graph"]
When: parse_graph_args is called
Then: returns Err(GraphArgsError::UnrecognizedArgument { arg: "--graph" })
```
Test name: `parse_graph_args_rejects_second_graph_flag`

#### T27: parse_graph_args handles --graph as first arg
```
Given: args = ["bin", "--graph"]
When: parse_graph_args is called
Then: returns Ok(GraphArgs)
```
Test name: `parse_graph_args_accepts_graph_as_first_arg`

#### T28: WorkflowSpec JSON output uses snake_case for workflow_name
```
Given: a WorkflowSpec built from the builder
When: to_json_bytes is called
Then: JSON contains "workflow_name" key (not "workflowName")
```
Test name: `workflow_spec_json_uses_snake_case`

#### T29: WorkflowSpec with many edges serializes correctly
```
Given: a WorkflowSpec with 50 nodes and 100 edges (complete-ish graph)
When: serialized and deserialized
Then: all nodes and edges preserved
```
Test name: `workflow_spec_large_graph_roundtrip`

#### T30: NodeHandle equality is based on name
```
Given: two NodeHandle instances with same name but different phantom types
When: compared
Then: they are equal (name-based equality)
```
Test name: `node_handle_equality_is_name_based`

#### T31: NodeHandle hash is consistent with equality
```
Given: two equal NodeHandle instances
When: hashed
Then: produce same hash value
```
Test name: `node_handle_hash_consistent_with_equality`

#### T32: Cross-crate: IdempotencyKey validation rejects numeric-only keys
```
Given: idempotency_key = "12345"
When: read_input_inner parses envelope
Then: returns Err(SdkError::InvalidInput)
```
Test name: `read_numeric_idempotency_key_returns_invalid_input`

#### T33: Cross-crate: NodeName boundary at exactly 128 chars
```
Given: a node name of exactly 128 valid characters
When: Dag::add_node_with_kind is called
Then: succeeds (128 is the max)
```
Test name: `dag_add_node_accepts_name_at_max_length`

#### T34: Cross-crate: NodeName boundary at 129 chars
```
Given: a node name of exactly 129 characters
When: Dag::add_node_with_kind is called
Then: returns Err(DagError::InvalidNodeName)
```
Test name: `dag_add_node_rejects_name_over_max_length`

#### T35: Concurrent AtomicBool guard ordering (integration)
```
Given: IS_WRITTEN static AtomicBool
When: write_success is called from two threads concurrently
Then: exactly one call succeeds, the other gets AlreadyWritten
```
Test name: `concurrent_write_success_only_one_succeeds`

#### T36: Concurrent AtomicBool read guard ordering (integration)
```
Given: IS_READ static AtomicBool
When: read_input is called from two threads concurrently
Then: exactly one call succeeds, the other gets FdNotOpen
```
Test name: `concurrent_read_input_only_one_succeeds`

#### T37: SdkError Debug == Display for all variants
```
Given: each SdkError variant
When: format!("{:?}", err) and format!("{}", err) are compared
Then: they are equal (current implementation uses write!(f, "{self:?}"))
```
NOTE: Already tested. This is a property verification.

#### T38: Dag Default impl matches Dag::new()
```
Given: Dag::default() and Dag::new()
When: both are called
Then: produce equivalent empty dags
```
Test name: `dag_default_matches_new`

#### T39: GraphArgs is Copy and Clone
```
Given: GraphArgs value
When: copied and cloned
Then: all copies are equal
```
Test name: `graph_args_is_copy_and_clone`

#### T40: WorkflowSpec::to_json_bytes never panics
```
Given: any valid WorkflowSpec (including empty nodes/edges)
When: to_json_bytes is called
Then: returns non-empty Vec<u8> (expect in impl)
```
Test name: `workflow_spec_to_json_bytes_never_panics`

#### T41: GraphArgsError Display matches expected messages
```
Given: GraphArgsError::NoGraphFlag
When: to_string() is called
Then: contains "no --graph flag found"
```
Test name: `graph_args_error_no_graph_flag_display`

#### T42: TaskFailureKind is Copy, Clone, and has 3 variants
```
Given: TaskFailureKind enum
When: copied
Then: all three variants (User, System, Timeout) are accessible
```
Test name: `task_failure_kind_is_copy`

## 4. Proptest Invariants

### P1: read_input_inner never panics on any input
```
Invariant: For any byte sequence, read_input_inner returns Ok or Err, never panics
Strategy: proptest::collection::vec(any::<u8>(), 0..MAX_INPUT_SIZE)
Anti-invariant: No input should cause panic
```

### P2: write_failure_inner never panics on any message
```
Invariant: For any string message, write_failure_inner returns Ok or Err
Strategy: ".*" (any string)
Anti-invariant: No message should cause panic (including empty, huge, unicode)
```

### P3: WorkflowSpec serde round-trip preserves equality
```
Invariant: serialize(deserialize(spec)) == spec for any valid WorkflowSpec
Strategy: Generate valid node names, any combination of valid NodeKinds, any edge topology
```

### P4: Dag::build determinism
```
Invariant: build(dag, name) always returns same result for same inputs
Strategy: 1..20 nodes, random edge connections, random valid names
```

### P5: parse_graph_args is deterministic
```
Invariant: parse_graph_args(args) always returns same result
Strategy: random arg lists with "--graph" at random positions
```

### P6: write_success_inner output is valid JSON
```
Invariant: For any Value input, the written bytes parse as valid JSON with status="success"
Strategy: proptest::arbitrary::any::<Value>()
```

### P7: write_failure_inner output is valid JSON
```
Invariant: For any (kind, message) input, the written bytes parse as valid JSON
Strategy: any string message
```

### P8: Dag node_count matches actual nodes
```
Invariant: dag.node_count() == number of add_node_with_kind calls that succeeded
Strategy: 0..50 nodes with random valid names
```

## 5. Fuzz Targets

### F1: read_input_inner byte fuzzing
```
Input type: arbitrary bytes (0..11 MiB)
Risk: panic on malformed UTF-8, OOM on huge input, logic error in size check
Corpus seeds: empty, valid JSON, invalid JSON, exactly 10 MiB, 10 MiB + 1, binary garbage
Location: fuzz/fuzz_targets/parse_input.rs (exists but may need expansion)
```

### F2: WorkflowSpec JSON deserialization fuzzing
```
Input type: arbitrary bytes
Risk: panic on malformed JSON, stack overflow on deeply nested JSON, OOM on huge arrays
Corpus seeds: valid spec, empty object, array, null, deeply nested, huge node list
Location: NEW - fuzz/fuzz_targets/workflow_spec_parse.rs
```

## 6. Kani Harnesses

### K1: write_failure_inner message boundary
```
Property: write_failure_inner rejects message.len() > 1024 and accepts message.len() <= 1024
Bound: message length 0..1025
Rationale: byte boundary check is critical for contract compliance
```

### K2: read_input_inner size boundary
```
Property: read_input_inner rejects input > 10 MiB and accepts input <= 10 MiB
Bound: input size 0..10*1024*1024+1
Rationale: size check prevents OOM; off-by-one would be catastrophic
```

### K3: Dag::add_node_with_kind never panics on any string
```
Property: add_node_with_kind returns Ok or Err(DagError::InvalidNodeName), never panics
Bound: name length 0..256
Rationale: NodeName::parse is a parsing boundary; any string must be handled
```

## 7. Mutation Checkpoints

Critical mutations to survive:
- `read_input_inner`: swapping `if len == 0` and `if len > MAX_INPUT_SIZE` order -> caught by T5/T6
- `read_input_inner`: removing `*is_read = true` before parse -> caught by T7
- `write_failure_inner`: changing `> MAX_MESSAGE_BYTES` to `>=` -> caught by existing test
- `write_failure_inner`: removing guard set before check -> caught by existing test
- `Dag::build`: removing empty check -> caught by existing test
- `Dag::connect`: removing find_index validation -> caught by T19
- `parse_graph_args`: accepting args before --graph -> caught by T25

Threshold: 90% mutation kill rate minimum.

## 8. Combinatorial Coverage Matrix

### read_input_inner

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| happy path | valid JSON envelope | Ok(TaskInput) | integration |
| empty input | 0 bytes | Err(InvalidInput), guard=true | integration |
| oversized | 10 MiB + 1 bytes | Err(InvalidInput) | integration |
| exactly at max | 10 MiB valid JSON | Ok(TaskInput) | integration |
| non-UTF-8 | 0xFF 0xFE bytes | Err(InvalidInput) | integration |
| malformed JSON | "not json" | Err(InvalidInput) | integration |
| missing key field | no idempotency_key | Err(InvalidInput) | integration |
| missing data field | no data | Err(InvalidInput) | integration |
| empty key | key="" | Err(InvalidInput) | integration |
| invalid key chars | key="has spaces" | Err(InvalidInput) | integration |
| numeric key | key="12345" | Err(InvalidInput) | integration |
| double read | valid then valid | Err(FdNotOpen) on second | integration |
| failed parse guard | malformed JSON | Err(InvalidInput), guard=true | integration |
| any data value | data=null/array/nested | Ok with matching data | integration |
| arbitrary bytes | fuzz corpus | never panics | integration |

### write_success_inner

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| happy path | any Value | Ok, valid JSON | integration |
| double write | valid then valid | Err(AlreadyWritten) | integration |
| oversized | >10 MiB Value | Err(WriteError), guard=true | integration |
| I/O failure | broken writer | Err(WriteError), guard=true | integration |
| envelope structure | any Value | exact keys {status, output} | integration |
| after write_failure | shared guard | Err(AlreadyWritten) | integration |

### write_failure_inner

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| happy path (User) | kind, msg | Ok, kind="User" | integration |
| happy path (System) | kind, msg | Ok, kind="System" | integration |
| happy path (Timeout) | kind, msg | Ok, kind="Timeout" | integration |
| double write | valid then valid | Err(AlreadyWritten) | integration |
| message >1024 bytes | 1025 char msg | Err(InvalidInput), guard=true | integration |
| message =1024 bytes | 1024 char msg | Ok | integration |
| multibyte overflow | 513 x 2-byte chars | Err(InvalidInput) | integration |
| empty message | msg="" | Ok, message="" | integration |
| newline in message | msg="a\nb" | Ok | integration |
| I/O failure | broken writer | Err(WriteError), guard=true | integration |
| after write_success | shared guard | Err(AlreadyWritten) | integration |
| envelope structure | any kind, msg | exact keys {status, kind, message} | integration |

### Dag::add_node_with_kind

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| valid name | "my-node" | Ok(NodeHandle) | unit |
| empty name | "" | Err(InvalidNodeName) | unit |
| only hyphens | "---" | Err(InvalidNodeName) | unit |
| starts with number | "123node" | Err(InvalidNodeName) | unit |
| consecutive underscores | "a__b" | Err(InvalidNodeName) | unit |
| exactly 128 chars | valid 128-char name | Ok | unit |
| 129 chars | valid 129-char name | Err(InvalidNodeName) | unit |

### Dag::connect

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| valid same-dag | handles from same Dag | Ok | unit |
| phantom from-node | handle not in Dag | Err(NodeNotFound) | unit |
| phantom to-node | handle not in Dag | Err(NodeNotFound) | unit |
| cross-dag handles | handles from different Dags | Err(NodeNotFound) | unit |

### Dag::build

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| valid single node | 1 node, no edges | Ok(WorkflowSpec) | unit |
| valid multi-node | 3 nodes, 2 edges | Ok with correct spec | unit |
| empty | 0 nodes | Err(EmptyWorkflow) | unit |
| invalid workflow name | "bad name!" | Err(InvalidNodeName) | unit |
| self-loop | a->a | Ok (KNOWN GAP) | unit |
| 2-cycle | a->b->a | Ok (KNOWN GAP) | unit |
| edge ordering | edges A->B, A->C, B->D | preserves insertion order | unit |
| node ordering | nodes A, B, C | preserves insertion order | unit |

### parse_graph_args

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| no flag | ["bin"] | Err(NoGraphFlag) | unit |
| flag present | ["bin", "--graph"] | Ok(GraphArgs) | unit |
| flag first | ["bin", "--graph"] | Ok(GraphArgs) | unit |
| extra after flag | ["bin", "--graph", "extra"] | Err(UnrecognizedArgument) | unit |
| double flag | ["bin", "--graph", "--graph"] | Err(UnrecognizedArgument) | unit |

### WorkflowSpec Serde

| Scenario | Input Class | Expected Output | Test Layer |
|----------|-------------|-----------------|------------|
| valid spec | complete JSON | Ok(WorkflowSpec) | integration |
| round-trip | any valid spec | equality preserved | integration |
| malformed JSON | "{not valid" | Err | integration |
| empty JSON | "" | Err | integration |
| null workflow_name | null | Err | integration |
| extra fields | valid + extra keys | Ok (ignored) | integration |
| missing node name | {kind: "pure"} | Err | integration |
| missing edge from | {to: "a"} | Err | integration |
| self-loop edge | valid spec with a->a | Ok | integration |
| duplicate edges | valid spec with dup edges | Ok | integration |

## Open Questions

1. Should `Dag::build()` actually implement cycle detection? The `CycleDetected` variant exists but is dead code. Tests document this as a known gap.
2. Should `parse_graph_args` handle `--graph` with an equals sign (`--graph=true`)? Current impl only checks exact match.
3. Should `WorkflowSpec` validate edge references (reject edges to nonexistent nodes)? Currently serde bypasses this.
4. The `emit_graph_if_requested` function calls `process::exit(0)` making it untestable directly. Should it be refactored to accept an exit function?
