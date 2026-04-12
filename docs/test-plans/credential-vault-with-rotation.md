# Test Plan: Credential Vault with Rotation

## Summary

- **Bead**: ve-scmo (Test Plan: Credential vault with rotation)
- **Contract**: ve-yzhf (Contract: Credential vault with rotation)
- **Implementation**: `crates/vo-core/src/vault/mod.rs`, `crates/vo-core/src/vault/rotation.rs`, `crates/vo-core/src/vault/access.rs`, `crates/vo-types/src/credentials.rs`
- **Behaviors identified**: 87
- **Trophy allocation**: 180 unit / 95 integration / 8 e2e / 42 proptest (Total 325 tests)
- **Proptest invariants**: 28
- **Fuzz targets**: 3
- **Kani harnesses**: 2
- **Target Mutation Kill Rate**: ≥90%

---

## 1. Behavior Inventory

### 1.1 CredentialKind Variants (6 variants + 1 extension)

1. `ApiKey` - API key for external service
2. `Password` - Password credential
3. `Token` - OAuth or bearer token
4. `Certificate` - X.509 certificate
5. `SigningKey` - Ed25519, ECDSA, RSA signing key
6. `EncryptionKey` - AES, ChaCha20 encryption key
7. `Custom(String)` - Extension point for custom credential types

### 1.2 CredentialKind Validation

8. `CredentialKind::all_variants()` returns exactly 6 standard variants
9. `CredentialKind::Custom(_)` creates a custom variant with non-empty string
10. Two `Custom` variants with same string are equal
11. Two `Custom` variants with different strings are not equal

### 1.3 CredentialStatus Variants (5 variants)

12. `Active` - Currently in use as the primary credential
13. `Rotating` - Being rotated out but still valid for verification
14. `Expired` - Past expiration time, no longer valid
15. `Revoked` - Manually revoked, no longer valid
16. `Superseded` - Replaced by a newer version

### 1.4 CredentialStatus is_terminal()

17. `is_terminal()` returns `false` for `Active`
18. `is_terminal()` returns `false` for `Rotating`
19. `is_terminal()` returns `true` for `Expired`
20. `is_terminal()` returns `true` for `Revoked`
21. `is_terminal()` returns `true` for `Superseded`

### 1.5 RotationPolicy Variants (4 variants)

22. `Manual` - No automatic rotation
23. `TimeBased { interval, overlap_window }` - Time-based rotation policy
24. `UsageBased { max_uses, overlap_window }` - Usage-based rotation policy
25. `EventBased { trigger_events, overlap_window }` - Event-based rotation policy

### 1.6 RotationPolicy Validation

26. `TimeBased.interval` must be positive duration
27. `TimeBased.overlap_window` must be >= 1 minute (INV-005)
28. `UsageBased.max_uses` must be > 0
29. `UsageBased.overlap_window` must be >= 1 minute (INV-005)
30. `EventBased.trigger_events` must be non-empty
31. `EventBased.overlap_window` must be >= 1 minute (INV-005)

### 1.7 SecretValue Structure

32. `SecretValue::new()` creates valid ciphertext with 12-byte nonce
33. `SecretValue::ciphertext()` returns encrypted bytes
34. `SecretValue::nonce()` returns 12-byte nonce array
35. `SecretValue::key_version()` returns master key version used
36. `SecretValue::key_version()` must reference valid, non-revoked key (INV-015)

### 1.8 SecretValue Encryption (INV-003)

37. `SecretValue` ciphertext is never stored unencrypted (INV-003)
38. Decrypting `SecretValue` with correct key returns original plaintext
39. Decrypting `SecretValue` with wrong key returns error

### 1.9 CredentialVersion Structure

40. `CredentialVersion::new()` creates version with generated ID
41. `CredentialVersion::status()` returns current status
42. `CredentialVersion::created_at()` returns creation timestamp
43. `CredentialVersion::expires_at()` returns optional expiration
44. `CredentialVersion::rotated_from()` returns optional previous version ID
45. `CredentialVersion::rotated_to()` returns optional next version ID
46. Version chain integrity: `v1.rotated_to == v2` implies `v2.rotated_from == v1` (INV-012)

### 1.10 Credential Structure

