# se-xtg Findings: Plaintext API Keys in Memory

## Issue
SECURITY: API keys stored as plaintext strings in `Arc<Vec<String>>`, vulnerable to memory dump extraction. Should use hashed keys with constant-time comparison.

## Location
`crates/vo-api/src/middleware/auth.rs:14`

## Current State Analysis

### Existing Code (Lines 12-14, 29-35, 117-150)
The current code ALREADY implements:
1. Key hashing via `hash_api_key()` using blake3
2. Constant-time comparison via `constant_time_compare()` (manual XOR-based implementation)

### Remaining Issues

**1. Wrong Hash Function (Line 12-14)**
```rust
fn hash_api_key(plaintext: &str) -> String {
    blake3::hash(plaintext.as_bytes()).to_hex().to_string()
}
```
- blake3 is a FAST cryptographic hash designed for content-addressable storage (Merkle trees)
- blake3 is NOT a password hash - it is vulnerable to brute-force if hashes are leaked
- Should use argon2 (memory-hard password hash) instead

**2. Manual Constant-Time Comparison (Lines 16-27)**
```rust
fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;  // TIMING LEAK: length check before comparison
    }
    let mut diff = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
```
- The length check creates a timing difference based on key length
- Should use `subtle::ConstantTimeEq` for guaranteed constant-time behavior

## Fix Implementation

### 1. Replace blake3 with argon2 password hash
- Add argon2 dependency to vo-api Cargo.toml
- Use Argon2id for memory-hard hashing

### 2. Replace manual constant-time compare with subtle
- Use `subtle::ConstantTimeEq::ct_eq()` for constant-time comparison

## Code Changes Required

### crates/vo-api/Cargo.toml
Add: `argon2 = "0.5"`

### crates/vo-api/src/middleware/auth.rs
1. Replace `hash_api_key()` with argon2-based hashing
2. Replace `constant_time_compare()` with subtle::ConstantTimeEq
3. Update tests to use argon2 hashing

## Verification
After fix:
- API keys from VO_API_KEYS env var are hashed with argon2
- Verification uses constant-time comparison
- Memory dumps cannot extract usable key material
