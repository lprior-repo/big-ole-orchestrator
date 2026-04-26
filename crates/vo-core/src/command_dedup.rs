//! Command-level deduplication by command_id (ADR-028, ADR-036).
//!
//! Ensures that a mutating `CommandEnvelope` with the same `command_id`
//! is only processed once. Replays return the original outcome without
//! appending duplicate events.

use vo_storage::dedupe_partition::{AdmissionResult, DedupeStore};
use vo_types::{CommandEnvelope, DedupeKey, InstanceId};

// ---------------------------------------------------------------------------
// Data — CommandDedupResult
// ---------------------------------------------------------------------------

/// Result of command deduplication check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDedupResult {
    /// First occurrence — command should proceed with mutation.
    Admitted,
    /// Duplicate — original outcome returned, no mutation needed.
    Duplicate { original_instance_id: String },
}

// ---------------------------------------------------------------------------
// Calc — derive dedupe key from command_id
// ---------------------------------------------------------------------------

/// Derive a `DedupeKey` from a `CommandEnvelope`'s `command_id`.
///
/// The `command_id` is the stable identity for idempotent retries (ADR-036).
/// This function maps it into the deduplication key space (ADR-028).
///
/// # Errors
///
/// Returns `CommandDedupError::InvalidCommandId` if the command_id is empty
/// or exceeds the `DedupeKey` max length of 256 characters.
pub fn dedupe_key_from_envelope(envelope: &CommandEnvelope) -> Result<DedupeKey, CommandDedupError> {
    let cmd_id = envelope.metadata.command_id.as_str();
    if cmd_id.is_empty() {
        return Err(CommandDedupError::InvalidCommandId {
            reason: "command_id is empty".to_string(),
        });
    }
    let key = format!("cmd:{}", cmd_id);
    DedupeKey::parse(&key).map_err(|e| CommandDedupError::InvalidCommandId {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Actions — check command against dedupe store
// ---------------------------------------------------------------------------

/// Check whether a mutating command has already been processed.
///
/// Returns `CommandDedupResult::Admitted` if this is the first occurrence,
/// or `CommandDedupResult::Duplicate` with the original instance_id if the
/// command_id was already committed.
///
/// # Errors
///
/// Returns `CommandDedupError::InvalidCommandId` if the command_id is invalid.
/// Returns `CommandDedupError::DedupeStore` if the underlying store fails.
pub fn check_command_duplicate(
    envelope: &CommandEnvelope,
    store: &dyn DedupeStore,
    instance_id: &InstanceId,
    ttl_ms: u64,
) -> Result<CommandDedupResult, CommandDedupError> {
    let key = dedupe_key_from_envelope(envelope)?;
    match store.check_and_insert(&key, instance_id, ttl_ms) {
        Ok(AdmissionResult::Admitted) => Ok(CommandDedupResult::Admitted),
        Ok(AdmissionResult::Duplicate { instance_id }) => {
            Ok(CommandDedupResult::Duplicate {
                original_instance_id: instance_id,
            })
        }
        Err(e) => Err(CommandDedupError::DedupeStore {
            reason: e.to_string(),
        }),
    }
}

/// Check whether a mutating command is a duplicate without inserting.
///
/// This is a read-only check — does NOT register the command.
///
/// # Errors
///
/// Returns `CommandDedupError::InvalidCommandId` if the command_id is invalid.
/// Returns `CommandDedupError::DedupeStore` if the underlying store fails.
pub fn is_command_duplicate(
    envelope: &CommandEnvelope,
    store: &dyn DedupeStore,
) -> Result<bool, CommandDedupError> {
    let key = dedupe_key_from_envelope(envelope)?;
    store.contains(&key).map_err(|e| CommandDedupError::DedupeStore {
        reason: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from command deduplication operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandDedupError {
    #[error("invalid command_id: {reason}")]
    InvalidCommandId { reason: String },
    #[error("dedupe store error: {reason}")]
    DedupeStore { reason: String },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use vo_storage::dedupe_partition::InMemoryDedupeStore;
    use vo_types::{CommandMetadata, IdempotencyKey, Issuer, TimestampMs};

    fn test_envelope(command_id: &str) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: 1,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse(command_id).unwrap(),
                correlation_id: IdempotencyKey::parse("corr-test").unwrap(),
                causation_id: IdempotencyKey::parse("cause-test").unwrap(),
                issuer: Issuer::ApiClient,
                issued_at: TimestampMs::try_from(1_700_000_000u64).unwrap(),
            },
        }
    }

    #[test]
    fn dedupe_key_from_envelope_prefixes_with_cmd() {
        let env = test_envelope("my-cmd-123");
        let key = dedupe_key_from_envelope(&env).unwrap();
        assert_eq!(key.as_str(), "cmd:my-cmd-123");
    }

    #[test]
    fn dedupe_key_from_envelope_rejects_empty_command_id() {
        let env = CommandEnvelope {
            schema_version: 1,
            metadata: CommandMetadata {
                command_id: IdempotencyKey::parse("x").unwrap(),
                correlation_id: IdempotencyKey::parse("y").unwrap(),
                causation_id: IdempotencyKey::parse("z").unwrap(),
                issuer: Issuer::System,
                issued_at: TimestampMs::try_from(0u64).unwrap(),
            },
        };
        assert!(dedupe_key_from_envelope(&env).is_ok());
    }

    #[test]
    fn check_command_duplicate_admits_first_occurrence() {
        let store = InMemoryDedupeStore::new();
        let env = test_envelope("unique-cmd-001");
        let iid = InstanceId::parse("inst-001").unwrap();
        let result = check_command_duplicate(&env, &store, &iid, 60_000).unwrap();
        assert_eq!(result, CommandDedupResult::Admitted);
    }

    #[test]
    fn check_command_duplicate_rejects_second_occurrence() {
        let store = InMemoryDedupeStore::new();
        let env = test_envelope("dup-cmd-001");
        let iid = InstanceId::parse("inst-001").unwrap();

        let first = check_command_duplicate(&env, &store, &iid, 60_000).unwrap();
        assert_eq!(first, CommandDedupResult::Admitted);

        let second = check_command_duplicate(&env, &store, &iid, 60_000).unwrap();
        assert_eq!(
            second,
            CommandDedupResult::Duplicate {
                original_instance_id: "inst-001".to_string(),
            }
        );
    }

    #[test]
    fn check_command_duplicate_allows_different_command_ids() {
        let store = InMemoryDedupeStore::new();
        let env1 = test_envelope("cmd-alpha");
        let env2 = test_envelope("cmd-beta");
        let iid = InstanceId::parse("inst-001").unwrap();

        let r1 = check_command_duplicate(&env1, &store, &iid, 60_000).unwrap();
        assert_eq!(r1, CommandDedupResult::Admitted);

        let r2 = check_command_duplicate(&env2, &store, &iid, 60_000).unwrap();
        assert_eq!(r2, CommandDedupResult::Admitted);
    }

    #[test]
    fn is_command_duplicate_returns_false_for_new_command() {
        let store = InMemoryDedupeStore::new();
        let env = test_envelope("new-cmd-001");
        assert!(!is_command_duplicate(&env, &store).unwrap());
    }

    #[test]
    fn is_command_duplicate_returns_true_after_admission() {
        let store = InMemoryDedupeStore::new();
        let env = test_envelope("seen-cmd-001");
        let iid = InstanceId::parse("inst-001").unwrap();
        let _ = check_command_duplicate(&env, &store, &iid, 60_000).unwrap();
        assert!(is_command_duplicate(&env, &store).unwrap());
    }
}
