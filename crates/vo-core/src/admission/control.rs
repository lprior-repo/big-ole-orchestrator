//! Admission control types and trait for ADR-028 deduplication and ADR-029 fencing.
//!
//! This module defines:
//! - `AdmissionResult` — exhaustive enum of admission outcomes
//! - `DedupeToken` — opaque token issued on admission
//! - `RejectionReason` — why a command was rejected
//! - `AdmissionCheck` — pure trait for dedupe and fence checks
//!
//! # Purity Contract
//!
//! All types are pure data. No I/O, no async, no side effects.
//! The trait is pure — implementations provide persistence-backed logic.

use std::fmt;

use serde::{Deserialize, Serialize};

use vo_types::{DedupeKey, FenceToken, InstanceId, StepId};

// ============================================================================
// DedupeToken
// ============================================================================

/// Opaque token issued on successful admission for exactly-once tracking.
///
/// Non-empty by construction. Used to correlate the admission decision
/// with downstream processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DedupeToken(String);

impl DedupeToken {
    /// Create a new `DedupeToken`.
    ///
    /// # Panics
    ///
    /// Panics if `value` is empty. Use `DedupeToken::try_new` for a fallible constructor.
    #[must_use]
    pub fn new(value: String) -> Self {
        assert!(!value.is_empty(), "DedupeToken must be non-empty");
        Self(value)
    }

    /// Try to create a new `DedupeToken`.
    ///
    /// # Errors
    ///
    /// Returns `DedupeTokenError::Empty` if `value` is empty.
    pub fn try_new(value: String) -> Result<Self, DedupeTokenError> {
        if value.is_empty() {
            return Err(DedupeTokenError::Empty);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DedupeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors for `DedupeToken` construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupeTokenError {
    /// Token value is empty.
    Empty,
}

impl fmt::Display for DedupeTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DedupeTokenError::Empty => write!(f, "DedupeToken must be non-empty"),
        }
    }
}

impl std::error::Error for DedupeTokenError {}

// ============================================================================
// RejectionReason
// ============================================================================

/// Exhaustive enum of why a command was rejected during admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    /// The dedupe key is missing for an exact-workflow ingress surface.
    MissingDedupeKey,
    /// The dedupe key exceeds the maximum allowed length.
    DedupeKeyTooLong {
        max_length: usize,
        actual_length: usize,
    },
    /// The fence token does not match the current lease (stale execution attempt).
    FenceTokenMismatch {
        expected: FenceToken,
        actual: FenceToken,
    },
    /// No active lease exists for the (instance_id, step_id) pair.
    NoActiveLease,
    /// A generic admission policy violation with a human-readable message.
    PolicyViolation(String),
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectionReason::MissingDedupeKey => write!(f, "missing dedupe key"),
            RejectionReason::DedupeKeyTooLong {
                max_length,
                actual_length,
            } => {
                write!(
                    f,
                    "dedupe key too long: {actual_length} characters exceeds maximum of {max_length}"
                )
            }
            RejectionReason::FenceTokenMismatch { expected, actual } => {
                write!(f, "fence token mismatch: expected {expected}, got {actual}")
            }
            RejectionReason::NoActiveLease => {
                write!(f, "no active lease for the given instance and step")
            }
            RejectionReason::PolicyViolation(msg) => write!(f, "policy violation: {msg}"),
        }
    }
}

impl std::error::Error for RejectionReason {}

// ============================================================================
// AdmissionResult
// ============================================================================

/// Exhaustive enum of admission check outcomes.
///
/// Every admission check returns exactly one variant. No panics, no `Option`, no `unwrap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdmissionResult {
    /// Command is admitted; carries the dedupe token for idempotent tracking.
    Admitted { dedupe_token: DedupeToken },
    /// Command is rejected due to a policy violation or precondition failure.
    Rejected { reason: RejectionReason },
    /// Command is a duplicate of an already-admitted command.
    Duplicate { original_instance_id: InstanceId },
}

// ============================================================================
// AdmissionCheck Trait
// ============================================================================

