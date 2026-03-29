# Contract Specification: Wire definition storage and registry loading

## Context
- Feature: Load workflow definitions from NATS KV on startup and populate WorkflowRegistry
- Location: vo-cli/src/commands/serve.rs (load_definitions_from_kv)
- Related: vo-actor/src/master/registry.rs (WorkflowRegistry)
- EARS Requirement: THE SYSTEM SHALL load definition sources from KV on startup

## Preconditions
- [ ] NATS KV vo-definitions bucket is provisioned
- [ ] NATS connection is established
- [ ] KV store is accessible

## Postconditions
- [ ] WorkflowRegistry.definitions contains all definitions from KV
- [ ] Each definition is deserialized as WorkflowDefinition
- [ ] Malformed entries are logged and skipped (not failures)
- [ ] Empty KV bucket is handled gracefully with info log

## Invariants
- [ ] All definitions in registry are valid WorkflowDefinition objects
- [ ] Registry keys match KV keys (namespace/name format)
- [ ] Loading does not modify KV state

## Error Taxonomy
- `anyhow::Error` - when KV scan fails (NATS connection issue)
- Skipped entries (warn level) - when entry fails to deserialize

## Error Handling Flow
1. Scan KV keys -> Error on scan failure
2. For each key: get value -> skip on get failure
3. Deserialize value -> warn and skip on parse failure
4. Collect valid definitions -> return Vec

## Non-goals
- [ ] Writing definitions to KV (that's ingest_definition's job)
- [ ] Validating workflow code (that's linter's job)
- [ ] Real-time sync of KV changes after startup
