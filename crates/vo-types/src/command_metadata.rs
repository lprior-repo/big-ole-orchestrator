#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Issuer {
    System,
    ApiClient,
    Operator,
    AiAgent,
    TimerLoop,
    RecoveryLoop,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct CommandMetadata {
    pub command_id: crate::IdempotencyKey,
    pub correlation_id: crate::IdempotencyKey,
    pub causation_id: crate::IdempotencyKey,
    pub issuer: Issuer,
    pub issued_at: crate::TimestampMs,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_system_serializes_as_snake_case() {
        let json = serde_json::to_string(&Issuer::System).unwrap();
        assert_eq!(json, "\"system\"");
    }

    #[test]
    fn issuer_api_client_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&Issuer::ApiClient).unwrap(),
            "\"api_client\""
        );
    }

    #[test]
    fn issuer_api_client_round_trips_through_serde() {
        let round_tripped: Issuer = serde_json::from_str("\"api_client\"").unwrap();
        assert_eq!(round_tripped, Issuer::ApiClient);
    }

    #[test]
    fn command_metadata_can_be_constructed() {
        let _cmd = CommandMetadata {
            command_id: crate::IdempotencyKey::parse("cmd-001").unwrap(),
            correlation_id: crate::IdempotencyKey::parse("corr-001").unwrap(),
            causation_id: crate::IdempotencyKey::parse("cause-001").unwrap(),
            issuer: Issuer::System,
            issued_at: crate::TimestampMs::try_from(1_700_000_000u64).unwrap(),
        };
    }

    #[test]
    fn command_metadata_has_issuer_field() {
        let cmd = CommandMetadata {
            command_id: crate::IdempotencyKey::parse("cmd-001").unwrap(),
            correlation_id: crate::IdempotencyKey::parse("corr-001").unwrap(),
            causation_id: crate::IdempotencyKey::parse("cause-001").unwrap(),
            issuer: Issuer::Operator,
            issued_at: crate::TimestampMs::try_from(1_700_000_000u64).unwrap(),
        };
        assert_eq!(cmd.issuer, Issuer::Operator);
    }

    #[test]
    fn command_metadata_serde_round_trips() {
        let original = CommandMetadata {
            command_id: crate::IdempotencyKey::parse("cmd-abc").unwrap(),
            correlation_id: crate::IdempotencyKey::parse("corr-xyz").unwrap(),
            causation_id: crate::IdempotencyKey::parse("cause-123").unwrap(),
            issuer: Issuer::AiAgent,
            issued_at: crate::TimestampMs::try_from(1_700_000_000u64).unwrap(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let recovered: CommandMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered, original);
    }

    #[test]
    fn issuer_rejects_unknown_variant() {
        let result: Result<Issuer, serde_json::Error> =
            serde_json::from_str("\"totally_bogus\"");
        let err = result.expect_err("should reject unknown variant");
        assert!(err.is_data(), "expected data error, got: {:?}", err);
    }

    #[test]
    fn command_metadata_exported_from_crate_root() {
        let _cmd: crate::CommandMetadata = CommandMetadata {
            command_id: crate::IdempotencyKey::parse("x").unwrap(),
            correlation_id: crate::IdempotencyKey::parse("y").unwrap(),
            causation_id: crate::IdempotencyKey::parse("z").unwrap(),
            issuer: crate::Issuer::TimerLoop,
            issued_at: crate::TimestampMs::try_from(0u64).unwrap(),
        };
    }
}
