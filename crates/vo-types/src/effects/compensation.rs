//! Compensation policy types for managed effects (ADR-030 §5, ADR-034).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CompensationPolicy {
    None,
    Manual,
    Automatic,
}

impl CompensationPolicy {
    #[must_use]
    pub const fn all_variants() -> &'static [CompensationPolicy] {
        &[
            CompensationPolicy::None,
            CompensationPolicy::Manual,
            CompensationPolicy::Automatic,
        ]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn compensationpolicy_debug_format_equals_variant_name() {
        assert_eq!(format!("{:?}", CompensationPolicy::None), "None");
        assert_eq!(format!("{:?}", CompensationPolicy::Manual), "Manual");
        assert_eq!(format!("{:?}", CompensationPolicy::Automatic), "Automatic");
    }

    #[rstest]
    #[case(CompensationPolicy::None, "None")]
    #[case(CompensationPolicy::Manual, "Manual")]
    #[case(CompensationPolicy::Automatic, "Automatic")]
    fn compensationpolicy_serializes_and_deserializes_for_all_variants(
        #[case] variant: CompensationPolicy,
        #[case] expected_json: &str,
    ) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected_json}\""));
        let recovered: CompensationPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, variant);
    }

    #[test]
    fn compensationpolicy_all_variants_returns_three_variants_in_declaration_order() {
        let variants = CompensationPolicy::all_variants();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0], CompensationPolicy::None);
        assert_eq!(variants[1], CompensationPolicy::Manual);
        assert_eq!(variants[2], CompensationPolicy::Automatic);
    }
}