47. `Credential::new()` creates credential with generated ID
48. `Credential::id()` returns unique credential ID
49. `Credential::kind()` returns credential kind
50. `Credential::name()` returns human-readable identifier
51. `Credential::current_version()` returns active version ID
52. `Credential::versions()` returns all versions
53. Exactly one version has status `Active` at any time (INV-002)
54. `Credential::rotation_policy()` returns current rotation policy
55. `Credential::metadata()` returns key-value metadata
56. `Credential::created_at()` returns creation timestamp
57. `Credential::updated_at()` returns last update timestamp
58. Timestamps are monotonically increasing (INV-013)

### 1.11 VaultEntry Structure

59. `VaultEntry::new()` creates entry with generated ID
60. `VaultEntry::entry_id()` returns unique entry ID
61. `VaultEntry::credential()` returns the credential
62. `VaultEntry::access_policy()` returns access policy
63. `VaultEntry::rotation_state()` returns rotation state machine

### 1.12 AccessPolicy Structure

64. `AccessPolicy::new()` with allowed principals creates valid policy
65. `AccessPolicy::allowed_principals()` returns list of principals
66. `AccessPolicy::require_approval()` returns approval requirement flag
67. `AccessPolicy::approvers()` returns list of approvers
68. `AccessPolicy::audit_enabled()` returns audit flag

### 1.13 Principal Variants (4 variants)

69. `Principal::User(UserId)` - User principal
70. `Principal::Actor(ActorId)` - Actor principal
71. `Principal::Workflow(WorkflowId)` - Workflow principal
72. `Principal::System` - System principal

### 1.14 RotationState Structure

73. `RotationState::new()` creates idle state
74. `RotationState::state()` returns rotation status
75. `RotationState::last_rotation()` returns optional last rotation timestamp
76. `RotationState::next_scheduled_rotation()` returns optional next scheduled rotation
77. `RotationState::consecutive_failures()` returns failure count
78. `consecutive_failures` resets to 0 on successful rotation (INV-008)
79. `RotationState::last_failure_reason()` returns optional failure reason

### 1.15 RotationStatus Variants (4 variants)

80. `Idle` - No rotation in progress
81. `Rotating` - Rotation in progress
82. `WaitingForOverlap` - Old credential still valid during overlap window
83. `Failed(String)` - Rotation failed with reason

### 1.16 VaultEntry Immutability (Constraint 4)

84. Vault entries are immutable for credential and secret values
85. Only status transitions and metadata updates are allowed

---

## 2. Invariant Tests (INV-001 through INV-015)

### 2.1 INV-001: CredentialId uniqueness

86. Two credentials with same ID are considered equal
87. `CredentialId` generation produces unique IDs within a `CredentialVault`
88. `CredentialVault::create_credential()` rejects duplicate IDs

### 2.2 INV-002: Exactly one Active version

89. A credential has exactly one version with status `Active` at any time
90. `create_credential()` creates first version with `Active` status
91. `rotate()` creates new `Active` version and transitions old to `Rotating`
92. Cannot have zero `Active` versions for an existing credential
93. Cannot have two `Active` versions simultaneously

### 2.3 INV-003: SecretValue always encrypted

94. `SecretValue` ciphertext is never zero-length
95. `SecretValue` ciphertext is not equal to plaintext
96. Master key encryption is always applied before storage
97. Cannot create `SecretValue` with unencrypted bytes

### 2.4 INV-004: Rotation transitions through Rotating

98. Old version transitions to `Rotating` (not directly to `Superseded`) during rotation
99. After overlap window, `Rotating` transitions to `Superseded`
100. Cannot skip `Rotating` state during rotation

### 2.5 INV-005: overlap_window minimum 1 minute

101. `TimeBased.overlap_window` must be >= 1 minute
102. `UsageBased.overlap_window` must be >= 1 minute
103. `EventBased.overlap_window` must be >= 1 minute
104. Reject rotation policies with overlap_window < 1 minute

### 2.6 INV-006: Expired credentials blocked

105. `Expired` credentials cannot be used for new operations
106. `Expired` credentials may still be valid for verification during overlap
107. `get_secret()` returns `CredentialExpired` for expired credentials

### 2.7 INV-007: Revoked credentials immediately invalid

108. `Revoked` credentials are immediately invalid for all operations
109. `get_secret()` returns error for revoked credentials
110. No overlap window applies to revoked credentials

### 2.8 INV-008: consecutive_failures resets on success

