# BEAD: vo-cdpi - definitions: Store definition source in KV after lint

id: "vo-cdpi"
title: "definitions: Store definition source in KV after lint"
type: feature
priority: 1
effort_estimate: "30min"
labels: [definitions, kv, storage]

clarification_status: "RESOLVED"

resolved_clarifications:
  - question: "What format should definitions be stored in KV?"
    answer: "Raw source string as UTF-8 bytes, as-is from the POST body field `source`"
    decided_by: "NEXT.md Action Item 1, line 61"
    date: "2026-03-23"
  - question: "What is the KV key format for definitions?"
    answer: "`<namespace>/<workflow_type>` — but since the current endpoint has no namespace, use `default/<workflow_type>` as the key via `definition_key()`"
    decided_by: "kv.rs:161 definition_key() signature requires namespace + workflow_type"
    date: "2026-03-23"
  - question: "Should we store even when diagnostics contain warnings (valid=true but non-empty diagnostics)?"
    answer: "Yes — store when `valid == true` (no error-severity diagnostics). Warnings are acceptable."
    decided_by: "definitions.rs:21 valid = dtos.iter().all(|d| d.severity != \"error\")"
    date: "2026-03-23"
  - question: "The current DefinitionRequest only has `source`, no `workflow_type`. Where does workflow_type come from?"
    answer: "Add `workflow_type: String` field to DefinitionRequest. This is needed as the KV key component."
    decided_by: "NEXT.md Action Item 1 line 61 says 'key = workflow_type'; DefinitionRequest at requests.rs:22-24 only has source"
    date: "2026-03-23"

assumptions:
  - assumption: "KV bucket vo-definitions already exists (provisioned by provision_kv_buckets in kv.rs:77-90)"
    validation_method: "Called in serve.rs during startup via provision_kv_buckets()"
    risk_if_wrong: "Store put will fail if bucket not provisioned — but provision_kv_buckets is idempotent"
  - assumption: "KvStores is already injected as Extension<KvStores> in app.rs:66"
    validation_method: "See app.rs:66 .layer(Extension(kv))"
    risk_if_wrong: "Handler cannot access KV without the Extension"
  - assumption: "async_nats::jetstream::kv::Store::put(&self, key: &str, value: Bytes) -> Result<SequencePair, KvError>"
    validation_method: "See kv.rs:125 and lib.rs:79 for existing usage patterns"
    risk_if_wrong: "Wrong API signature would cause compile error — trivially caught"

ears_requirements:
  ubiquitous:
    - "THE SYSTEM SHALL persist workflow definitions to NATS KV after successful lint validation"
    - "THE SYSTEM SHALL store raw source text (not JSON-wrapped) as the KV value"
  event_driven:
    - trigger: "WHEN a definition passes lint validation (valid == true)"
      shall: "THE SYSTEM SHALL store the raw source in KV bucket vo-definitions with key = definition_key(\"default\", workflow_type)"
    - trigger: "WHEN KV store operation fails"
      shall: "THE SYSTEM SHALL return HTTP 500 Internal Server Error with ApiError { error: \"kv_store_failure\", message: <detail> }"
    - trigger: "WHEN the lint produces error-severity diagnostics (valid == false)"
      shall: "THE SYSTEM SHALL return HTTP 200 with DefinitionResponse { valid: false, diagnostics } and NOT store in KV"
    - trigger: "WHEN the source fails to parse entirely (lint returns Err)"
      shall: "THE SYSTEM SHALL return HTTP 400 with ApiError { error: \"parse_error\", message: <detail> } and NOT store in KV"
  unwanted:
    - condition: "IF the definition fails lint (parse error or error-severity diagnostics)"
      shall_not: "THE SYSTEM SHALL NOT store the definition in KV"
      because: "Invalid definitions would corrupt the workflow registry and cause immediate termination when loaded"

