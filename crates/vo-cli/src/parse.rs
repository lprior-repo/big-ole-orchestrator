use crate::cli::CliError;

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
