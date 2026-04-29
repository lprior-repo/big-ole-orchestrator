# BLACK-HAT REVIEW: Privacy/Encryption Adversarial Audit

**Bead**: ve-1aygp  
**Parent**: ve-6e68 (Privacy/encryption)  
**ADR References**: ADR-025 (State Privacy & GDPR Purging), ADR-040 (Canonical Blob Durability)  
**Date**: 2026-04-15  
**Reviewer**: Polecat pipboy (adversarial black-hat)  
**Verdict**: **APPROVED WITH CONDITIONS**

---

## Scope

Adversarial audit of the encryption-at-rest, key management, redaction, and privacy subsystems across:
- `vo-storage/src/crypto.rs` — AES-256-GCM encryption primitives
- `vo-storage/src/key_partition/` — KEK/DEK lifecycle (generate, rotate, retire, crypto-shred)
- `vo-types/src/encryption.rs` — Encryption domain types
- `vo-types/src/dual_representation.rs` — Redaction engine
- `vo-types/tests/ai_redaction_moon_gate.rs` — PII leak prevention gate
- `vo-core/src/vault/` — Credential vault with RBAC

---

## 1. Algorithm & Cryptographic Correctness

### Algorithm: AES-256-GCM (AEAD)
- Key size: 256-bit (32 bytes)
- IV/Nonce: 96-bit (12 bytes), randomly generated via `OsRng` per operation
- Auth tag: 128-bit (16 bytes)

**PASS**. AES-256-GCM is the correct choice for authenticated encryption at rest. IV randomization via `OsRng` (CSPRNG) is proper. The 96-bit nonce space provides ~2^32 before birthday collision concerns, which is astronomically unlikely for per-instance DEK usage patterns.

### Verified Properties (all tests pass)
| Property | Test | Result |
|---|---|---|
| DEK generation produces 32 non-zero bytes | `generate_dek_produces_*` | PASS |
| Wrap/unwrap roundtrip preserves DEK | `wrap_and_unwrap_dek_roundtrip` | PASS |
| Different IV per wrapping | `wrap_dek_produces_different_output_each_time` | PASS |
| Wrong KEK rejected | `unwrap_dek_with_wrong_kek_fails` | PASS |
| Wrong DEK rejected | `decrypt_blob_with_wrong_dek_fails` | PASS |
| Key rotation preserves DEK | `key_rotation_preserves_dek_through_rewrap` | PASS |
| Old KEK cannot decrypt after rotation | `key_rotation_old_kek_cannot_decrypt_new_wrapping` | PASS |
| Destroyed KEK prevents decryption | `expired_key_simulated_by_destroying_kek` | PASS |
| Large payload (100KB) roundtrip | `encryption_roundtrip_large_payload` | PASS |
| Empty payload roundtrip | `encryption_empty_payload_succeeds` | PASS |

**38 crypto tests pass.**

---

## 2. Key Management (KEK/DEK Hierarchy)

### Architecture
```
KEK (caller-managed, never stored)
 └── wraps DEK (per-instance, stored as WrappedDek)
      └── encrypts blob data
```

### Invariant Verification

| Invariant | Enforced By | Test Coverage |
|---|---|---|
| DEKs NEVER stored unwrapped | Type system: `WrappedDek` in `DekEntry` | `fjall_dek_store` tests verify only wrapped form persisted |
| One active DEK per instance | `DekStore::generate_and_store_dek` returns `DekAlreadyExists` | `generate_and_store_dek_duplicate_fails` |
| Retired DEKs cannot be retrieved | `retrieve_dek` checks `DekStatus` | `retrieve_dek_retired_returns_error` |
| Rotation retires old before creating new | `rotate_dek` implementation order | `rotate_dek_*` tests |

**PASS**. The type system enforces that `WrappedDek` is the only form persisted. The `DekStore` trait contract prevents DEK duplication and enforces lifecycle transitions.

---

## 3. Plaintext Leak Analysis

### 3.1 No Plaintext at Rest

**VERIFIED**. Full search for plaintext leak vectors:

| Check | Result |
|---|---|
| DEK in Fjall? | NO — only `WrappedDek` (ciphertext) |
| Blob data in Fjall? | NO — only `EncryptedBlob` (ciphertext + IV + tag) |
| KEK in Fjall? | NO — passed by caller, never persisted |
| Hardcoded keys? | NO — test-only `[0x42u8; 32]`, not in production paths |
| `.env` or credential files? | NO — clean scan |

### 3.2 Plaintext in Memory (FINDING)

**F-1 (MEDIUM): No `zeroize` on DEK/KEK after use**

`retrieve_dek()` returns `[u8; 32]` — the unwrapped DEK lives on the stack until the caller's frame is reclaimed. The KEK similarly lives on the stack during wrap/unwrap. Neither is zeroed after use.

```rust
// crypto.rs:115-117
let mut dek = [0u8; DEK_SIZE_BYTES];
dek.copy_from_slice(&plaintext);
Ok(dek) // DEK bytes persist in caller's stack frame
```

**Impact**: Memory dump or core dump could contain plaintext DEK/KEK bytes. In a containerized deployment with coredumps disabled, this is low risk. In a bare-metal deployment with swap enabled, this is medium risk.

**Mitigation**: Add `zeroize` crate dependency. Implement `Drop` for key types that zeroizes on scope exit. This is a standard defense-in-depth measure.

### 3.3 No AAD in Encryption (FINDING)

**F-2 (MEDIUM): No Additional Authenticated Data (AAD) bound to ciphertext**

