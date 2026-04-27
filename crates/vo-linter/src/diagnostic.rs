#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintCode {
    L002,
}

#[derive(Debug, Clone, PartialEq)]
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

    #[must_use]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }

    #[must_use]
    pub const fn code(&self) -> &LintCode {
        &self.code
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

    #[test]
    fn test_diagnostic_message() {
        let d = Diagnostic::new(LintCode::L002, "random UUID call detected");
        assert_eq!(d.message(), "random UUID call detected");
    }

    #[test]
    fn test_diagnostic_message_empty() {
        let d = Diagnostic::new(LintCode::L002, "");
        assert_eq!(d.message(), "");
    }

    #[test]
    fn test_diagnostic_message_unicode() {
        let d = Diagnostic::new(LintCode::L002, "error: \u{274c} invalid");
        assert!(d.message().contains('\u{274c}'));
    }

    #[test]
    fn test_diagnostic_clone() {
        let d = Diagnostic::new(LintCode::L002, "msg").with_suggestion("fix");
        let d2 = d.clone();
        assert_eq!(d.message(), d2.message());
    }

    #[test]
    fn test_diagnostic_suggestion_none() {
        let d = Diagnostic::new(LintCode::L002, "msg");
        assert!(d.suggestion().is_none());
    }

    #[test]
    fn test_diagnostic_suggestion_some() {
        let d = Diagnostic::new(LintCode::L002, "msg").with_suggestion("use this instead");
        assert!(d.suggestion().is_some());
        assert_eq!(d.suggestion().unwrap(), "use this instead");
    }
}
