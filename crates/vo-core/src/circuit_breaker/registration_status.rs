//! Registration status enum for workflow lifecycle.
//!
//! Re-exports the canonical type from `vo-types`.

pub use vo_types::RegistrationStatus;

#[cfg(test)]
mod tests {
    use super::*;

    // B-39: RegistrationStatus has exactly 3 variants
    #[test]
    fn registration_status_has_exactly_three_variants() {
        // Compile-time exhaustiveness check via match.
        // If a 4th variant is added, this match will fail to compile.
        let statuses = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
        ];
        assert_eq!(statuses.len(), 3);
        // Verify each variant matches exhaustively
        statuses.iter().for_each(|s| match s {
            RegistrationStatus::Active => {}
            RegistrationStatus::Deactivated => {}
            RegistrationStatus::Quarantined => {}
        });
    }

    // B-40: Serde round-trip for all variants
    #[test]
    fn registration_status_serde_round_trips_for_all_variants() {
        let variants = [
            RegistrationStatus::Active,
            RegistrationStatus::Deactivated,
            RegistrationStatus::Quarantined,
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