/// Pure trait for admission control with deduplication and fencing.
///
/// Implementations provide persistence-backed logic. The trait itself
/// is pure — no I/O, no async, no side effects.
///
/// # Design (ADR-028, ADR-029)
///
/// - `check_deduplicate`: Ensures exactly-once ingress by rejecting duplicates.
/// - `check_fence`: Ensures only the current lease holder can commit.
pub trait AdmissionCheck {
    /// Check deduplication for an incoming command.
    ///
    /// Returns `AdmissionResult::Admitted` if the command is new,
    /// `AdmissionResult::Duplicate` if a prior command with the same dedupe key
    /// was already admitted, or `AdmissionResult::Rejected` if the dedupe key
    /// is invalid.
    fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult;

    /// Check fencing for a step execution attempt.
    ///
    /// Returns `AdmissionResult::Admitted` if the fence token matches the
    /// current lease, `AdmissionResult::Rejected` if the token is stale or
    /// no lease exists.
    fn check_fence(
        &self,
        instance_id: &InstanceId,
        step_id: &StepId,
        fence_token: &FenceToken,
    ) -> AdmissionResult;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Helper constructors for test data.
    // InstanceId field is pub(crate) in vo-types; use from_bytes or parse.
    // StepId field is pub(crate); use parse with valid identifiers.
    fn make_instance_id(label: &str) -> InstanceId {
        // Deterministic bytes from label: hash label into 16 bytes.
        // Simple approach: use label bytes to seed a 16-byte array.
        let b = label.as_bytes();
        let mut bytes = [0u8; 16];
        for (i, &byte) in b.iter().enumerate() {
            bytes[i % 16] = bytes[i % 16].wrapping_add(byte);
        }
        // Ensure non-zero (ULID nil is rejected)
        if bytes.iter().all(|&b| b == 0) {
            bytes[0] = 1;
        }
        InstanceId::from_bytes(bytes)
    }

    fn make_step_id(s: &str) -> StepId {
        // StepId::parse requires valid identifier chars, no leading underscore.
        // Sanitize the input to be a valid identifier.
        let sanitized: String = s
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let cleaned = sanitized.strip_prefix('-').unwrap_or(&sanitized);
        let cleaned = cleaned.strip_suffix('-').unwrap_or(cleaned);
        StepId::parse(cleaned).unwrap_or_else(|_| StepId::parse("step-id").expect("fallback"))
    }

    fn make_fence_token(v: u64) -> FenceToken {
        FenceToken::new(v).expect("fence token must be nonzero")
    }

    fn make_dedupe_key(s: &str) -> DedupeKey {
        DedupeKey::parse(s).expect("valid dedupe key for test")
    }

    // ========================================================================
    // DedupeToken Tests
    // ========================================================================

    #[test]
    fn dedupe_token_new_creates_token_with_valid_value() {
        let token = DedupeToken::new("tok-abc-123".to_string());
        assert_eq!(token.as_str(), "tok-abc-123");
    }

    #[test]
    fn dedupe_token_try_new_succeeds_for_non_empty() {
        let token = DedupeToken::try_new("tok-xyz".to_string());
        assert_eq!(
            token.map(|t| t.as_str().to_string()),
            Ok("tok-xyz".to_string())
        );
    }

    #[test]
    fn dedupe_token_try_new_fails_for_empty() {
        let result = DedupeToken::try_new(String::new());
        assert_eq!(result, Err(DedupeTokenError::Empty));
    }

    #[test]
    fn dedupe_token_display_returns_inner_string() {
        let token = DedupeToken::new("display-test".to_string());
        assert_eq!(format!("{token}"), "display-test");
    }

    #[test]
    fn dedupe_token_clone_equality() {
        let token = DedupeToken::new("clone-test".to_string());
        assert_eq!(token.clone(), token);
    }

    #[test]
    fn dedupe_token_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let t1 = DedupeToken::new("hash-test".to_string());
        let t2 = DedupeToken::new("hash-test".to_string());

        let mut h1 = DefaultHasher::new();
        t1.hash(&mut h1);
        let hash1 = h1.finish();

        let mut h2 = DefaultHasher::new();
        t2.hash(&mut h2);
        let hash2 = h2.finish();