contracts:
  preconditions:
    auth_required: false
    required_inputs:
      - field: "workflow_type (new field on DefinitionRequest)"
        type: "String"
        constraints: "Non-empty, used as KV key component via definition_key()"
        example_valid: "my-procedural-workflow"
        example_invalid: ""
      - field: "source"
        type: "String"
        constraints: "Must be valid workflow definition that passes vo_linter::lint_workflow_code"
        example_valid: "steps:\n  - run: echo\n  - sleep: 1000"
        example_invalid: "!!!invalid yaml"
      - field: "definition_type (path param)"
        type: "String"
        constraints: "Currently unused (_definition_type), kept for route compatibility"
        example_valid: "procedural"
    system_state:
      - "NATS KV bucket vo-definitions is provisioned (provision_definitions_kv at kv.rs:77)"
      - "KvStores.definitions Store handle is available via Extension"
  postconditions:
    state_changes:
      - "KV bucket vo-definitions contains entry with key = definition_key(\"default\", req.workflow_type) and value = req.source bytes"
    return_guarantees:
      - field: "HTTP status on valid + store success"
        guarantee: "200 with DefinitionResponse { valid: true, diagnostics }"
      - field: "HTTP status on valid + store failure"
        guarantee: "500 with ApiError { error: \"kv_store_failure\", message: ... }"
      - field: "HTTP status on lint warnings only (valid=true)"
        guarantee: "200 — definition is stored"
      - field: "HTTP status on lint error (valid=false)"
        guarantee: "200 — definition is NOT stored"
      - field: "HTTP status on parse failure"
        guarantee: "400 — definition is NOT stored"
    side_effects:
      - "KV put to vo-definitions bucket (only on valid definitions)"
  invariants:
    - "Only linted definitions where valid == true are stored in KV"
    - "KV key is exactly definition_key(\"default\", workflow_type) — i.e. \"default/<workflow_type>\""
    - "KV value is exactly the raw UTF-8 bytes of req.source — no wrapping, no JSON encoding of the source string"
    - "No partial writes — either full source is stored or 500 is returned"
    - "The existing DefinitionResponse format is preserved — no new fields added to the response"

research_requirements:
  files_to_read:
    - path: "crates/vo-api/src/handlers/definitions.rs"
      what_to_extract: "Current ingest_definition function signature, lint call, valid check, response construction"
    - path: "crates/vo-storage/src/kv.rs"
      what_to_extract: "definition_key() function, KvStores struct, Store::put() usage pattern"
    - path: "crates/vo-api/src/types/requests.rs"
      what_to_extract: "DefinitionRequest struct — currently only has source field, needs workflow_type"
    - path: "crates/vo-api/src/types/responses.rs"
      what_to_extract: "DefinitionResponse struct, ApiError::new() constructor"
    - path: "crates/vo-api/src/app.rs"
      what_to_extract: "Extension<KvStores> injection at line 66"
  research_questions:
    - question: "What is the exact Store::put signature?"
      answered: true
      answer: "put(&self, key: &str, value: async_nats::Bytes) -> Result<SequencePair, KvError>. Value is created via .into() on Vec<u8>. See kv.rs:125 and lib.rs:79."
    - question: "Does definition_key() take namespace as first arg?"
      answered: true
      answer: "Yes — definition_key(namespace: &str, workflow_type: &str) -> String at kv.rs:161"
  research_complete_when:
    - "[x] definitions.rs has been read and current flow understood (37 lines, lint-only, no storage)"
    - "[x] kv.rs has been read and put API + definition_key() extracted"
    - "[x] requests.rs has been read — DefinitionRequest only has source field"
    - "[x] app.rs has been read — KvStores already injected as Extension"

inversions:
  data_integrity_failures:
    - failure: "KV write succeeds but handler panics before returning response — definition stored but client sees error"
      prevention: "Write KV first, then construct response. Idempotent key means retry is safe."
      test_for_it: "test_store_definition_idempotent"
    - failure: "Concurrent PUT to same workflow_type — last write wins"
      prevention: "Acceptable behavior — definitions are versioned by the user (re-deploy overwrites)"
      test_for_it: "test_concurrent_definition_overwrite"
  ordering_failures:
    - failure: "Lint passes but definition_type path param is ignored — could store under wrong paradigm"
      prevention: "Use req.workflow_type from body, not the path param, as the KV key component"