111. After successful rotation, `consecutive_failures` resets to 0
112. After failed rotation, `consecutive_failures` increments
113. Failed state machine correctly tracks failure count

### 2.9 INV-009: next_scheduled_rotation computed from policy

114. After rotation, `next_scheduled_rotation` is computed from policy
115. `TimeBased` policy schedules rotation at `created_at + interval`
116. `UsageBased` policy schedules rotation after `max_uses`
117. `Manual` policy sets `next_scheduled_rotation` to `None`
118. `EventBased` policy schedules based on first trigger event

### 2.10 INV-010: get_secret requires authorized principal

119. `get_secret()` requires caller in `allowed_principals`
120. `get_secret()` requires caller in `approvers` if `require_approval` is true
121. `get_secret()` returns `AccessDenied` for unauthorized principals
122. System principal can access any credential

### 2.11 INV-011: Audit logging on all operations

123. `create_credential()` creates audit log entry
124. `get_credential()` creates audit log entry
125. `get_secret()` creates audit log entry
126. `update_metadata()` creates audit log entry
127. `rotate()` creates audit log entry with old and new version IDs
128. `revoke_version()` creates audit log entry
129. `revoke_all()` creates audit log entry

### 2.12 INV-012: rotated_from/rotated_to chain integrity

130. If `v1.rotated_to == Some(v2_id)`, then `v2.rotated_from == Some(v1_id)`
131. Chain traversal follows `rotated_to` links forward correctly
132. Chain traversal follows `rotated_from` links backward correctly
133. Chain is always acyclic (no circular references)

### 2.13 INV-013: Timestamps monotonically increasing

134. `created_at <= updated_at` for all credentials
135. `version.created_at` <= next version's `created_at`
136. `last_rotation <= next_scheduled_rotation` when both present
137. Timestamp updates maintain monotonicity

### 2.14 INV-014: Cannot delete non-Superseded versions

138. Cannot delete credential with any `Active` versions
139. Cannot delete credential with any `Rotating` versions
140. Cannot delete credential with any `Expired` versions
141. Cannot delete credential with any `Revoked` versions
142. Can delete credential when all versions are `Superseded`

### 2.15 INV-015: key_version must reference valid master key

143. `SecretValue.key_version()` must reference existing master key
144. `SecretValue.key_version()` must not reference revoked master key
145. Master key revocation invalidates all `SecretValue` using that key
146. Loading a new master key version updates `key_version` for new secrets

---

## 3. Error Taxonomy

### 3.1 CredentialNotFound

147. `CredentialNotFound` displays as "credential not found: {credential_id}"
148. `CredentialVault::get_credential()` returns `CredentialNotFound` for missing ID
149. `CredentialVault::get_secret()` returns `CredentialNotFound` for missing ID
150. `CredentialVault::rotate()` returns `CredentialNotFound` for missing ID

### 3.2 VersionNotFound

151. `VersionNotFound` displays as "version {version_id} not found for credential {credential_id}"
152. `get_secret()` with non-existent version returns `VersionNotFound`
153. `revoke_version()` with non-existent version returns `VersionNotFound`

### 3.3 InvalidCredentialState

154. `InvalidCredentialState` displays as "credential {credential_id} is {current_status}, required {required_status} for {operation}"
155. Rotating already rotating credential returns `InvalidCredentialState`
156. Revoking `Superseded` version returns `InvalidCredentialState`
157. Rotating credential with `Manual` policy returns `InvalidCredentialState`

### 3.4 RotationFailed Variants

158. `RotationFailed::GenerationError` displays as "rotation failed for {credential_id}: GenerationError({reason})"
159. `RotationFailed::StorageError` displays as "rotation failed for {credential_id}: StorageError({reason})"
160. `RotationFailed::EncryptionError` displays as "rotation failed for {credential_id}: EncryptionError({reason})"
161. `RotationFailed::DecryptionError` displays as "rotation failed for {credential_id}: DecryptionError({reason})"
162. `RotationFailed::OverlapViolation` displays as "rotation failed for {credential_id}: OverlapViolation"
163. `RotationFailed::PolicyViolation` displays as "rotation failed for {credential_id}: PolicyViolation"

### 3.5 AccessDenied

