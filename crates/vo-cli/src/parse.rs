use crate::cli::{CliError, NatsUrl};

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

/// Parse and validate a NATS URL, rejecting invalid ports and empty hosts.
///
/// # Errors
/// Returns `CliError::InvalidNatsUrl` if the scheme is unsupported, the host is empty,
/// or the port is missing or out of bounds (e.g. `0` or `65536`).
pub fn parse_nats_url(s: &str) -> Result<NatsUrl, CliError> {
    if s.is_empty() {
        return Err(CliError::InvalidNatsUrl("empty host".to_string()));
    }

    if s.contains("://") {
        return Err(CliError::InvalidNatsUrl("scheme not allowed".to_string()));
    }

    let (host_str, port_str) = match s.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (s, None),
    };

    if host_str.is_empty() {
        return Err(CliError::InvalidNatsUrl("empty host".to_string()));
    }

    let port = match port_str {
        Some(p) => {
            let p_num = p
                .parse::<u16>()
                .map_err(|_| CliError::InvalidNatsUrl("port out of bounds".to_string()))?;
            if p_num == 0 {
                return Err(CliError::InvalidNatsUrl("port out of bounds".to_string()));
            }
            Some(p_num)
        }
        None => None,
    };

    Ok(NatsUrl {
        host: host_str.to_string(),
        port,
    })
}