acceptance_tests:
  happy_paths:
    - name: "test_store_definition_after_lint"
      given: "Valid procedural workflow definition with workflow_type and source"
      when: "POST /api/v1/definitions/procedural with {\"workflow_type\": \"test-workflow\", \"source\": \"steps:\\n  - run: echo\\n  - sleep: 1000\"}"
      then:
        - "HTTP status is 200"
        - "DefinitionResponse { valid: true, diagnostics: [] }"
        - "KV bucket vo-definitions contains key \"default/test-workflow\" with value matching source"
      real_input: '{"workflow_type": "test-workflow", "source": "steps:\n  - run: echo\n  - sleep: 1000"}'
      expected_output: '{"valid": true, "diagnostics": []}'
    - name: "test_store_definition_with_warnings"
      given: "Definition that passes lint but has warning-severity diagnostics"
      when: "POST /api/v1/definitions/procedural with source that triggers warnings but not errors"
      then:
        - "HTTP status is 200"
        - "DefinitionResponse { valid: true, diagnostics: [...] } with non-empty diagnostics"
        - "KV bucket vo-definitions contains the definition"
  error_paths:
    - name: "test_parse_error_not_stored"
      given: "Source that fails to parse entirely (vo_linter::lint_workflow_code returns Err)"
      when: "POST /api/v1/definitions/procedural with invalid YAML source"
      then:
        - "HTTP status is 400"
        - "ApiError { error: \"parse_error\", message: \"...\" }"
        - "KV bucket vo-definitions does NOT contain the definition"
      real_input: '{"workflow_type": "bad", "source": "!!!invalid"}'
      expected_error: '{"error": "parse_error", "message": "..."}'
    - name: "test_lint_error_not_stored"
      given: "Source that parses but produces error-severity diagnostics (valid == false)"
      when: "POST /api/v1/definitions/procedural with source that has lint errors"
      then:
        - "HTTP status is 200"
        - "DefinitionResponse { valid: false, diagnostics: [...] }"
        - "KV bucket vo-definitions does NOT contain the definition"
    - name: "test_kv_store_failure_returns_500"
      given: "Valid definition but KV put operation fails (e.g., NATS unavailable)"
      when: "POST /api/v1/definitions/procedural with valid source, KV Store::put returns Err"
      then:
        - "HTTP status is 500"
        - "ApiError { error: \"kv_store_failure\", message: \"...\" }"

e2e_tests:
  pipeline_test:
    name: "test_full_definition_store_pipeline"
    description: "POST valid definition -> lint passes -> stored in KV -> retrievable"
    setup:
      precondition_commands:
        - "docker start vo-nats-test"
        - "cargo run -p vo-cli -- serve &"
    execute:
      command: "curl -s -X POST http://localhost:8080/api/v1/definitions/procedural -H 'Content-Type: application/json' -d '{\"workflow_type\":\"e2e-test\",\"source\":\"steps:\\n  - run: echo\\n  - sleep: 1000\"}'"
      timeout_ms: 5000
    verify:
      exit_code: 0
      stdout_contains:
        - '"valid"'
        - "true"
      side_effects:
        - "NATS CLI can retrieve: nats kv get vo-definitions 'default/e2e-test'"

