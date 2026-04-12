## Contract: Credential Vault with Rotation

### 1. Purpose

Defines the contract for a credential vault system with rotation support in the veloxide event-sourced actor system. This contract establishes the types, invariants, and error taxonomy for secure credential storage, access, and automatic rotation.

### 2. Source ADRs

- `docs/adr/v2/ADR-014-v2-secure-ipc-fd-management.md` (secret handling)
- `docs/adr/v2/ADR-012-v2-execution-boundary-hardening.md` (execution isolation)
- `docs/adr/v2/ADR-017-v2-version-pinning-validation.md` (version semantics)

### 3. Credential Types

#### 3.1 CredentialKind

Represents the category of credential.

```rust
enum CredentialKind {
    ApiKey,           // API key for external service
    Password,         // Password credential
    Token,            // OAuth or bearer token
    Certificate,      // X.509 certificate
    SigningKey,       // Ed25519, ECDSA, RSA signing key
    EncryptionKey,    // AES, ChaCha20 encryption key
    Custom(String),    // Extension point for custom credential types
}
```

#### 3.2 Credential

Core credential structure with versioning support.

```rust
struct Credential {
    id: CredentialId,
    kind: CredentialKind,
    name: String,                    // Human-readable identifier
    current_version: CredentialVersionId,
    versions: Vec<CredentialVersion>,
    rotation_policy: RotationPolicy,
    metadata: HashMap<String, String>,
    created_at: TimestampMs,
    updated_at: TimestampMs,
}
```

#### 3.3 CredentialVersion

A single version of a credential. Multiple versions exist during rotation.

```rust
struct CredentialVersion {
    version_id: CredentialVersionId,
    secret_value: SecretValue,      // The actual credential value (encrypted at rest)
    status: CredentialStatus,
    created_at: TimestampMs,
    expires_at: Option<TimestampMs>,
    rotated_from: Option<CredentialVersionId>,  // Previous version if rotated
    rotated_to: Option<CredentialVersionId>,   // Next version if rotated
}
```

#### 3.4 SecretValue

Encrypted representation of the actual credential.

```rust
struct SecretValue {
    ciphertext: Vec<u8>,            // Encrypted credential bytes
    nonce: [u8; 12],               // ChaCha20 nonce or AES-GCM IV
    key_version: u32,               // Master key version used for encryption
}
```

#### 3.5 CredentialStatus

Lifecycle status of a credential version.

```rust
enum CredentialStatus {
    Active,         // Currently in use as the primary credential
    Rotating,       // Being rotated out but still valid for verification
    Expired,        // Past expiration time, no longer valid
    Revoked,        // Manually revoked, no longer valid
    Superseded,     // Replaced by a newer version
}
```

#### 3.6 RotationPolicy

Defines how and when credential rotation occurs.

```rust
enum RotationPolicy {
    Manual,                         // No automatic rotation
    TimeBased {
        interval: Duration,         // Rotate every N days/hours
        overlap_window: Duration,    // Keep old credential valid for this duration during rotation
    },
    UsageBased {
        max_uses: u64,               // Rotate after N uses
        overlap_window: Duration,
    },
    EventBased {
        trigger_events: Vec<String>, // Rotate on specific events
        overlap_window: Duration,
    },
}
```

### 4. Vault Types

#### 4.1 VaultEntry

A stored credential in the vault.

```rust
struct VaultEntry {
    entry_id: VaultEntryId,
    credential: Credential,
    access_policy: AccessPolicy,
    rotation_state: RotationState,
}
```

#### 4.2 AccessPolicy

Defines who and how credentials can be accessed.

```rust
struct AccessPolicy {
    allowed_principals: Vec<Principal>,
    require_approval: bool,
    approvers: Vec<Principal>,
    audit_enabled: bool,
}

enum Principal {
    User(UserId),
    Actor(ActorId),
    Workflow(WorkflowId),
    System,
}
```

#### 4.3 RotationState

Current state of the credential rotation machine.

```rust
struct RotationState {
    state: RotationStatus,
    last_rotation: Option<TimestampMs>,
    next_scheduled_rotation: Option<TimestampMs>,
    consecutive_failures: u32,
    last_failure_reason: Option<String>,
}

enum RotationStatus {
    Idle,               // No rotation in progress
    Rotating,           // Rotation in progress
    WaitingForOverlap,  // Old credential still valid during overlap window
    Failed(String),     // Rotation failed with reason
}
```

