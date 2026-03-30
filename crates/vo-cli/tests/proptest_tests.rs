use proptest::prelude::*;
use vo_cli::{parse_nats_url, parse_strict_numeric, CliError};

proptest! {
    #[test]
    fn parse_strict_numeric_rejects_non_digits(s in ".*[^0-9].*") {
        prop_assert!(matches!(parse_strict_numeric(&s), Err(CliError::InvalidNumeric(_))));
    }

    #[test]
    fn parse_nats_url_accepts_valid_urls(host in "[a-z]+", port in 1..=65535u16) {
        let url = format!("{}:{}", host, port);
        prop_assert!(parse_nats_url(&url).is_ok());
    }
}
