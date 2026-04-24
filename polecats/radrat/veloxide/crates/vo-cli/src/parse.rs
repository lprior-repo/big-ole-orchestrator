use crate::cli::CliError;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("0", 0)]
    #[case("1", 1)]
    #[case("42", 42)]
    #[case("123456789", 123456789)]
    #[case("18446744073709551615", 18446744073709551615_u64)]
    fn valid_integers(#[case] input: &str, #[case] expected: u64) {
        assert_eq!(parse_strict_numeric(input).unwrap(), expected);
    }

    #[rstest]
    #[case("-1")]
    #[case("-42")]
    fn negative_numbers_rejected(#[case] input: &str) {
        assert!(parse_strict_numeric(input).is_err());
        let err = parse_strict_numeric(input).unwrap_err();
        assert!(err.to_string().contains("negative"));
    }

    #[rstest]
    #[case("+1")]
    #[case("+42")]
    fn leading_plus_rejected(#[case] input: &str) {
        assert!(parse_strict_numeric(input).is_err());
        let err = parse_strict_numeric(input).unwrap_err();
        assert!(err.to_string().contains("leading plus"));
    }

    #[test]
    fn empty_string_rejected() {
        let result = parse_strict_numeric("");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[rstest]
    #[case("abc")]
    #[case("12a")]
    #[case("1.0")]
    #[case("1e10")]
    #[case("1_000")]
    fn invalid_digits_rejected(#[case] input: &str) {
        assert!(parse_strict_numeric(input).is_err());
    }

    #[test]
    fn overflow_rejected() {
        let result = parse_strict_numeric("18446744073709551616");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("overflow"));
    }

    #[test]
    fn max_u64_is_valid() {
        assert_eq!(
            parse_strict_numeric("18446744073709551615").unwrap(),
            18446744073709551615_u64
        );
    }
}

/// Parse a numeric token, strictly rejecting leading `+` signs.
///
/// # Errors
/// Returns `CliError::InvalidNumeric` if the string is empty, starts with `+` or `-`,
/// or cannot be parsed as a valid `u64`.
pub fn parse_strict_numeric(s: &str) -> Result<u64, CliError> {
    if s.is_empty() {
        return Err(CliError::InvalidNumeric("empty string".to_string()));
    }
    if s.starts_with('+') {
        return Err(CliError::InvalidNumeric(
            "leading plus sign not allowed".to_string(),
        ));
    }
    if s.starts_with('-') {
        return Err(CliError::InvalidNumeric(
            "negative value not allowed".to_string(),
        ));
    }

    s.parse::<u64>().map_err(|e| match e.kind() {
        std::num::IntErrorKind::PosOverflow => {
            CliError::InvalidNumeric("numeric value overflowed u64".to_string())
        }
        _ => CliError::InvalidNumeric("invalid digits".to_string()),
    })
}