`encrypt_blob()` and `wrap_dek()` use AES-GCM without AAD. This means ciphertext is not bound to its context (instance ID, DEK ID, blob hash).

```rust
// crypto.rs:134-136
let ciphertext = cipher
    .encrypt(&nonce, data)  // No AAD parameter
    .map_err(|_| CryptoError::EncryptionFailed)?;
```

**Impact**: An attacker with write access to the storage layer could transplant a ciphertext from one instance to another (if both use the same DEK), and the decryption would succeed without detecting the swap. Since DEKs are per-instance, this requires the attacker to also swap the DEK mapping — a higher bar but not impossible.

**Mitigation**: Pass `instance_id + dek_id + content_hash` as AAD to `encrypt()` and `decrypt()`.

---

## 4. Redaction & Privacy (ADR-025 Dual Representation)

### 4.1 Redaction Engine

The `apply_redaction()` function provides four modes:
- `Remove`: Replace with JSON null
- `ReplaceWith(String)`: Replace with fixed placeholder (e.g., `[EMAIL_REDACTED]`)
- `ReplaceWithType`: Replace with type name
- `Hash`: Hash via `DefaultHasher`

**F-3 (LOW): Non-cryptographic hash for redaction**

`RedactionKind::Hash` uses `std::collections::hash_map::DefaultHasher` which outputs only 64 bits and is NOT designed to resist reverse engineering.

**Impact**: An attacker with knowledge of the original value could brute-force the 64-bit hash to correlate redacted data with known PII. With only 2^64 possibilities, targeted attacks are feasible for structured data (phone numbers, SSNs).

**Mitigation**: Replace with SHA-256 or BLAKE3 (already in the dependency tree). The hash output is already prefixed with `HASH`, so the format change would be transparent.

### 4.2 AI Redaction Moon Gate (809-line test suite)

**PASS**. Comprehensive PII injection tests verify zero leaks for:
- SSN, email, credit card, phone, password, TOTP
- Multi-field simultaneous redaction
- Nested object redaction
- Array-of-users redaction
- Deep nesting (5 levels)
- Canonical blob structure preservation
- GDPR purge: data absent after purge policy
- AI access control: operator projection receives redacted view

35+ tests, all passing. This is the strongest privacy enforcement in the codebase.

---

## 5. Credential Vault

**APPROVED**. `CredentialVault` provides:
- RBAC with `Permission` enum (read/write/admin)
- `SecretValue` stores ciphertext + nonce + key_version (never plaintext)
- `RotationPolicy` for automatic key rotation
- `AccessPolicy` for per-credential access control

No plaintext secrets found in vault storage.

---

## 6. Dependency Audit

| Dependency | Version | Status |
|---|---|---|
| `aes-gcm` | 0.9 | **OUTDATED** — current is 0.10.x. No known CVEs but 2 versions behind. |
| `aes` | 0.7 | **OUTDATED** — current is 0.8.x |
| `rand` | 0.8 | Current |
| `sha2` | workspace | OK |
| `blake3` | workspace | OK |

**F-4 (LOW): Outdated `aes-gcm` dependency**

The `aes-gcm` crate is at v0.9 while v0.10+ is available. No published CVEs, but trailing by 2 minor versions means missing any hardening improvements.

**Mitigation**: Upgrade `aes-gcm` to 0.10+ and `aes` to 0.8+ in next dependency refresh cycle.

---

## 7. Missing Coverage

| Gap | Severity | Description |
|---|---|---|
| No Known Answer Tests (KATs) | LOW | No test verifies crypto against published AES-GCM test vectors |
| No fuzzing of crypto operations | LOW | Fuzz target exists for key parsing but not for encrypt/decrypt |
| No AAD integration test | LOW | No test demonstrates the absence of context binding |
| No memory zeroization test | LOW | No test verifies key material is scrubbed after use |

---

## 8. Security Posture Summary

### Strengths
1. Proper AES-256-GCM AEAD with randomized IVs from CSPRNG
2. KEK/DEK two-tier hierarchy — DEKs never stored unwrapped
3. Per-instance DEK isolation — compromise of one DEK doesn't affect others
4. Crypto-shredding (DEK retirement) makes data irrecoverable (GDPR compliance)
5. Comprehensive redaction with 809-line moon gate preventing PII leaks
6. Zero hardcoded secrets in production code
7. Type-enforced separation: `WrappedDek` vs raw key bytes
8. Key rotation preserves DEK through re-wrap cycle
9. All 38 crypto tests pass

### Conditions for Approval
1. **F-1 (MEDIUM)**: Add `zeroize` crate for DEK/KEK memory scrubbing — track as follow-up
2. **F-2 (MEDIUM)**: Add AAD binding to encryption operations — track as follow-up
3. **F-3 (LOW)**: Replace `DefaultHasher` with SHA-256/BLAKE3 for redaction — track as follow-up
4. **F-4 (LOW)**: Upgrade `aes-gcm` to 0.10+ — track as dependency refresh

---

## 9. Verdict

**APPROVED WITH CONDITIONS**. The encryption architecture is sound. AES-256-GCM with KEK/DEK hierarchy is industry-standard. No plaintext leaks at rest. Redaction is comprehensive with exhaustive PII prevention testing. The four findings (F-1 through F-4) are defense-in-depth improvements, not critical vulnerabilities. The system is safe to ship with these tracked as follow-up work items.

---

## 10. Test Execution Evidence

```
cargo test -p vo-storage --lib -- "crypto"
→ 38 passed; 0 failed

cargo test -p vo-storage --lib -- "fjall_dek_store"
→ All key_partition tests pass
```
