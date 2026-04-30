# ADR 053 (v2): Secrets Injection and Rotation

## Status
Accepted

## Context
The Veloxide system passes secrets (API keys, tokens, credentials) to workflow task binaries via the FD3 pipe mechanism described in ADR-014. The FD3 envelope (`Fd3Envelope`) carries secrets as a `BTreeMap<String, String>` in plaintext. The `vo-sdk` reads these secrets from FD3 at process startup and exposes them via `ctx.secret("KEY")`.

However, no ADR currently documents the complete lifecycle of secrets: how they are injected at startup, how encryption keys (KEK/DEK) are rotated, and how secrets are refreshed mid-workflow. The system already has partial implementations:
- `vo-ipc` carries plaintext secrets in `Fd3Envelope.secrets`
- `vo-sdk` provides `secret("KEY")` and `read_input()` for task binaries
- `vo-storage` has `DekStore` trait with `generate_and_store_dek`, `rotate_dek`, `retire_dek`
- `vo-storage` has crypto primitives: `wrap_dek`, `unwrap_dek`, `encrypt_blob`, `decrypt_blob`
- `vo-types` has `SecretValue` with `ciphertext`, `nonce`, `key_version`

The gaps are:
1. No ADR documents the startup injection flow end-to-end
2. No ADR covers KEK/DEK rotation procedures or key versioning strategy
3. Mid-workflow secret refresh (e.g., token rotation during long-running workflows) is not addressed

## Decision

### 1. Secrets Injection at Startup (FD3, Plaintext in Flight)

Secrets are injected at task binary startup via the FD3 pipe as part of the `Fd3Envelope` JSON payload. This is the existing mechanism from ADR-014.

**Flow:**
1. Engine resolves secrets from its configured secret source (CLI flags, file, or external vault)
2. Engine constructs `Fd3Envelope { version, instance_id, node_id, input, secrets: BTreeMap<String, String>, metadata }`
3. Engine serializes the envelope to JSON and sends it over FD3 to the child process
4. Child process `vo-sdk` calls `read_input()` to deserialize the envelope from FD3
5. `vo-sdk::secret("KEY")` provides direct access to individual secrets
6. Secrets are held in heap memory (`Zeroizing<Vec<u8>>` during read) and exposed as `String`

**Constraints:**
- Secrets are NEVER passed as environment variables (prevents `/proc/<pid>/environ` leakage)
- Secrets are NEVER logged
- The `FD3` pipe uses `O_CLOEXEC` to prevent child subprocess inheritance
- Secrets travel as plaintext over the IPC pipe (the pipe is kernel-isolated; it is not network-exposed)
- Secrets are never persisted to disk in the FD3 payload

**Invariants:**
- `Fd3Envelope.secrets` is the canonical source of truth for task-binary secrets
- The `vo-sdk` `IS_READ` atomic guard ensures secrets are read exactly once per process lifetime
- The `Zeroizing` wrapper zeroes the read buffer when dropped

### 2. KEK/DEK Rotation

Encryption keys follow a two-tier model:
- **KEK (Key Encryption Key):** 32-byte master key that wraps DEKs. Managed by the engine, never stored in the database.
- **DEK (Data Encryption Key):** 32-byte per-instance key that encrypts canonical event payloads at rest.

**Rotation Procedure:**

When a KEK rotation is required (e.g., compliance requirement, suspected compromise):

1. **Generate new KEK:** Create a fresh 32-byte KEK using `aes_gcm`-compatible randomness
2. **Retire all active DEKs:** For each instance in the system, call `DekStore::rotate_dek(instance_id, old_kek)` which:
   - Unwraps the active DEK with the old KEK
   - Retires the old DEK entry (marks `DekStatus::Retired`)
   - Generates a new DEK via `generate_dek()`
   - Wraps the new DEK with the NEW KEK
   - Stores the new wrapped DEK as active
3. **Update engine config:** All engine instances adopt the new KEK
4. **Decommission old KEK:** The old KEK is permanently destroyed after all DEKs have been rewrapped

**Per-Instance DEK Rotation:**

For routine per-instance DEK rotation (without KEK change):
```rust
// Existing DekStore API
fn rotate_dek(&self, instance_id: &InstanceId, kek: &[u8; 32]) -> Result<DekId, DekStoreError>;
```

This retires the old DEK, generates a new one, wraps it with the current KEK, and stores it. The old DEK remains in the store with `DekStatus::Retired` for potential legacy data access.

**Key Versioning:**