### 5. Invariants (INV-*)

- **INV-001**: `CredentialId` is unique within a `CredentialVault`
- **INV-002**: A credential has exactly one version with status `Active` at any time
- **INV-003**: `SecretValue` is never stored unencrypted; master key encryption is always applied
- **INV-004**: When rotating, the old version transitions to `Rotating` (not directly to `Superseded`)
- **INV-005**: `overlap_window` defines the minimum duration old credential remains valid after new one is activated
- **INV-006**: `expired` credentials cannot be used for new operations but may still be valid for verification during overlap
- **INV-007**: `revoked` credentials are immediately invalid for all operations
- **INV-008**: `RotationState.consecutive_failures` resets to 0 on successful rotation
- **INV-009**: `next_scheduled_rotation` is computed based on `rotation_policy` after each rotation
- **INV-010**: Access to `secret_value` requires the caller to be in `allowed_principals` or be an `approver`
- **INV-011**: Audit log entry is created for every `get`, `put`, `rotate`, and `revoke` operation
- **INV-012**: `rotated_from` and `rotated_to` form a chain: `v1.rotated_to == v2` implies `v2.rotated_from == v1`
- **INV-013**: `created_at` and `updated_at` timestamps are always monotonically increasing
- **INV-014**: A credential cannot be deleted if it has any non-`Superseded` versions
- **INV-015**: `key_version` in `SecretValue` must reference a valid, non-revoked master key

### 6. Error Taxonomy

```rust
enum CredentialError {
    // Credential not found
    CredentialNotFound(CredentialId),

    // Version not found
    VersionNotFound {
        credential_id: CredentialId,
        version_id: CredentialVersionId,
    },

    // Credential is in invalid state for operation
    InvalidCredentialState {
        credential_id: CredentialId,
        current_status: CredentialStatus,
        required_status: Vec<CredentialStatus>,
        operation: String,
    },

    // Rotation failures
    RotationFailed {
        credential_id: CredentialId,
        reason: RotationFailureReason,
        retry_after: Option<Duration>,
    },

    enum RotationFailureReason {
        GenerationError(String),      // Failed to generate new credential
        StorageError(String),          // Failed to persist new credential
        EncryptionError(String),       // Failed to encrypt credential
        DecryptionError(String),       // Failed to decrypt credential
        OverlapViolation,              // Overlap window policy violated
        PolicyViolation,              // Credential doesn't support rotation
    },

    // Access control failures
    AccessDenied {
        principal: Principal,
        credential_id: CredentialId,
        required_permission: Permission,
    },

    // Expiration
    CredentialExpired {
        credential_id: CredentialId,
        version_id: CredentialVersionId,
        expired_at: TimestampMs,
    },

    // Validation
    InvalidCredentialFormat {
        kind: CredentialKind,
        detail: String,
    },

    // Key management
    MasterKeyNotFound(u32),
    MasterKeyRevoked(u32),

    // Storage
    VaultStorageError(String),
}
```

#### 6.1 Error Categories

| Error Variant | Category | Recoverable |
|--------------|----------|-------------|
| `CredentialNotFound` | NotFound | Yes (create new) |
| `VersionNotFound` | NotFound | Yes (use different version) |
| `InvalidCredentialState` | InvalidState | No (must fix state first) |
| `RotationFailed::GenerationError` | Transient | Yes (retry) |
| `RotationFailed::StorageError` | Transient | Yes (retry) |
| `RotationFailed::EncryptionError` | Fatal | No (key issue) |
| `RotationFailed::DecryptionError` | Fatal | No (key issue) |
| `RotationFailed::OverlapViolation` | PolicyViolation | No (config issue) |
| `RotationFailed::PolicyViolation` | InvalidState | No (credential type) |
| `AccessDenied` | Security | No (permissions) |
| `CredentialExpired` | Temporal | Yes (rotate) |
| `InvalidCredentialFormat` | Validation | No (input issue) |
| `MasterKeyNotFound` | NotFound | Yes (load key) |
| `MasterKeyRevoked` | Fatal | No (key revoked) |
| `VaultStorageError` | Transient | Yes (retry) |

#### 6.2 Error Display Format