164. `AccessDenied` displays as "access denied for {principal} on {credential_id}: requires {required_permission}"
165. Unauthorized `get_secret()` returns `AccessDenied`
166. Unauthorized `revoke_version()` returns `AccessDenied`
167. Unauthorized `revoke_all()` returns `AccessDenied`

### 3.6 CredentialExpired

168. `CredentialExpired` displays as "credential {credential_id} version {version_id} expired at {expired_at}"
169. Accessing expired credential returns `CredentialExpired`
170. Expired credential during overlap window may still be accessible

### 3.7 InvalidCredentialFormat

171. `InvalidCredentialFormat` displays as "invalid {kind} format: {detail}"
172. Creating credential with invalid format returns `InvalidCredentialFormat`
173. `Certificate` kind validates X.509 format
174. `SigningKey` kind validates key format

### 3.8 MasterKeyNotFound and MasterKeyRevoked

175. `MasterKeyNotFound(u32)` displays as "master key not found: {version}"
176. `MasterKeyRevoked(u32)` displays as "master key revoked: {version}"
177. Using revoked key returns `MasterKeyRevoked`
178. Using non-existent key returns `MasterKeyNotFound`

### 3.9 VaultStorageError

179. `VaultStorageError(String)` displays as "vault storage error: {detail}"
180. Storage write failure returns `VaultStorageError`
181. Storage read failure returns `VaultStorageError`

---

## 4. Vault Operations

### 4.1 create_credential

182. `create_credential()` with valid `VaultEntry` returns `Ok(CredentialId)`
183. `create_credential()` stores encrypted `SecretValue`
184. `create_credential()` sets initial version to `Active` (INV-002)
185. `create_credential()` sets `created_at` and `updated_at` timestamps
186. `create_credential()` creates audit log entry (INV-011)
187. `create_credential()` with duplicate ID returns error

### 4.2 get_credential

188. `get_credential()` returns `Ok(Credential)` without secret values
189. `get_credential()` returns all versions' metadata
190. `get_credential()` does not expose `SecretValue.ciphertext`
191. `get_credential()` with valid ID returns credential
192. `get_credential()` with invalid ID returns `CredentialNotFound`
193. `get_credential()` creates audit log entry (INV-011)

### 4.3 get_secret

194. `get_secret()` with authorized principal returns `Ok(SecretValue)`
195. `get_secret()` with unauthorized principal returns `AccessDenied` (INV-010)
196. `get_secret()` with expired credential returns `CredentialExpired` (INV-006)
197. `get_secret()` with revoked credential returns error (INV-007)
198. `get_secret()` decrypts to original plaintext
199. `get_secret()` with invalid key version returns `MasterKeyNotFound`
200. `get_secret()` with revoked key version returns `MasterKeyRevoked`
201. `get_secret()` creates audit log entry (INV-011)

### 4.4 update_metadata

202. `update_metadata()` updates metadata HashMap
203. `update_metadata()` does not change secret value
204. `update_metadata()` updates `updated_at` timestamp
205. `update_metadata()` with invalid credential ID returns error
206. `update_metadata()` creates audit log entry (INV-011)

### 4.5 rotate

207. `rotate()` generates new credential value of same `CredentialKind`
208. `rotate()` encrypts new value with current master key
209. `rotate()` creates new `CredentialVersion` with status `Active` (INV-002)
210. `rotate()` transitions previous `Active` to `Rotating` (INV-004)
211. `rotate()` sets `rotated_to` on old version
212. `rotate()` sets `rotated_from` on new version
213. `rotate()` schedules next rotation based on policy (INV-009)
214. `rotate()` resets `consecutive_failures` to 0 (INV-008)
215. `rotate()` with `Manual` policy returns `InvalidCredentialState`
216. `rotate()` while already rotating returns `InvalidCredentialState`
217. `rotate()` creates audit log entry with old and new version IDs (INV-011)
218. `rotate()` with encryption failure returns `RotationFailed::EncryptionError`

### 4.6 revoke_version

219. `revoke_version()` transitions version to `Revoked` immediately (INV-007)
220. `revoke_version()` with unauthorized principal returns `AccessDenied`
221. `revoke_version()` with non-existent version returns `VersionNotFound`
222. `revoke_version()` on already revoked version is idempotent
223. `revoke_version()` does not affect other versions
224. `revoke_version()` creates audit log entry (INV-011)

### 4.7 revoke_all

