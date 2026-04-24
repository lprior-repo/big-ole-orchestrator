use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ParseError;

const MAX_NAME_LEN: usize = 64;
const TYPE_NAME: &str = "WorkspaceName";

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkspaceName(String);

impl WorkspaceName {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        if input.is_empty() {
            return Err(ParseError::Empty {
                type_name: TYPE_NAME,
            });
        }
        if input.len() > MAX_NAME_LEN {
            return Err(ParseError::ExceedsMaxLength {
                type_name: TYPE_NAME,
                max: MAX_NAME_LEN,
                actual: input.len(),
            });
        }
        if !input
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            let invalid: String = input
                .chars()
                .filter(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && *c != '-')
                .collect();
            return Err(ParseError::InvalidCharacters {
                type_name: TYPE_NAME,
                invalid_chars: invalid,
            });
        }
        if input.starts_with('-') {
            return Err(ParseError::BoundaryViolation {
                type_name: TYPE_NAME,
                reason: "must not start with hyphen".to_string(),
            });
        }
        if input.ends_with('-') {
            return Err(ParseError::BoundaryViolation {
                type_name: TYPE_NAME,
                reason: "must not end with hyphen".to_string(),
            });
        }
        if input.contains("--") {
            return Err(ParseError::ConsecutiveHyphens {
                type_name: TYPE_NAME,
            });
        }
        Ok(Self(input.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl TryFrom<String> for WorkspaceName {
    type Error = ParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<WorkspaceName> for String {
    fn from(value: WorkspaceName) -> String {
        value.0
    }
}

impl fmt::Display for WorkspaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for WorkspaceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tn_001_valid_lowercase_alphanumeric_name() {
        let name = WorkspaceName::parse("workspace").unwrap();
        assert_eq!(name.as_str(), "workspace");
    }

    #[test]
    fn tn_002_valid_hyphenated_name() {
        let name = WorkspaceName::parse("my-workspace-42").unwrap();
        assert_eq!(name.as_str(), "my-workspace-42");
    }

    #[test]
    fn tn_003_reject_uppercase() {
        let result = WorkspaceName::parse("Workspace");
        assert!(matches!(result, Err(ParseError::InvalidCharacters { .. })));
    }

    #[test]
    fn tn_004_reject_empty_string() {
        let result = WorkspaceName::parse("");
        assert!(matches!(result, Err(ParseError::Empty { .. })));
    }

    #[test]
    fn tn_005_reject_spaces() {
        let result = WorkspaceName::parse("my workspace");
        assert!(matches!(result, Err(ParseError::InvalidCharacters { .. })));
    }

    #[test]
    fn tn_006_reject_special_chars() {
        let result = WorkspaceName::parse("my@workspace");
        assert!(matches!(result, Err(ParseError::InvalidCharacters { .. })));
    }

    #[test]
    fn tn_007_reject_name_exceeding_64_bytes() {
        let long = "a".repeat(65);
        let result = WorkspaceName::parse(&long);
        assert!(matches!(
            result,
            Err(ParseError::ExceedsMaxLength { max: 64, .. })
        ));
    }

    #[test]
    fn tn_008_accept_name_at_exactly_64_bytes() {
        let exact = "a".repeat(64);
        let name = WorkspaceName::parse(&exact).unwrap();
        assert_eq!(name.as_str().len(), 64);
    }

    #[test]
    fn tn_009_reject_name_starting_with_hyphen() {
        let result = WorkspaceName::parse("-leading");
        assert!(matches!(result, Err(ParseError::BoundaryViolation { .. })));
    }

    #[test]
    fn tn_010_reject_name_ending_with_hyphen() {
        let result = WorkspaceName::parse("trailing-");
        assert!(matches!(result, Err(ParseError::BoundaryViolation { .. })));
    }

    #[test]
    fn tn_011_reject_consecutive_hyphens() {
        let result = WorkspaceName::parse("double--hyphen");
        assert!(matches!(result, Err(ParseError::ConsecutiveHyphens { .. })));
    }
}