        assert_eq!(hash1, hash2, "Equal tokens must have equal hashes");
    }

    // ========================================================================
    // RejectionReason Tests
    // ========================================================================

    #[test]
    fn rejection_reason_missing_dedupe_key() {
        let reason = RejectionReason::MissingDedupeKey;
        assert_eq!(reason, RejectionReason::MissingDedupeKey);
        assert_eq!(format!("{reason}"), "missing dedupe key");
    }

    #[test]
    fn rejection_reason_dedupe_key_too_long() {
        let reason = RejectionReason::DedupeKeyTooLong {
            max_length: 256,
            actual_length: 300,
        };
        assert_eq!(
            reason,
            RejectionReason::DedupeKeyTooLong {
                max_length: 256,
                actual_length: 300,
            }
        );
        let display = format!("{reason}");
        assert!(display.contains("300"));
        assert!(display.contains("256"));
    }

    #[test]
    fn rejection_reason_fence_token_mismatch() {
        let expected = make_fence_token(5);
        let actual = make_fence_token(3);
        let reason = RejectionReason::FenceTokenMismatch { expected, actual };
        assert_eq!(
            reason,
            RejectionReason::FenceTokenMismatch {
                expected: make_fence_token(5),
                actual: make_fence_token(3),
            }
        );
        let display = format!("{reason}");
        assert!(display.contains("5"));
        assert!(display.contains("3"));
    }

    #[test]
    fn rejection_reason_no_active_lease() {
        let reason = RejectionReason::NoActiveLease;
        assert_eq!(reason, RejectionReason::NoActiveLease);
        assert!(format!("{reason}").contains("no active lease"));
    }

    #[test]
    fn rejection_reason_policy_violation() {
        let reason = RejectionReason::PolicyViolation("rate limit exceeded".to_string());
        assert_eq!(
            reason,
            RejectionReason::PolicyViolation("rate limit exceeded".to_string())
        );
        let display = format!("{reason}");
        assert!(display.contains("rate limit exceeded"));
    }

    #[test]
    fn rejection_reason_implements_error() {
        let reason = RejectionReason::MissingDedupeKey;
        let _: &dyn std::error::Error = &reason;
    }

    // ========================================================================
    // AdmissionResult Tests
    // ========================================================================

    #[test]
    fn admission_result_admitted_constructs_correctly() {
        let token = DedupeToken::new("tok-admitted".to_string());
        let result = AdmissionResult::Admitted {
            dedupe_token: token.clone(),
        };
        match result {
            AdmissionResult::Admitted { dedupe_token } => {
                assert_eq!(dedupe_token.as_str(), "tok-admitted");
            }
            _ => panic!("Expected Admitted variant"),
        }
    }

    #[test]
    fn admission_result_rejected_constructs_correctly() {
        let result = AdmissionResult::Rejected {
            reason: RejectionReason::MissingDedupeKey,
        };
        match result {
            AdmissionResult::Rejected { reason } => {
                assert_eq!(reason, RejectionReason::MissingDedupeKey);
            }
            _ => panic!("Expected Rejected variant"),
        }
    }

    #[test]
    fn admission_result_duplicate_constructs_correctly() {
        let instance_id = make_instance_id("duplicate-test-id");
        let result = AdmissionResult::Duplicate {
            original_instance_id: instance_id.clone(),
        };
        match result {
            AdmissionResult::Duplicate {
                original_instance_id,
            } => {
                assert_eq!(original_instance_id, instance_id);
            }
            _ => panic!("Expected Duplicate variant"),
        }
    }

    #[test]
    fn admission_result_clone_and_equality() {
        let result1 = AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("eq-test".to_string()),
        };
        let result2 = result1.clone();
        assert_eq!(result1, result2);
    }

    #[test]
    fn admission_result_inequality_between_variants() {
        let admitted = AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("tok-a".to_string()),
        };
        let rejected = AdmissionResult::Rejected {
            reason: RejectionReason::MissingDedupeKey,
        };
        assert_ne!(admitted, rejected);
    }

    // ========================================================================
    // Mock AdmissionCheck for BDD Scenarios
    // ========================================================================

    /// Mock implementation for testing the trait contract.
    /// Uses typed keys (InstanceId, StepId) to avoid string mismatch with ULIDs.
    struct MockAdmissionCheck {
        seen_keys: HashMap<String, InstanceId>,
        leases: HashMap<(InstanceId, StepId), FenceToken>,
    }

    impl MockAdmissionCheck {
        fn new() -> Self {
            Self {
                seen_keys: HashMap::new(),
                leases: HashMap::new(),
            }
        }

        fn with_seen_key(mut self, key: &str, instance_id: InstanceId) -> Self {
            self.seen_keys.insert(key.to_string(), instance_id);
            self
        }

        fn with_lease(
            mut self,
            instance_id: InstanceId,
            step_id: StepId,
            token: FenceToken,
        ) -> Self {
            self.leases.insert((instance_id, step_id), token);
            self
        }
    }

    impl AdmissionCheck for MockAdmissionCheck {
        fn check_deduplicate(&self, dedupe_key: &DedupeKey) -> AdmissionResult {
            if let Some(original_id) = self.seen_keys.get(dedupe_key.as_str()) {
                return AdmissionResult::Duplicate {
                    original_instance_id: original_id.clone(),
                };
            }
            AdmissionResult::Admitted {
                dedupe_token: DedupeToken::new(format!("token-{}", dedupe_key.as_str())),
            }
        }

        fn check_fence(
            &self,
            instance_id: &InstanceId,
            step_id: &StepId,
            fence_token: &FenceToken,
        ) -> AdmissionResult {
            let key = (instance_id.clone(), step_id.clone());
            if let Some(expected_token) = self.leases.get(&key) {
                if expected_token == fence_token {
                    return AdmissionResult::Admitted {
                        dedupe_token: DedupeToken::new(format!("fence-ok",)),
                    };
                }
                return AdmissionResult::Rejected {
                    reason: RejectionReason::FenceTokenMismatch {
                        expected: expected_token.clone(),
                        actual: fence_token.clone(),
                    },
                };
            }
            AdmissionResult::Rejected {
                reason: RejectionReason::NoActiveLease,
            }
        }
    }

    // ========================================================================
    // BDD SCENARIO 4: check_deduplicate returns Admitted for new key
    // ========================================================================

    #[test]
    fn check_deduplicate_returns_admitted_for_new_key() {
        let check = MockAdmissionCheck::new();
        let key = make_dedupe_key("unique-key-123");
        let result = check.check_deduplicate(&key);

        match result {
            AdmissionResult::Admitted { dedupe_token } => {
                assert!(!dedupe_token.as_str().is_empty());
            }
            _ => panic!("Expected Admitted for new key, got {:?}", result),
        }
    }

    // ========================================================================
    // BDD SCENARIO 5: check_deduplicate returns Duplicate for seen key
    // ========================================================================

    #[test]
    fn check_deduplicate_returns_duplicate_for_seen_key() {
        let original_id = make_instance_id("original-instance");
        let check = MockAdmissionCheck::new().with_seen_key("key-1", original_id.clone());
        let key = make_dedupe_key("key-1");
        let result = check.check_deduplicate(&key);

        match result {
            AdmissionResult::Duplicate {
                original_instance_id,
            } => {
                assert_eq!(original_instance_id, original_id);
            }
            _ => panic!("Expected Duplicate for seen key, got {:?}", result),
        }
    }

    // ========================================================================
    // BDD SCENARIO 6: check_fence returns Admitted when token matches
    // ========================================================================

    #[test]
    fn check_fence_returns_admitted_when_token_matches() {
        let inst = make_instance_id("inst-1");
        let step = make_step_id("step-1");
        let check =
            MockAdmissionCheck::new().with_lease(inst.clone(), step.clone(), make_fence_token(5));
        let result = check.check_fence(&inst, &step, &make_fence_token(5));

        match result {
            AdmissionResult::Admitted { dedupe_token } => {
                assert!(!dedupe_token.as_str().is_empty());
            }
            _ => panic!("Expected Admitted for matching token, got {:?}", result),
        }
    }

    // ========================================================================
    // BDD SCENARIO 7: check_fence returns Rejected on stale token
    // ========================================================================

    #[test]
    fn check_fence_returns_rejected_on_stale_token() {
        let inst = make_instance_id("inst-1");
        let step = make_step_id("step-1");
        let check =
            MockAdmissionCheck::new().with_lease(inst.clone(), step.clone(), make_fence_token(5));
        let result = check.check_fence(&inst, &step, &make_fence_token(3));

        match result {
            AdmissionResult::Rejected { reason } => {
                assert!(matches!(reason, RejectionReason::FenceTokenMismatch { .. }));
            }
            _ => panic!("Expected Rejected for stale token, got {:?}", result),
        }
    }

    // ========================================================================
    // BDD SCENARIO 8: check_fence returns Rejected when no lease
    // ========================================================================

    #[test]
    fn check_fence_returns_rejected_when_no_lease() {
        let check = MockAdmissionCheck::new();
        let result = check.check_fence(
            &make_instance_id("inst-2"),
            &make_step_id("step-1"),
            &make_fence_token(1),
        );

        match result {
            AdmissionResult::Rejected { reason } => {
                assert_eq!(reason, RejectionReason::NoActiveLease);
            }
            _ => panic!("Expected Rejected for missing lease, got {:?}", result),
        }
    }

    // ========================================================================
    // Exhaustiveness Tests
    // ========================================================================

    #[test]
    fn admission_result_is_exhaustive_three_variants() {
        // This test documents that AdmissionResult has exactly 3 variants.
        // If a variant is added or removed, this test must be updated.
        let admitted = AdmissionResult::Admitted {
            dedupe_token: DedupeToken::new("exhaustive".to_string()),
        };
        let rejected = AdmissionResult::Rejected {
            reason: RejectionReason::MissingDedupeKey,
        };
        let duplicate = AdmissionResult::Duplicate {
            original_instance_id: make_instance_id("01JMX"),
        };

        // Verify each is distinct
        assert_ne!(admitted, rejected);
        assert_ne!(admitted, duplicate);
        assert_ne!(rejected, duplicate);

        // Verify match is exhaustive
        let all_variants_match = |r: &AdmissionResult| match r {
            AdmissionResult::Admitted { .. } => 1,
            AdmissionResult::Rejected { .. } => 2,
            AdmissionResult::Duplicate { .. } => 3,
        };
        assert_eq!(all_variants_match(&admitted), 1);
        assert_eq!(all_variants_match(&rejected), 2);
        assert_eq!(all_variants_match(&duplicate), 3);
    }

    #[test]
    fn rejection_reason_is_exhaustive_five_variants() {
        let reasons = [
            RejectionReason::MissingDedupeKey,
            RejectionReason::DedupeKeyTooLong {
                max_length: 256,
                actual_length: 300,
            },
            RejectionReason::FenceTokenMismatch {
                expected: make_fence_token(5),
                actual: make_fence_token(3),
            },
            RejectionReason::NoActiveLease,
            RejectionReason::PolicyViolation("test".to_string()),
        ];
        assert_eq!(reasons.len(), 5);

        // Verify exhaustive match
        let classify = |r: &RejectionReason| match r {
            RejectionReason::MissingDedupeKey => 1,
            RejectionReason::DedupeKeyTooLong { .. } => 2,
            RejectionReason::FenceTokenMismatch { .. } => 3,
            RejectionReason::NoActiveLease => 4,
            RejectionReason::PolicyViolation(_) => 5,
        };
        for (i, reason) in reasons.iter().enumerate() {
            assert_eq!(classify(reason), i + 1);
        }
    }

    // ========================================================================
    // Kani Verification Harnesses
    // ========================================================================

    #[cfg(kani)]
    mod verification {
        use super::*;

        /// Kani proof: DedupeToken::try_new never panics for any String input.
        #[kani::proof]
        fn dedupe_token_try_new_never_panics() {
            let value: String = kani::any();
            let _ = DedupeToken::try_new(value);
        }

        /// Kani proof: DedupeToken::try_new returns Empty for empty string.
        #[kani::proof]
        fn dedupe_token_try_new_empty_returns_error() {
            let result = DedupeToken::try_new(String::new());
            match result {
                Err(DedupeTokenError::Empty) => {}
                _ => kani::assert(false, "Empty string must return Empty error"),
            }
        }
    }
}