`SecretValue` includes a `key_version: u32` field. Each time a DEK is rotated, the key version is incremented. This allows:
- Identifying which DEK was used to encrypt a given secret
- Supporting multi-KEK coexistence during rotation (unwrap with either old or new KEK)
- Audit trails for key usage

### 3. Mid-Workflow Secret Refresh

Long-running workflows may require secret refresh during execution (e.g., API key rotation, token expiration). This is achieved via the signal mechanism.

**Flow:**
1. **Engine emits a secret-refresh signal** to a running instance via the signal system (ADR-042)
2. **The signal carries encrypted new secret values** (encrypted with the current DEK)
3. **The task binary receives the signal** and can access refreshed secrets
4. **The `vo-sdk` provides a secret refresh API** that updates the in-memory secrets map

**API Design:**

```rust
// vo-sdk: refresh secrets during workflow execution
pub fn refresh_secrets(encrypted_secrets: EncryptedSecretMap) -> Result<(), SdkError>;

// vo-sdk: access the current (possibly refreshed) secrets
pub fn secrets() -> &BTreeMap<String, String>;
```

**Constraints:**
- Secret refresh can only occur while the task binary is in a wait/await state (not during critical I/O)
- Refreshed secrets replace previous values in the in-memory map
- Old secret values are zeroized when replaced (using `Zeroizing`)
- The `vo-actor` tracks which instances have pending secret refreshes

**Integration with vo-actor:**

The `vo-actor` signal system already supports delivering signals to running instances. A new signal type `SignalName::SecretRefresh` carries encrypted secret updates. The actor delivers these signals to the task process via the existing signal mechanism.

### 4. Secret Storage at Rest (Canonical Payloads)

When secrets are included in event payloads stored in Fjall:
- Payloads are encrypted with the per-instance DEK (`encrypt_blob`)
- The DEK is stored wrapped by the KEK in the DEK store
- The `key_version` field on `SecretValue` tracks which key was used
- On decryption, the system checks that the key version matches an active DEK

### 5. GDPR Purge Integration (ADR-025)

The existing `vo-cli purge --instance <id>` command already integrates with this system:
1. Calls `DekStore::retire_dek()` for the instance (crypto-shredding)
2. Deletes all event payloads and snapshots for the instance
3. Removes the instance index entry

After DEK retirement, all canonical payloads encrypted with that DEK become irrecoverable.

## Consequences

### Positive
- **Complete lifecycle coverage:** Secrets from injection through rotation to purge are documented
- **Existing implementations are preserved:** All existing code (vo-ipc, vo-sdk, vo-storage, vo-actor) remains valid
- **KEK/DEK rotation is operational:** The `DekStore` trait already provides the core rotation API
- **Mid-workflow refresh is design-ready:** The signal infrastructure exists; only the signal type and SDK API need implementation
- **Backward compatible:** The `key_version` field on `SecretValue` enables future key migrations without breaking existing encrypted data

### Negative
- **FD3 secrets are plaintext in flight:** The IPC pipe is kernel-isolated but not encrypted. This is an acceptable risk for local IPC but would need TLS for remote task execution.
- **KEK management is a real subsystem:** The KEK must be securely provisioned to all engine instances and rotated in lockstep. A KEK compromise requires full DEK rewrap across all instances.
- **Mid-workflow refresh adds complexity:** Task binaries must handle secret refresh during execution, which requires careful synchronization to avoid reading stale values.

### Trade-offs
- Secrets are plaintext over FD3 vs encrypted: Plaintext is chosen because the pipe is kernel-isolated (same-host IPC), and encrypting would add complexity without meaningful security improvement for this threat model. If remote task execution is added later, TLS would be the appropriate solution.
- KEK stored in memory vs hardware module: The current implementation stores the KEK in engine memory. For higher-security deployments, integration with a hardware security module (HSM) or cloud KMS (AWS KMS, GCP KMS) would be the next step.

## Related
- ADR-014: Secure IPC and File Descriptor Management (FD3/FD4) — in-memory secret vault, FD3 secrets injection
- ADR-025: State Privacy and GDPR Purging — DEK/KEK encryption, crypto-shredding, purge tool
- ADR-035: Event Schema Evolution and Upcasting — key versioning for schema evolution
- ADR-040: Canonical Blob Durability and Publication — encrypted payload storage
- ADR-042: Signal Matching and Wake-Up Semantics — signal delivery for secret refresh
- ADR-052: Fjall WAL and Mmap Lifecycle — storage layer where encrypted payloads live
