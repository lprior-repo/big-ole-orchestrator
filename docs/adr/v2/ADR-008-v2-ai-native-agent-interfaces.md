# ADR 008 (v2): AI-Native Agent Interfaces

## Status
Accepted

## Context
Workflow engines are traditionally built with only human operators in mind. This results in complex Domain Specific Languages (DSLs), fragmented debugging tools, and GUI-only features that are hostile to programmatic automation.

The `vo-engine` treats AI agents (such as `OpenClaw`, `opencode`, or `qa-enforcer`) as first-class citizens. An AI agent must be able to read a workflow, diagnose why it failed, generate a patch, compile it, and redeploy it entirely autonomously.

## Decision
We mandate strict, deterministic JSON boundaries and a specialized CLI specifically designed for LLM consumption.

### 1. Deterministic Execution Logs
Because the engine uses Event Sourcing backed by `fjall`, every state mutation is perfectly recorded. 
The CLI command `vo-cli history <instance_id> --json` is the primary AI interface. It bypasses the UI and outputs the raw, chronological JSON event array. 
An AI agent can read this array and know with absolute mathematical certainty exactly what the payload looked like before and after a specific binary executed.

### 2. The Rust SDK Generator
When a No-Code user wants a custom integration, they prompt the AI via the UI. 
The AI does not write JavaScript or YAML. It uses the `vo-sdk` to write a native Rust task, compiles it to a binary, and deploys it. The Engine requires no custom AI sandboxing because it blindly executes the compiled binary via `tokio::process::Command` (ADR-003).

### 3. API Contract Stability
The Engine's JSON schemas (for both the DAG definition and the Event Log) are treated as immutable API contracts. We cannot arbitrarily change field names, because doing so would break the AI agents that are trained to parse and generate those specific schemas.

## Consequences
- **Positive:** True autonomous debugging. An AI agent can parse the failure, read the `stderr` logs from the JSON history, rewrite the Rust binary, compile it, and resume the workflow.
- **Positive:** By forcing AI agents to use the same Rust `vo-sdk` as human developers, we eliminate the "No-Code wall" where visual workflows become unmaintainable.
- **Negative:** Schema migrations must be meticulously managed using Serde defaults to ensure backward compatibility for AI parsers.