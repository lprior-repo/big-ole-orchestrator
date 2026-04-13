#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintCode {
    L002,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    #[allow(dead_code)]
    code: LintCode,
    message: String,
    suggestion: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(code: LintCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
        }
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_new() {
        let d = Diagnostic::new(LintCode::L002, "test message");
        assert!(matches!(d.code, LintCode::L002));
        assert_eq!(d.message, "test message");
        assert!(d.suggestion.is_none());
    }

    #[test]
    fn test_diagnostic_with_suggestion() {
        let d = Diagnostic::new(LintCode::L002, "test message").with_suggestion("use this instead");
        assert!(d.suggestion.is_some());
        assert_eq!(d.suggestion.as_ref().unwrap(), "use this instead");
    }
}
