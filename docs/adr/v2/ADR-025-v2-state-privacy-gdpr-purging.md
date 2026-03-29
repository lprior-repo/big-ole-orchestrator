# ADR 025 (v2): State Privacy and GDPR Purging

## Status
Accepted

## Context
Because `vo-engine` uses Event Sourcing and records the output of every step into `fjall`, the event log is a complete history of all data that flowed through the system. If a workflow processes healthcare records, passwords, or PII, the `fjall` database becomes a massive compliance liability. 

Furthermore, under GDPR, a user has a "Right to Erasure." In a Key-Value store where keys are `<instance_id>:<sequence>`, you cannot easily execute a query like `DELETE WHERE email = 'test@example.com'`.

## Decision
We implement a strictly explicit lifecycle for State Privacy and Deletion.

### 1. The PII Redaction Filter (The Interceptor)
The Engine config allows operators to define a `state_filter` list of JSON keys (e.g., `["ssn", "credit_card", "password"]`). 
When an actor receives a JSON payload from a child binary, it performs a fast recursive scrub of those keys, replacing their values with `"[REDACTED]"`.
**Crucially:** The Engine writes the *scrubbed* JSON to `fjall` for the event log, but pipes the *un-scrubbed* JSON directly to the next binary via `stdin`. The workflow executes with real data, but the disk only stores the redacted data.

### 2. The GDPR Purge Tool
We provide a dedicated CLI command: `vo-cli purge --instance <id>`. 
Because the system stores instance IDs alongside identifying metadata in the `instances` partition, an operator can search for a workflow by ID and execute the purge. The command issues a `fjall.remove()` for every sequence key associated with that instance, physically deleting the event history from the disk.

## Consequences
- **Positive:** System is immediately deployable in HIPAA/GDPR environments.
- **Positive:** Clear, documented boundaries between what the Engine holds in memory (plaintext) and what it writes to disk (redacted).
- **Negative:** Redaction filtering adds a slight CPU overhead during the event appending phase.