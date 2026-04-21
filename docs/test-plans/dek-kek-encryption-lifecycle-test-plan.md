# DEK/KEK Encryption Lifecycle Test Plan (ADR-025/040)

## Overview

This test plan covers the DEK (Data Encryption Key) and KEK (Key Encryption Key) encryption lifecycle management per ADR-025 and ADR-040. The tests verify:

1. Key rotation
2. Re-encryption with new keys
3. Expired key handling
4. DEK never stored unwrapped invariant

## Test Categories

### 1. DEK Generation Tests

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_dek_generation_produces_32_bytes` | Verify `generate_dek()` returns 32-byte key | Given valid KEK, When generate DEK, Then 32 bytes produced |
| `test_dek_generation_produces_unique_keys` | Two generated DEKs differ | Given no prior DEKs, When generate two DEKs, Then they differ |
| `test_dek_size_constant` | DEK_SIZE_BYTES = 32 | Given crypto constants, When query DEK_SIZE_BYTES, Then equals 32 |

### 2. KEK Wrapping Tests (DEK never stored unwrapped)

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_wrap_dek_produces_wrapped_dek` | `wrap_dek()` returns WrappedDek, never raw bytes | Given valid DEK and KEK, When wrap DEK, Then WrappedDek returned |
| `test_unwrap_dek_recovers_original` | Roundtrip: wrap → unwrap yields original DEK | Given DEK wrapped with KEK, When unwrap with same KEK, Then original DEK recovered |
| `test_unwrap_with_wrong_kek_fails` | Cannot unwrap with different KEK | Given DEK wrapped with KEK1, When unwrap with KEK2, Then error returned |
| `test_wrapped_dek_serde_roundtrip` | WrappedDek serializes/deserializes correctly | Given WrappedDek, When serialize and deserialize, Then same bytes recovered |

### 3. Key Rotation Tests

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_rotate_dek_preserves_data` | Data encrypted with old DEK after rotation | Given blob encrypted with DEK1, When rotate to DEK2, Then data still decryptable with DEK1's wrapped version |
| `test_re_wrap_with_new_kek` | DEK can be re-wrapped with new KEK | Given DEK wrapped with old KEK, When re-wrap with new KEK, Then new KEK can unwrap |
| `test_old_kek_cannot_decrypt_after_rotation` | Old KEK cannot decrypt after re-wrap | Given DEK re-wrapped with new KEK, When try decrypt with old KEK, Then failure |
| `test_rotation_produces_different_ciphertext` | Same plaintext encrypts differently with new DEK | Given plaintext encrypted with DEK1, When encrypt with DEK2, Then ciphertext differs |

### 4. Expired Key / Retirement Tests

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_dek_status_active_after_creation` | New DEK has Active status | Given new DEK entry, When check status, Then Active |
| `test_retire_dek_marks_status_retired` | `retire()` sets status to Retired | Given active DEK, When retire, Then status is Retired |
| `test_retired_dek_cannot_unwrap` | Cannot unwrap with retired DEK | Given retired DEK, When try unwrap, Then error returned |
| `test_destroyed_kek_prevents_unwrap` | KEK destruction makes DEK unrecoverable | Given wrapped DEK, When destroy KEK, Then cannot unwrap DEK |
| `test_crypto_shredding_makes_data_irrecoverable` | Per ADR-025: after DEK destruction, data irrecoverable | Given blob encrypted and DEK crypto-shredded, When try decrypt, Then irrecoverable |

### 5. DEK Store Integration Tests

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_dek_store_insert_and_retrieve` | Store DEK entry, retrieve by dek_id | Given DEK entry, When insert and retrieve, Then same entry recovered |
| `test_dek_store_retire_marks_retired` | Retrieve retired DEK returns error | Given retired DEK, When retrieve, Then DekStoreError::DekRetired |
| `test_dek_store_nonexistent_returns_error` | Retrieve non-existent DEK | Given non-existent dek_id, When retrieve, Then DekStoreError::DekNotFound |
| `test_dek_store_same_instance_rejects_duplicate` | Cannot have two active DEKs per instance | Given existing active DEK for instance, When insert another, Then error |
| `test_dek_store_instance_to_dek_mapping` | InstanceId maps to exactly one active DekId | Given multiple instances, When query each, Then exactly one active DekId per instance |

### 6. Invariant Verification Tests

| Test Name | Description | GWT Pattern |
|-----------|-------------|-------------|
| `test_invariant_dek_never_stored_unwrapped` | API design enforces: only WrappedDek persisted | Given DEK operations, When inspect storage, Then only WrappedDek found |
| `test_invariant_one_active_dek_per_instance` | Exactly one active DEK per InstanceId | Given DEK store, When query active DEKs, Then one per instance |
| `test_invariant_purge_ordering` | DEK destruction → index cleanup → blob removal | Given purge sequence, When observe operations, Then correct ordering |

## Failure Mode Coverage

| Failure Mode | Test | Expected Behavior |
|--------------|------|-------------------|
| Wrong KEK for unwrap | `test_unwrap_with_wrong_kek_fails` | CryptoError::KekUnwrapFailed |
| Retired DEK usage | `test_retired_dek_cannot_unwrap` | DekStoreError::DekRetired |
| Non-existent DEK | `test_dek_store_nonexistent_returns_error` | DekStoreError::DekNotFound |
| KEK destroyed | `test_destroyed_kek_prevents_unwrap` | CryptoError::KekUnwrapFailed |
| Duplicate DEK | `test_dek_store_same_instance_rejects_duplicate` | DekStoreError::DekAlreadyExists |

## Testing Trophy Allocation

- **Unit tests (70%)**: DEK generation, wrap/unwrap, status transitions
- **Integration tests (20%)**: DEK store operations, lifecycle sequences
- **Property tests (10%)**: Adversarial key generation, roundtrip invariants

## Test Execution

```bash
# Run DEK/KEK lifecycle tests
cargo test -p vo-storage dek_kek_lifecycle

# Run with property-based tests
cargo test -p vo-storage --features proptest dek_kek

# Run integration tests
cargo test -p vo-storage --test dek_store_integration
```
