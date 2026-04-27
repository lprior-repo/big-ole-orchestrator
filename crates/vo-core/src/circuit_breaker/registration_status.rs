//! Registration status enum for workflow lifecycle.
//!
//! Re-exports the canonical [`RegistrationStatus`] type from `vo-types`.
//! This type represents the four possible states a workflow can be in during
//! its lifecycle, as defined in [ADR-021].
//!
//! # Lifecycle
//!
//! ```text
//!  ┌──────────┐    register    ┌──────────┐    fail N times    ┌─────────────┐
//! │  Active   │───────────────>│  Active  │───────────────────>│ Quarantined │
//! │  (default)│                │          │                    │             │
//!  └──────────┘                └──────────┘                    └──────┬──────┘
//!       │                                                              │
//!       │              unquarantine()                                  │
//!       │──────────────────────────────────────────────────────────────┘
//!
//!  ┌──────────────┐    operator removes    ┌──────────┐
//! │  Deactivated  │───────────────────────>│ Deleted   │
//! │               │                        └──────────┘
//! ```
//!
//! # Variants
//!
//! | Variant | Meaning | Registrations Allowed? |
//! |---------|---------|----------------------|
//! | `Active` | Normal, healthy workflow | Yes (subject to rate limiting) |
//! | `Quarantined` | Automatically blocked due to repeated failures | No |
//! | `Deactivated` | Manually disabled by operator | No |
//! | `Deleted` | Removed by operator | No |
//!
//! # See Also
//!
//! - [ADR-021] — Workflow registration status architecture
//! - [`crate::circuit_breaker::evaluate_registration`] — Uses status for registration gate
//! - [`crate::circuit_breaker::unquarantine`] — Restores `Quarantined` → `Active`

pub use vo_types::RegistrationStatus;

#[cfg(test)]
mod tests {
    use super::*;

    // B-39: RegistrationStatus has exactly 4 variants (ADR-021 added Deleted)
    #[test]
    fn registration_status_has_exactly_four_variants() {
        let statuses = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
            RegistrationStatus::Deleted,
        ];
        assert_eq!(statuses.len(), 4);
        statuses.iter().for_each(|s| match s {
            RegistrationStatus::Active => {}
            RegistrationStatus::Deactivated => {}
            RegistrationStatus::Quarantined => {}
            RegistrationStatus::Deleted => {}
        });
    }

    // B-40: Serde round-trip for all variants
    #[test]
    fn registration_status_serde_round_trips_for_all_variants() {
        let variants = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
            RegistrationStatus::Deleted,
        ];
        variants.iter().for_each(|original| {
            let json = serde_json::to_string(original).expect("serialize");
            let restored: RegistrationStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*original, restored);
        });
    }

    // B-40 extended: specific round-trip for Quarantined
    #[test]
    fn registration_status_quarantined_serde_round_trip() {
        let original = RegistrationStatus::Quarantined;
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: RegistrationStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, RegistrationStatus::Quarantined);
    }

    // PROP-08: RegistrationStatus serde round-trip (exhaustive, not proptest needed)
    #[test]
    fn registration_status_serde_round_trip_is_identity_for_active() {
        let original = RegistrationStatus::Active;
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: RegistrationStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn registration_status_serde_round_trip_is_identity_for_deactivated() {
        let original = RegistrationStatus::Deactivated;
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: RegistrationStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, original);
    }
}