225. `revoke_all()` transitions all versions to `Revoked` immediately (INV-007)
226. `revoke_all()` with unauthorized principal returns `AccessDenied`
227. `revoke_all()` with invalid credential ID returns `CredentialNotFound`
228. `revoke_all()` creates single audit log entry for mass revocation
229. After `revoke_all()`, no versions are accessible

### 4.8 list_credentials

230. `list_credentials()` returns metadata only (no secrets)
231. `list_credentials()` returns all credentials in vault
232. `list_credentials()` returns empty vec for empty vault
233. `list_credentials()` includes rotation status summary

### 4.9 get_rotation_status

234. `get_rotation_status()` returns current `RotationState`
235. `get_rotation_status()` with idle state returns `Idle`
236. `get_rotation_status()` during rotation returns `Rotating`
237. `get_rotation_status()` during overlap window returns `WaitingForOverlap`
238. `get_rotation_status()` after failed rotation returns `Failed(reason)`
239. `get_rotation_status()` with invalid credential ID returns error

---

## 5. Rotation Protocol

### 5.1 TimeBased Rotation

240. Time-based rotation triggers at `created_at + interval`
241. Time-based rotation generates new credential value
242. Time-based rotation encrypts with current master key
243. Time-based rotation stores new `SecretValue`
244. Time-based rotation transitions old to `Rotating`
245. Time-based rotation sets `overlap_window` duration
246. After overlap, transitions from `Rotating` to `Superseded`

### 5.2 UsageBased Rotation

247. Usage-based rotation tracks `get_secret()` call count
248. Usage-based rotation triggers at `max_uses` threshold
249. Usage count resets after successful rotation
250. Usage count does not reset on failed rotation

### 5.3 EventBased Rotation

251. Event-based rotation registers trigger event handlers
252. Event-based rotation triggers on `trigger_events`
253. Trigger events are matched by string equality

### 5.4 Manual Rotation

254. Manual rotation requires explicit `rotate()` call
255. Manual rotation respects overlap window
256. Manual rotation updates `last_rotation` timestamp

### 5.5 Concurrent Rotation Serialization

257. Concurrent `rotate()` calls for same credential are serialized
258. Second rotation attempt while rotating returns `InvalidCredentialState`
259. Rotation state is consistent under concurrent access

### 5.6 Failed Rotation Recovery

260. Failed rotation sets `RotationStatus::Failed(reason)`
261. `consecutive_failures` increments on failure
262. Credential in `Failed` state cannot be rotated until acknowledged
263. Acknowledging failure resets state to `Idle`

### 5.7 Overlap Window Semantics (INV-005)

264. Old credential remains valid during overlap window
265. Overlap window must be >= 1 minute
266. After overlap window, old credential is invalidated
267. Overlap window applies to all rotation policy types

---

## 6. BDD Scenarios (Given-When-Then)

### 6.1 Create Credential

**Scenario**: Create new API credential
- **Given** no existing credential with name "github-api"
- **When** `create_credential()` is called with API key credential
- **Then** credential is created with `Active` status and single version

**Scenario**: Create credential with duplicate name
- **Given** existing credential "github-api"
- **When** `create_credential()` is called with name "github-api"
- **Then** operation succeeds (names are not unique identifiers)

### 6.2 Get Secret

**Scenario**: Authorized user retrieves secret
- **Given** credential exists with authorized principal User(alice)
- **When** `get_secret()` is called by User(alice)
- **Then** secret value is returned

**Scenario**: Unauthorized user denied
- **Given** credential exists with allowed principals [User(alice)]
- **When** `get_secret()` is called by User(bob)
- **Then** `AccessDenied` error is returned

**Scenario**: Expired credential blocked
- **Given** credential version has expired
- **When** `get_secret()` is called
- **Then** `CredentialExpired` error is returned

### 6.3 Rotate Credential

**Scenario**: Successful time-based rotation
- **Given** credential with `TimeBased` policy (interval: 30 days)
- **When** rotation is triggered by time
- **Then** new version is `Active`, old version is `Rotating`
- **And** overlap window is observed
- **And** `consecutive_failures` is 0

**Scenario**: Rotation failure recovery
- **Given** credential rotation failed due to encryption error
- **When** `get_rotation_status()` is called
- **Then** status is `Failed(EncryptionError(...))`
- **And** `consecutive_failures` is incremented

