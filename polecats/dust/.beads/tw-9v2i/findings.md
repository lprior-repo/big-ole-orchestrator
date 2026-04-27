# Findings: tw-9v2i - vo-sdk: Add 4-byte BE length prefix to FD4 writes

## Issue
CRITICAL ADR-012/014: SDK write functions write raw JSON but engine expects 4-byte BE length prefix.

## Root Cause
The `write_success_inner` and `write_failure_inner` functions in `crates/vo-sdk/src/io.rs` were writing raw JSON bytes to FD4 without prepending the length prefix required by the engine.

## Changes Made

### 1. crates/vo-sdk/src/io.rs - write_success_inner (lines 69-71)
Added 4-byte BE length prefix before JSON payload:
```rust
let len_bytes = (bytes.len() as u32).to_be_bytes();
writer.write_all(&len_bytes).map_err(|_| SdkError::WriteError)?;
writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
```

### 2. crates/vo-sdk/src/io.rs - write_failure_inner (lines 132-134)
Added 4-byte BE length prefix before JSON payload:
```rust
let len_bytes = (bytes.len() as u32).to_be_bytes();
writer.write_all(&len_bytes).map_err(|_| SdkError::WriteError)?;
writer.write_all(&bytes).map_err(|_| SdkError::WriteError)
```

### 3. crates/vo-sdk/examples/fd_test_helper.rs
Updated `test_write_success_with_fd4` and `test_write_failure_with_fd4` to skip the 4-byte length prefix when reading and parsing responses.

## Verification
- vo-sdk compiles successfully: `cargo check -p vo-sdk` passes
- vo-sdk tests pass: 64 passed (write-related tests specifically verified)

## Status
COMPLETED - Code changes committed
