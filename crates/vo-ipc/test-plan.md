# Test Plan: vo-ipc Adversarial Testing (ve-6se9)

## Scope

vo-ipc has 97 existing tests covering happy paths, basic error handling, and some adversary scenarios.
This plan targets the **FD3/FD4 contract** with adversarial testing focused on five dimensions.

## Existing Coverage (gaps identified)

| Area | Covered | Gap |
|------|---------|-----|
| Envelope roundtrip | Yes | - |
| Payload size limits | Yes (at limit, over limit) | Zero-length frame |
| Schema validation | Yes (version, IDs) | Non-integer version, array instead of object |
| Identity mismatch | Yes | - |
| Timeout | Yes (elapsed_ms, stderr, reap) | Exactly 1ms timeout, timeout during read_exact |
| Signals | Yes (SIGTERM, SIGKILL) | SIGUSR1/SIGHUP during active IPC |
| Adversary subprocess | Yes (burst, immediate exit, closed fds) | Partial byte-by-byte writes, multi-frame fd4, signal-during-ipc |

## Dimensions

### D1: Partial Writes

Test that the IPC layer correctly handles incomplete/truncated writes.

| ID | Test | Type | Contract |
|----|------|------|----------|
| D1-1 | `envelope_write_then_read_with_byte_by_byte_stream` | Unit | Cursor-based: writer produces bytes consumed by reader one byte at a time |
| D1-2 | `envelope_read_truncated_at_header_boundary` | Unit | 0, 1, 2, 3 byte headers all return IncompleteRead |
| D1-3 | `envelope_read_truncated_at_payload_boundary` | Unit | Header says N bytes, only N-1 available |
| D1-4 | `envelope_read_exactly_one_byte_payload` | Unit | Single byte payload roundtrip |
| D1-5 | `adversary_fd4_byte_by_byte_response` | Integration | Subprocess writes fd4 header+payload one byte at a time |

### D2: Frame Boundary Conditions

Test edge cases at frame boundaries.

| ID | Test | Type | Contract |
|----|------|------|----------|
| D2-1 | `envelope_write_read_empty_payload` | Unit | Zero-length payload (header says 0) roundtrips as empty object parse |
| D2-2 | `envelope_read_max_size_header_zero_follow_bytes` | Unit | Header at MAX_PAYLOAD_SIZE with 0 actual bytes returns IncompleteRead |
| D2-3 | `envelope_read_multiple_frames_first_valid` | Unit | Two frames concatenated: reader returns first, leaves remainder |
| D2-4 | `envelope_read_corrupted_magic_bytes` | Unit | Header bytes are all 0xFF (max u32) triggers PayloadTooLarge |
| D2-5 | `adversary_fd4_two_envelopes_sent` | Integration | Subprocess sends two valid fd4 envelopes; parent reads first |

### D3: Concurrent Read/Write Races

Test behavior under concurrent pressure.

| ID | Test | Type | Contract |
|----|------|------|----------|
| D3-1 | `spsc_concurrent_send_recv_under_pressure` | Unit | Multi-threaded SPSC: sender fills, receiver drains, no data loss |
| D3-2 | `spsc_wraparound_stress` | Unit | Fill-drain-fill cycle 10x on small queue validates wraparound |
| D3-3 | `adversary_concurrent_fd3_fd4` | Integration | Subprocess writes fd4 while parent is still writing fd3 |

### D4: Signal Delivery During IPC

Test that signals during active IPC are handled correctly.

| ID | Test | Type | Contract |
|----|------|------|----------|
| D4-1 | `adversary_sigusr1_during_ipc` | Integration | Child receives SIGUSR1 while reading fd3; should not panic |
| D4-2 | `adversary_sighup_during_ipc` | Integration | Child receives SIGHUP while writing fd4; should not panic |
| D4-3 | `adversary_sigterm_during_fd4_write` | Integration | Child killed mid-write on fd4; parent gets error, not panic |

### D5: Timeout Edge Cases

Test timeout behavior at boundaries.

| ID | Test | Type | Contract |
|----|------|------|----------|
| D5-1 | `timeout_exactly_1ms` | Integration | 1ms timeout with instant-exit child succeeds |
| D5-2 | `timeout_with_full_stderr_buffer` | Integration | Child floods stderr past 1MB then sleeps; timeout includes truncation marker |
| D5-3 | `timeout_stderr_truncated_flag_set` | Integration | Verify stderr_truncated is true when stderr exceeds limit during timeout |
| D5-4 | `timeout_grace_period_sigkill_fallback` | Integration | Child ignores SIGTERM; SIGKILL arrives after 100ms grace |
| D5-5 | `adversary_timeout_during_read_exact` | Integration | Child sends valid header but stalls on payload; parent times out |

## Test Count

- New unit tests: ~15
- New integration tests: ~8
- New adversary subprocess scripts: ~5
- Total new: ~28 tests

## Files to Create/Modify

- `crates/vo-ipc/tests/adversarial_contract_v2.rs` - New integration test file (D3-D5)
- `crates/vo-ipc/tests/adversary_fd4_byte_by_byte.py` - New adversary script
- `crates/vo-ipc/tests/adversary_sigusr1_during_ipc.py` - New adversary script
- `crates/vo-ipc/tests/adversary_sighup_during_ipc.py` - New adversary script
- `crates/vo-ipc/tests/adversary_sigterm_mid_write.py` - New adversary script
- `crates/vo-ipc/tests/adversary_timeout_during_read.py` - New adversary script
- `crates/vo-ipc/tests/adversary_two_envelopes.py` - New adversary script
- `crates/vo-ipc/tests/adversary_concurrent_fd3_fd4.py` - New adversary script
- `crates/vo-ipc/src/envelope.rs` - Add unit tests inline (D1, D2) or in existing test files
- `crates/vo-ipc/src/spsc.rs` - Add stress tests inline