**Scenario**: Rotate during active rotation fails
- **Given** credential is currently rotating
- **When** `rotate()` is called
- **Then** `InvalidCredentialState` error is returned

### 6.4 Revoke Credential

**Scenario**: Revoke specific version
- **Given** credential has 3 versions
- **When** `revoke_version()` is called for version 2
- **Then** version 2 is `Revoked`
- **And** versions 1 and 3 are unaffected

**Scenario**: Immediate revocation invalidates
- **Given** credential version is `Active`
- **When** `revoke_version()` is called
- **Then** version immediately becomes invalid
- **And** no overlap window applies

**Scenario**: Revoke all versions
- **Given** credential with 3 versions
- **When** `revoke_all()` is called
- **Then** all versions become `Revoked`
- **And** `get_secret()` returns error for any version

### 6.5 Overlap Window

**Scenario**: Old credential valid during overlap
- **Given** rotation just completed with 5-minute overlap window
- **When** `get_secret()` is called for old version
- **Then** secret is returned (old version still valid)

**Scenario**: Old credential invalid after overlap
- **Given** rotation completed and overlap window has expired
- **When** `get_secret()` is called for old version
- **Then** error is returned

### 6.6 Access Control

**Scenario**: Approver can access when approval required
- **Given** credential with `require_approval: true` and approvers [User(charlie)]
- **When** `get_secret()` is called by User(charlie)
- **Then** secret is returned

**Scenario**: System principal bypasses access control
- **Given** credential with allowed principals [User(alice)]
- **When** `get_secret()` is called by System
- **Then** secret is returned

---

## 7. Proptest Invariants

### 7.1 Encrypted Ciphertext Invariants

270. `SecretValue.ciphertext()` is never equal to input plaintext
271. `SecretValue.ciphertext()` length is >= 16 bytes (auth tag)
272. Same plaintext encrypted twice produces different ciphertext (due to nonce)

### 7.2 Version Chain Invariants

273. Following `rotated_to` chain never cycles (INV-012)
274. `rotated_from` and `rotated_to` are consistent pairs
275. Active version always has `rotated_from` set (except first version)

### 7.3 Timestamp Invariants

276. `created_at < updated_at` for any credential
277. Version timestamps are ordered by creation
278. `last_rotation < next_scheduled_rotation` when both defined

### 7.4 Active Version Invariants

279. Exactly one version has `Active` status (INV-002)
280. `current_version` always points to `Active` version
281. Cannot have `Active` version without being in `versions` list

### 7.5 Failure Counter Invariants

282. `consecutive_failures` is 0 after successful rotation
283. `consecutive_failures` increments on each failure
284. `consecutive_failures` has no upper bound in state

### 7.6 Overlap Window Invariants

285. `overlap_window` for any policy is >= 1 minute (60,000 ms)
286. `overlap_window` is well-formed duration

### 7.7 Version Limit Invariants

287. Credential has <= 10 non-Superseded versions (Constraint 3)
288. When limit is exceeded, oldest `Superseded` version is pruned

### 7.8 Key Version Invariants

289. `key_version` is always > 0
290. `key_version` references currently valid master key

---

## 8. Fuzz Targets

### 8.1 CredentialId Parsing Fuzz

**Target**: `CredentialId::parse()`
- Valid inputs: properly formatted IDs
- Invalid inputs: malformed strings, too long, special characters
- Expected: valid IDs parse successfully, invalid IDs return error

### 8.2 SecretValue Decryption Fuzz

**Target**: `SecretValue::decrypt()`
- Valid inputs: properly encrypted ciphertext
- Invalid inputs: tampered ciphertext, wrong nonce, corrupted data
- Expected: valid inputs decrypt, invalid inputs return error

### 8.3 EventBased Trigger Parsing Fuzz

**Target**: `RotationPolicy::EventBased::new()`
- Valid inputs: non-empty event strings
- Invalid inputs: empty strings, unicode, control characters
- Expected: valid inputs create policy, invalid inputs return error

---

## 9. Kani Harnesses

### 9.1 RotationState Transitions

**Harness**: Verify `RotationState` transition correctness
- Model all possible `RotationStatus` values
- Verify `Idle -> Rotating -> WaitingForOverlap -> Idle` cycle
- Verify `Rotating -> Failed -> Idle` recovery path
- Prove no invalid transitions possible

