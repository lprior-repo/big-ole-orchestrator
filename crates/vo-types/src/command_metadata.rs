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
        let round_tripped: Issuer =
            serde_json::from_str("\"api_client\"").unwrap();
        assert_eq!(round_tripped, Issuer::ApiClient);
    }
}
