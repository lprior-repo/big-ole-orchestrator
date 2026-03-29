# ADR 026 (v2): AI Feedback Loop Poisoning (Circuit Breakers)

## Status
Accepted

## Context
A key differentiator of `vo-engine` is its AI-native design, allowing AI agents to diagnose failures, rewrite the Rust binary tasks, recompile, and redeploy.

However, LLMs frequently enter "hallucination loops." An AI reads a compiler error or runtime failure, generates a plausible but incorrect fix, Deploys, Fails, and repeats. Because this happens at machine speed, an unconstrained AI will burn through hundreds of compilation cycles, thrash the CPU, and fill the `/versions/<hash>/` directory with garbage binaries in minutes.

## Decision
We implement a strict, dual-layered **Circuit Breaker** on automated deployments.

### 1. The Deployment Rate Limit
The Engine enforces a hard API rate limit on binary registrations for a given workflow name (e.g., maximum 1 new version per workflow per minute). If the AI attempts to redeploy faster than this, the Engine returns `HTTP 429 Too Many Requests`, forcing the AI agent's polling loop to back off.

### 2. The Failure Loop Circuit Breaker
The Engine tracks the deployment history of every workflow. 
If a workflow registers $N$ consecutive failures (e.g., 5) across $N$ different binary version hashes within a short time window (e.g., 10 minutes), the Engine trips the circuit breaker for that workflow.
- The workflow is marked as `Quarantined`.
- The Engine explicitly rejects any further automated API deployments for that workflow name.
- The workflow must be manually un-quarantined by a human operator via the CLI (`vo-cli unquarantine <workflow_name>`) or the Dioxus UI.
- The UI prominently flags the quarantine status and displays the diffs between the failed AI-generated versions to assist the human operator.

## Consequences
- **Positive:** Protects the host server's CPU and disk space from runaway AI compilation loops.
- **Positive:** Forces "human-in-the-loop" intervention only when the AI has objectively proven it cannot solve the problem.
- **Negative:** Legitimate, complex debugging sessions by human developers using the API might occasionally trip the rate limits, requiring them to use a `--force` flag.