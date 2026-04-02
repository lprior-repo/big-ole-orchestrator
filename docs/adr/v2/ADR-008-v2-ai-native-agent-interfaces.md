# ADR 008 (v2): AI-Native Agent Interfaces

## Status
Accepted

## Context
Workflow engines are traditionally built with only human operators in mind. This results in complex DSLs, fragmented debugging tools, and GUI-only features that are hostile to programmatic automation.

The `vo-engine` treats AI agents (such as `OpenClaw`, `opencode`, or `qa-enforcer`) as first-class citizens. An AI agent must be able to read a workflow, diagnose why it failed, generate a patch, compile it, and redeploy it autonomously.

## Decision
We mandate strict, deterministic JSON boundaries and a specialized CLI designed for LLM consumption.

### 1. Deterministic Execution Logs
Because the engine uses event sourcing backed by `fjall`, every state mutation is durably recorded.

The CLI exposes two history views:
1. `vo-cli history <instance_id> --json` returns the redacted operator projection intended for UI, AI, and routine debugging.
2. `vo-cli history <instance_id> --canonical` is a privileged path for exact replay and deep forensic inspection of canonical encrypted state.

AI agents default to the operator projection. They do not require unrestricted access to raw secrets or plaintext payloads to diagnose most workflow failures.

### 2. The Rust SDK Generator
When a no-code user wants a custom integration, they prompt the AI via the UI.
The AI does not write JavaScript or YAML. It uses the `vo-sdk` and the canonical `WorkflowSpec` model to write native Rust tasks and workflow definitions.

### 3. API Contract Stability
The Engine's JSON schemas for workflow definitions, operator history, and effect journal records are treated as immutable API contracts. We cannot arbitrarily change field names because doing so would break AI agents trained to parse and generate those schemas.

Schema evolution is handled through versioned contracts and upcasters (ADR-035). AI tooling should see one stable logical contract even as internal storage schemas evolve.

## Consequences
- **Positive:** True autonomous debugging remains possible without making the default AI path a privacy disaster.
- **Positive:** By forcing AI agents to use the same SDK and workflow model as human developers, we eliminate the "No-Code wall" where visual workflows become unmaintainable.
- **Negative:** Schema migrations must be meticulously managed using Serde defaults and versioned contracts to preserve AI compatibility.
- **Negative:** Some advanced AI forensics now require privileged access to canonical history rather than the default redacted view.
