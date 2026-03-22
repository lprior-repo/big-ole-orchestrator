# Black Hat Review - wtf-5gtk

## Bead: wtf-5gtk
## Title: epic: Phase 4 — API Layer (wtf-api)

## Review Date: 2026-03-22

---

## Adversarial Analysis

### Attack Surface Assessment

| Vector | Risk | Mitigations in Place |
|--------|------|---------------------|
| Malformed ID injection | LOW | Input validation via `parse_journal_request_id` rejects empty/whitespace/invalid format |
| Replay stream exhaustion | MEDIUM | No pagination limit enforced - could return massive result sets |
| Event store unavailable | LOW | Returns 500 with `actor_error` - graceful degradation |
| Untrusted event data | MEDIUM | `serde_json::from_slice` uses `.ok()` - malformed payloads silently dropped |

---

## Potential Exploits

### 1. Large Result Set (DoS Vector)
**Scenario**: Attacker crafts valid namespace/ID pointing to instance with millions of events
**Impact**: Memory exhaustion, response timeouts
**Current State**: No pagination or limit on returned entries
**Severity**: MEDIUM

### 2. Missing Pagination
**Scenario**: Legitimate client with large workflow history cannot efficiently retrieve recent entries
**Impact**: API unusable for long-running workflows
**Severity**: MEDIUM

### 3. Silent Payload Drop
**Scenario**: Malformed JSON in event payloads deserialized with `.ok()` 
**Impact**: Client receives incomplete data without indication of loss
**Severity**: LOW-MEDIUM

---

## Recommendations

1. **Add pagination**: `?limit=N&after_seq=M` parameters to bound response size
2. **Add timeout**: Limit time spent in replay loop
3. **Log dropped payloads**: Add tracing for deserialization failures

---

## Conclusion

**FINDING: ADVISORY (Non-blocking)**

No critical security vulnerabilities found. The implementation correctly:
- Validates input IDs before processing
- Handles actor unavailability gracefully
- Returns appropriate HTTP status codes

The identified issues are scalability/robustness concerns rather than security vulnerabilities. The implementation is safe to ship but should be enhanced with pagination in a future iteration.
