# ADR 020 (v2): Fjall Key Encoding Collisions

## Status
Accepted

## Context
Fjall is an LSM-tree and operates on raw byte slices. If we encode event sequence numbers or timer timestamps as strings, lexicographic ordering will be wrong.

There is a second failure mode as well: concatenating variable-length identifiers without framing can create ambiguous prefixes and broken range scans.

## Decision
All Fjall keys must follow two rules:
1. **Numeric components use fixed-width, big-endian binary encoding.**
2. **Variable-length identifiers are length-prefixed.**

### Key Formats
1. **Events Partition:**
   `[instance_id_len_u16_be][instance_id_bytes][sequence_u64_be]`

2. **Timers Partition:**
   `[timestamp_u64_be][instance_id_len_u16_be][instance_id_bytes]`

3. **Any key involving step IDs, workflow names, or dedupe keys:**
   - length-prefix the string or use a fixed-width hash.
   - never concatenate raw text segments and hope prefix scans remain safe.

### Additional Guidance
- Exact-once partitions such as `dedupe`, `effects`, and `leases` must follow the same framing rules.
- Human readability is provided by CLI tooling, not by making the raw keys unsafe.

## Consequences
- **Positive:** Mathematically correct range scans and chronological replay.
- **Positive:** Ambiguous prefix collisions are eliminated.
- **Positive:** Performance remains strong because numeric comparison stays binary and fixed-width.
- **Negative:** Keys are not human-readable in raw database dumps, requiring the CLI to provide custom formatting for debugging.