verification_checkpoints:
  gate_0_research:
    name: "Research Gate"
    must_pass_before: "Writing any code"
    checks:
      - "[x] definitions.rs current implementation understood (37 lines, lint-only, no storage, no Extension)"
      - "[x] kv.rs definition_key(namespace, workflow_type) and Store::put pattern known"
      - "[x] DefinitionRequest struct at requests.rs:22-24 has only `source` field — needs `workflow_type` added"
      - "[x] KvStores already injected as Extension at app.rs:66"
    evidence_required:
      - "All function signatures and types documented above"
  gate_1_tests:
    name: "Test Gate"
    must_pass_before: "Writing implementation code"
    checks:
      - "[ ] Test for store-after-lint written and compiles"
      - "[ ] Test for parse-error-not-stored written and compiles"
      - "[ ] Test for lint-error-not-stored written and compiles"
    evidence_required:
      - "Tests exist in definitions.rs mod tests and fail (red) because no KV store logic exists"
  gate_2_implementation:
    name: "Implementation Gate"
    must_pass_before: "Declaring task complete"
    checks:
      - "[ ] All tests pass (green)"
      - "[ ] No unwrap() or expect() in new code"
      - "[ ] DefinitionRequest has workflow_type field"
      - "[ ] ingest_definition accepts Extension<KvStores>"
    evidence_required:
      - "cargo test -p vo-api shows green"
      - "cargo clippy --workspace -- -D warnings passes"

implementation_tasks:
  phase_0_research:
    parallelizable: true
    tasks:
      - task: "Read definitions.rs and extract current ingest_definition flow"
        file: "crates/vo-api/src/handlers/definitions.rs"
        done_when: "Function signature, return type, error handling documented — DONE (see research)"
      - task: "Read kv.rs and extract KV put API and definition_key()"
        file: "crates/vo-storage/src/kv.rs"
        done_when: "Store::put usage pattern, definition_key() signature documented — DONE (see research)"
      - task: "Read requests.rs to confirm DefinitionRequest needs workflow_type"
        file: "crates/vo-api/src/types/requests.rs"
        done_when: "Confirmed: DefinitionRequest at line 22-24 only has source — DONE"
  phase_1_tests_first:
    parallelizable: true
    gate_required: "gate_0_research"
    tasks:
      - task: "Add workflow_type field to DefinitionRequest struct"
        file: "crates/vo-api/src/types/requests.rs"
        done_when: "DefinitionRequest { source: String, workflow_type: String } compiles"
      - task: "Write integration test for definition storage after successful lint"
        file: "crates/vo-api/src/handlers/definitions.rs"
        done_when: "Test compiles and fails (red) — no KV store in handler yet"
      - task: "Write test for parse-error path does not store in KV"
        file: "crates/vo-api/src/handlers/definitions.rs"
        done_when: "Test compiles and fails (red)"
      - task: "Write test for lint-error (valid=false) path does not store in KV"
        file: "crates/vo-api/src/handlers/definitions.rs"
        done_when: "Test compiles and fails (red)"
  phase_2_implementation:
    parallelizable: false
    gate_required: "gate_1_tests"
    tasks:
      - task: "Add Extension<KvStores> parameter to ingest_definition"
        file: "crates/vo-api/src/handlers/definitions.rs:5-8"
        done_when: "Function signature accepts Extension(kv): Extension<KvStores>"
      - task: "Add KV store call after successful lint (valid == true)"
        file: "crates/vo-api/src/handlers/definitions.rs:21-28"
        done_when: "After valid check passes, call kv.definitions.put(definition_key(\"default\", &req.workflow_type), req.source.as_bytes().to_vec().into()).await and handle error"
      - task: "Return 500 on KV store failure"
        file: "crates/vo-api/src/handlers/definitions.rs"
        done_when: "KV put Err mapped to (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new(\"kv_store_failure\", ...)))"