### 9.2 SecretValue Encryption Invariants

**Harness**: Verify `SecretValue` encryption correctness
- Prove `decrypt(encrypt(plaintext)) == plaintext`
- Prove ciphertext is never equal to plaintext
- Prove nonce is always 12 bytes

---

## 10. Test Execution Plan

### 10.1 Unit Tests (180 tests)

Run with: `cargo test --lib -- vault`

| Module | Tests | Focus |
|--------|-------|-------|
| `vault/mod.rs` | 45 | Core types, Error taxonomy |
| `vault/rotation.rs` | 52 | Rotation state machine, policies |
| `vault/access.rs` | 35 | Access control, principals |
| `credentials.rs` | 48 | Credential types, invariants |

### 10.2 Integration Tests (95 tests)

Run with: `cargo test --test '*vault*'`

| Test Suite | Tests | Focus |
|------------|-------|-------|
| `vault_ops.rs` | 23 | Full vault operations |
| `rotation_protocol.rs` | 18 | Rotation flows |
| `access_control.rs` | 15 | Multi-principal scenarios |
| `error_handling.rs` | 16 | Error taxonomy coverage |
| `invariant_enforcement.rs` | 23 | INV-001 through INV-015 |

### 10.3 End-to-End Tests (8 tests)

Run with: `cargo test --test '*e2e*' -- vault`

| Test | Description |
|------|-------------|
| `e2e_api_key_rotation` | Full API key lifecycle with rotation |
| `e2e_password_rotation` | Password credential rotation |
| `e2e_overlap_window` | Verify overlap semantics |
| `e2e_concurrent_rotation` | Concurrent rotation serialization |
| `e2e_revocation` | Full revocation flow |
| `e2e_audit_logging` | Verify audit trail completeness |
| `e2e_failed_rotation_recovery` | Failed rotation handling |
| `e2e_master_key_rotation` | Master key rotation impact |

### 10.4 Proptest Tests (42 tests)

Run with: `cargo test --test '*proptest*' -- vault`

| Invariant | Test Count |
|-----------|------------|
| Encrypted ciphertext | 5 |
| Version chain | 4 |
| Timestamp monotonicity | 3 |
| Active version uniqueness | 3 |
| Failure counter | 3 |
| Overlap window | 3 |
| Version limit | 3 |
| Key version validity | 3 |

### 10.5 Fuzz Tests (3 targets)

Run with: `cargo fuzz run vault_*`

| Target | Corpus |
|--------|--------|
| `vault_credential_id` | `fuzz/corpus/credential_id/*` |
| `vault_secret_decrypt` | `fuzz/corpus/secret_decrypt/*` |
| `vault_trigger_parse` | `fuzz/corpus/trigger_parse/*` |

### 10.6 Kani Verification (2 harnesses)

Run with: `cargo kani --spec <file>`

| Harness | Properties |
|---------|------------|
| `rotation_state` | All transitions valid, no cycles |
| `secret_encryption` | Encrypt/decrypt roundtrip, ciphertext secrecy |

---

## 11. Coverage Targets

| Category | Target |
|----------|--------|
| Line coverage | ≥90% |
| Branch coverage | ≥85% |
| Path coverage | All invariant paths |
| Mutation kill rate | ≥90% |

---

## 12. Dependencies

- **Blockers**: None
- **Dependents**: `ve-8hn6` (TDD Red: Credential vault with rotation)
- **Related**: `ve-yzhf` (Contract: Credential vault with rotation) - COMPLETE

---

## 13. Acceptance Criteria

- [ ] All 87 behaviors have corresponding test cases
- [ ] All 15 invariants (INV-001 through INV-015) are tested
- [ ] Error taxonomy coverage: 100% (all 16 error variants)
- [ ] Vault operations coverage: 100% (all 9 operations)
- [ ] Rotation protocol coverage: 100% (all 4 policy types)
- [ ] BDD scenarios: 20 scenarios covering happy path and error cases
- [ ] Proptest invariants: 28 invariant tests
- [ ] Fuzz targets: 3 targets with corpus
- [ ] Kani harnesses: 2 harnesses verifying critical properties
- [ ] All tests pass on `cargo test`
- [ ] Mutation testing achieves ≥90% kill rate
- [ ] Test plan document matches implementation structure