- `CredentialNotFound`: "credential not found: {credential_id}"
- `VersionNotFound`: "version {version_id} not found for credential {credential_id}"
- `InvalidCredentialState`: "credential {credential_id} is {current_status}, required {required_status} for {operation}"
- `RotationFailed`: "rotation failed for {credential_id}: {reason}"
- `AccessDenied`: "access denied for {principal} on {credential_id}: requires {required_permission}"
- `CredentialExpired`: "credential {credential_id} version {version_id} expired at {expired_at}"
- `InvalidCredentialFormat`: "invalid {kind} format: {detail}"

### 7. Vault Operations

#### 7.1 Core Operations

```rust
impl CredentialVault {
    // Create a new credential
    fn create_credential(&self, entry: VaultEntry) -> Result<CredentialId, CredentialError>;

    // Get current active credential (does not expose secret_value)
    fn get_credential(&self, id: CredentialId) -> Result<Credential, CredentialError>;

    // Get secret value (requires authorization)
    fn get_secret(&self, id: CredentialId, principal: &Principal) -> Result<SecretValue, CredentialError>;

    // Update credential metadata (not the secret)
    fn update_metadata(&self, id: CredentialId, metadata: HashMap<String, String>) -> Result<(), CredentialError>;

    // Rotate credential
    fn rotate(&self, id: CredentialId, policy: Option<RotationPolicy>) -> Result<CredentialVersionId, CredentialError>;

    // Revoke a specific version
    fn revoke_version(&self, id: CredentialId, version_id: CredentialVersionId, principal: &Principal) -> Result<(), CredentialError>;

    // Revoke all versions (hard delete)
    fn revoke_all(&self, id: CredentialId, principal: &Principal) -> Result<(), CredentialError>;

    // List all credentials (metadata only, no secrets)
    fn list_credentials(&self) -> Result<Vec<CredentialSummary>, CredentialError>;

    // Get rotation status
    fn get_rotation_status(&self, id: CredentialId) -> Result<RotationState, CredentialError>;
}
```

#### 7.2 Rotation Protocol

1. **Validate**: Check credential supports rotation and is not already rotating
2. **Generate**: Create new credential value of same `CredentialKind`
3. **Encrypt**: Encrypt new value with current master key, store `SecretValue`
4. **Store**: Persist new `CredentialVersion` with status `Active`
5. **Update Old**: Transition previous `Active` version to `Rotating`, set `rotated_to`
6. **Schedule**: Set `next_scheduled_rotation` based on policy
7. **Audit**: Log rotation event with old and new version IDs

### 8. Constraints

- Credentials are encrypted at rest using AES-256-GCM or ChaCha20-Poly1305
- Master keys are stored separately and never in the same store as credentials
- Overlap window must be >= 1 minute (prevent accidental immediate revocation)
- Maximum 10 active versions per credential (older are auto-superseded)
- Vault entries are immutable except for status transitions and metadata updates
- Secret values cannot be updated in place; rotation is the only way to change a credential
- All vault operations are logged to an immutable audit trail
- Concurrent rotation requests for the same credential are serialized
- A credential in `Failed` rotation state cannot be rotated until the failure is acknowledged

### 9. Relevant Files

- `crates/vo-core/src/vault/mod.rs` (vault core types and errors)
- `crates/vo-core/src/vault/rotation.rs` (rotation state machine)
- `crates/vo-core/src/vault/access.rs` (access control)
- `crates/vo-types/src/credentials.rs` (credential types)
- `crates/vo-types/src/integer_types.rs` (TimestampMs, Duration types)
- `crates/vo-storage/src/secure_store.rs` (encrypted storage backend)

### 10. Acceptance Criteria

- [ ] All credential types (CredentialKind, Credential, CredentialVersion, SecretValue, CredentialStatus) compile and are well-formed
- [ ] All vault types (VaultEntry, AccessPolicy, RotationState) compile and are well-formed
- [ ] All invariants (INV-001 through INV-015) are formally stated
- [ ] Error taxonomy is exhaustive: every failure mode has a corresponding error variant
- [ ] Rotation protocol is deterministic: same inputs always produce same state transitions
- [ ] Access control correctly restricts `get_secret` to allowed principals
- [ ] Overlap window semantics ensure old credential remains valid during rotation transition
- [ ] Audit logging covers all credential operations (create, get, rotate, revoke)
- [ ] Contract is self-contained and does not reference nonexistent crates or files