failure_modes:
  - symptom: "Definition not in KV after POST returns 200"
    likely_cause: "KV put called but .await missing, or valid check wrong"
    where_to_look:
      - file: "crates/vo-api/src/handlers/definitions.rs"
        what_to_check: "Is .await called on the KV put operation? Is the put inside the valid == true branch?"
    fix_pattern: "Ensure async KV write is properly awaited, and only executed when valid == true"
  - symptom: "Compile error: cannot find definition_key in scope"
    likely_cause: "Missing use statement for vo_storage::kv::definition_key"
    where_to_look:
      - file: "crates/vo-api/src/handlers/definitions.rs:1"
        what_to_check: "Is `use vo_storage::kv::definition_key;` imported?"
    fix_pattern: "Add the import"
  - symptom: "Compile error: expected Store, found KvStores"
    likely_cause: "Trying to call put on KvStores instead of kv.definitions"
    where_to_look:
      - file: "crates/vo-api/src/handlers/definitions.rs"
        what_to_check: "Are you using kv.definitions.put() not kv.put()?"
    fix_pattern: "Access the definitions field: kv.definitions.put(...)"
  - symptom: "Deserialization error on POST — missing workflow_type field"
    likely_cause: "DefinitionRequest updated but client not sending workflow_type"
    where_to_look:
      - file: "crates/vo-api/src/types/requests.rs:22-24"
        what_to_check: "Is workflow_type field present?"
    fix_pattern: "Ensure workflow_type is in the request body"
  - symptom: "clippy error: clippy::unwrap_used"
    likely_cause: "Using .unwrap() or .expect() on KV operations"
    where_to_look:
      - file: "crates/vo-api/src/handlers/definitions.rs"
        what_to_check: "Are you using match or map_err instead of unwrap?"
    fix_pattern: "Follow the pattern in kv.rs:125-127 — .await.map_err(|e| VoError::nats_publish(...))"

anti_hallucination:
  read_before_write:
    - file: "crates/vo-api/src/handlers/definitions.rs"
      must_read_first: true
      key_sections_to_understand:
        - "ingest_definition function (lines 5-37) — current lint-only flow"
        - "Match on vo_linter::lint_workflow_code result (line 9)"
        - "valid check: dtos.iter().all(|d| d.severity != \"error\") (line 21)"
        - "Error handling: ApiError::new(\"parse_error\", ...) for Err branch (line 33)"
    - file: "crates/vo-storage/src/kv.rs"
      must_read_first: true
      key_sections_to_understand:
        - "definition_key(namespace, workflow_type) at line 161 — returns format!(\"{}/{}\", namespace, workflow_type)"
        - "KvStores struct (line 23) with .definitions: Store field"
        - "Store::put usage pattern at line 125: .put(&key, bytes.into()).await.map_err(...)"
    - file: "crates/vo-api/src/types/requests.rs"
      must_read_first: true
      key_sections_to_understand:
        - "DefinitionRequest at line 22-24 — currently only has `source: String`"
    - file: "crates/vo-api/src/types/responses.rs"
      must_read_first: true
      key_sections_to_understand:
        - "ApiError::new(error, message) at line 131 — two string args"
        - "DefinitionResponse at line 91-94 — valid: bool, diagnostics: Vec<DiagnosticDto>"
  no_placeholder_values:
    - "Do NOT use placeholder workflow definitions — use actual procedural workflow YAML: 'steps:\\n  - run: echo\\n  - sleep: 1000'"
    - "Do NOT guess the Store::put signature — it is put(&self, key: &str, value: Bytes) -> Result<SequencePair, KvError>"
    - "Do NOT invent a namespace parameter — use 'default' as hardcoded namespace string"
    - "Do NOT change the DefinitionResponse format"

context_survival:
  progress_file:
    path: ".beads/vo-cdpi/progress.txt"
    format: "Markdown checklist"
  recovery_instructions: |
    Read progress.txt and continue from last incomplete task.
    Key facts for context recovery:
    - ingest_definition is at crates/vo-api/src/handlers/definitions.rs:5-37
    - Currently lint-only, needs KV store after valid == true
    - DefinitionRequest needs workflow_type field added (crates/vo-api/src/types/requests.rs:22-24)
    - KV store: Extension<KvStores> already injected at app.rs:66
    - Store::put pattern: kv.definitions.put(&key, bytes.into()).await.map_err(...)
    - Key builder: definition_key("default", &req.workflow_type) from kv.rs:161
    - Error code for KV failure: "kv_store_failure" (new, not existing in codebase)

completion_checklist:
  tests:
    - "[ ] Unit test: valid definition stored in KV with correct key"
    - "[ ] Unit test: parse error does not store in KV"
    - "[ ] Unit test: lint error (valid=false) does not store in KV"
    - "[ ] Unit test: KV store failure returns 500"
    - "[ ] cargo test -p vo-api passes"
  code:
    - "[ ] Implementation uses Result<T, Error> throughout"
    - "[ ] Zero unwrap() or expect() calls in new code"
    - "[ ] DefinitionRequest has workflow_type field"
    - "[ ] ingest_definition accepts Extension<KvStores>"
    - "[ ] KV put only executed when valid == true"
    - "[ ] KV key uses definition_key(\"default\", &req.workflow_type)"
    - "[ ] KV value is raw source bytes (req.source.as_bytes().to_vec().into())"
  ci:
    - "[ ] cargo test -p vo-api passes"
    - "[ ] cargo clippy --workspace -- -D warnings passes"
    - "[ ] cargo check --workspace passes"

context:
  related_files:
    - path: "crates/vo-api/src/handlers/definitions.rs"
      relevance: "Primary file — add KV store call after lint passes (line 21-28)"
    - path: "crates/vo-storage/src/kv.rs"
      relevance: "definition_key() at line 161, KvStores.definitions Store at line 29"
    - path: "crates/vo-api/src/types/requests.rs"
      relevance: "DefinitionRequest at line 22-24 — add workflow_type field"
    - path: "crates/vo-api/src/types/responses.rs"
      relevance: "ApiError at line 124, DefinitionResponse at line 91"
    - path: "crates/vo-api/src/app.rs"
      relevance: "Extension<KvStores> already injected at line 66"
    - path: "crates/vo-actor/src/master/registry.rs"
      relevance: "Downstream consumer — WorkflowRegistry.definitions HashMap loads from KV (separate bead)"
  design_decisions:
    - decision: "Use 'default' as namespace for definition keys"
      rationale: "Current endpoint has no namespace concept. Downstream bead will add namespace support."
      reversible: true
    - decision: "Add workflow_type to DefinitionRequest body"
      rationale: "KV key requires workflow_type. Path param _definition_type exists but is unused/underspecified."
      reversible: true
    - decision: "Store raw source bytes, not JSON-wrapped"
      rationale: "Source is already a string. Storing raw bytes is simpler and more efficient. Consumers parse as needed."
      reversible: false

ai_hints:
  do:
    - "Read definitions.rs and kv.rs BEFORE writing any code"
    - "Add `use axum::extract::Extension;` and `use vo_storage::kv::{KvStores, definition_key};` to imports"
    - "Add `workflow_type: String` to DefinitionRequest struct in requests.rs"
    - "Use `kv.definitions.put(&key, req.source.as_bytes().to_vec().into()).await` — note the .into() to convert Vec<u8> to Bytes"
    - "Map KV errors to (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError::new(\"kv_store_failure\", e.to_string())))"
    - "Only call KV put when valid == true (line 21 check)"
    - "Follow existing error pattern: match on lint result, then inside Ok branch check valid, then try KV put"
    - "The function must become async and accept Extension<KvStores> — change signature"
  do_not:
    - "Do NOT modify the linter logic (vo_linter crate)"
    - "Do NOT use unwrap() or expect() on KV operations — use map_err"
    - "Do NOT change the DefinitionResponse format or add new fields"
    - "Do NOT change the HTTP status codes for existing paths (200 for valid/invalid, 400 for parse error)"
    - "Do NOT store definitions when valid == false"
    - "Do NOT wrap the source in JSON before storing — store raw UTF-8 bytes"
    - "Do NOT use the path param _definition_type as the KV key — use req.workflow_type from the body"
  constitution:
    - "Zero unwrap law: NEVER use .unwrap() or .expect()"
    - "Functional first: Use map_err over if-else for error conversion"
    - "Test first: Tests MUST exist before implementation"
    - "Existing patterns: Follow the Store::put pattern from kv.rs:125 and lib.rs:79"
